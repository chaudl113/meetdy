use crate::audio_toolkit::apply_custom_words;
use crate::managers::model::{EngineType, ModelManager};
use crate::settings::{get_settings, ModelUnloadTimeout};
use anyhow::Result;
use log::{debug, error, info, warn};
use serde::Serialize;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::thread;
use std::time::{Duration, SystemTime};
use tauri::{AppHandle, Emitter};
use transcribe_rs::{
    engines::{
        parakeet::{
            ParakeetEngine, ParakeetInferenceParams, ParakeetModelParams, TimestampGranularity,
        },
        whisper::{WhisperEngine, WhisperInferenceParams},
    },
    TranscriptionEngine,
};

#[derive(Clone, Debug, Serialize)]
pub struct ModelStateEvent {
    pub event_type: String,
    pub model_id: Option<String>,
    pub model_name: Option<String>,
    pub error: Option<String>,
}

enum LoadedEngine {
    Whisper(WhisperEngine),
    Parakeet(ParakeetEngine),
}

const TRANSCRIPTION_NOISE_PATTERNS: &[&str] = &[
    "ghiền mì gõ",
    "ghien mi go",
    "hãy subscribe cho kênh",
    "hay subscribe cho kenh",
    "đăng ký kênh",
    "dang ky kenh",
    "không bỏ lỡ những video",
    "khong bo lo nhung video",
    "like và subscribe",
    "like va subscribe",
    "nhấn chuông thông báo",
    "nhan chuong thong bao",
    "cảm ơn các bạn đã xem",
    "cam on cac ban da xem",
    "hẹn gặp lại các bạn ở video",
    "hen gap lai cac ban o video",
];

fn is_noise_transcript(text: &str) -> bool {
    let normalized = text.trim().to_lowercase();
    normalized.is_empty()
        || normalized
            .chars()
            .all(|c| c.is_ascii_punctuation() || c.is_whitespace())
        || TRANSCRIPTION_NOISE_PATTERNS
            .iter()
            .any(|pattern| normalized.contains(pattern))
}

fn strip_noise_transcript(text: &str) -> String {
    text.lines()
        .filter(|line| !is_noise_transcript(line))
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string()
}

fn normalize_whisper_language(language: &str) -> String {
    match language {
        "zh-Hans" | "zh-Hant" => "zh".to_string(),
        other => other.to_string(),
    }
}

fn app_language_as_transcription_hint(language: &str) -> Option<String> {
    let lang = language.split(['-', '_']).next().unwrap_or(language);
    match lang {
        // When the UI is Vietnamese, prefer Vietnamese transcription over
        // Whisper auto-detect. Short chunks can otherwise drift to English
        // or — worse — Chinese hallucinations.
        "vi" => Some("vi".to_string()),
        _ => {
            // Fallback: if the OS locale is Vietnamese (e.g. user picked
            // English UI but lives in VN), still hint Vietnamese to keep
            // short live chunks from drifting.
            let os_lang = tauri_plugin_os::locale()
                .and_then(|l| l.split(['-', '_']).next().map(String::from))
                .unwrap_or_default();
            if os_lang == "vi" {
                Some("vi".to_string())
            } else {
                None
            }
        }
    }
}

#[derive(Clone)]
pub struct TranscriptionManager {
    engine: Arc<Mutex<Option<LoadedEngine>>>,
    model_manager: Arc<ModelManager>,
    app_handle: AppHandle,
    current_model_id: Arc<Mutex<Option<String>>>,
    last_activity: Arc<AtomicU64>,
    shutdown_signal: Arc<AtomicBool>,
    watcher_handle: Arc<Mutex<Option<thread::JoinHandle<()>>>>,
    is_loading: Arc<Mutex<bool>>,
    loading_condvar: Arc<Condvar>,
    is_busy: Arc<AtomicBool>,
    // Dedicated lightweight engine used for live transcription so it never
    // shares state or contention with the main (potentially heavy) engine.
    // Loaded lazily on the first live transcribe call.
    live_engine: Arc<Mutex<Option<WhisperEngine>>>,
    live_engine_model_id: Arc<Mutex<Option<String>>>,
}

impl TranscriptionManager {
    pub fn new(app_handle: &AppHandle, model_manager: Arc<ModelManager>) -> Result<Self> {
        let manager = Self {
            engine: Arc::new(Mutex::new(None)),
            model_manager,
            app_handle: app_handle.clone(),
            current_model_id: Arc::new(Mutex::new(None)),
            last_activity: Arc::new(AtomicU64::new(
                SystemTime::now()
                    .duration_since(SystemTime::UNIX_EPOCH)
                    .unwrap()
                    .as_millis() as u64,
            )),
            shutdown_signal: Arc::new(AtomicBool::new(false)),
            watcher_handle: Arc::new(Mutex::new(None)),
            is_loading: Arc::new(Mutex::new(false)),
            loading_condvar: Arc::new(Condvar::new()),
            is_busy: Arc::new(AtomicBool::new(false)),
            live_engine: Arc::new(Mutex::new(None)),
            live_engine_model_id: Arc::new(Mutex::new(None)),
        };

        // Start the idle watcher
        {
            let app_handle_cloned = app_handle.clone();
            let manager_cloned = manager.clone();
            let shutdown_signal = manager.shutdown_signal.clone();
            let handle = thread::spawn(move || {
                while !shutdown_signal.load(Ordering::Relaxed) {
                    thread::sleep(Duration::from_secs(10)); // Check every 10 seconds

                    // Check shutdown signal again after sleep
                    if shutdown_signal.load(Ordering::Relaxed) {
                        break;
                    }

                    let settings = get_settings(&app_handle_cloned);
                    let timeout_seconds = settings.model_unload_timeout.to_seconds();

                    if let Some(limit_seconds) = timeout_seconds {
                        // Skip polling-based unloading for immediate timeout since it's handled directly in transcribe()
                        if settings.model_unload_timeout == ModelUnloadTimeout::Immediately {
                            continue;
                        }

                        let last = manager_cloned.last_activity.load(Ordering::Relaxed);
                        let now_ms = SystemTime::now()
                            .duration_since(SystemTime::UNIX_EPOCH)
                            .unwrap()
                            .as_millis() as u64;

                        if now_ms.saturating_sub(last) > limit_seconds * 1000 {
                            // idle -> unload
                            if manager_cloned.is_model_loaded() {
                                let unload_start = std::time::Instant::now();
                                debug!("Starting to unload model due to inactivity");

                                if let Ok(()) = manager_cloned.unload_model() {
                                    let _ = app_handle_cloned.emit(
                                        "model-state-changed",
                                        ModelStateEvent {
                                            event_type: "unloaded".to_string(),
                                            model_id: None,
                                            model_name: None,
                                            error: None,
                                        },
                                    );
                                    let unload_duration = unload_start.elapsed();
                                    debug!(
                                        "Model unloaded due to inactivity (took {}ms)",
                                        unload_duration.as_millis()
                                    );
                                }
                            }
                        }
                    }
                }
                debug!("Idle watcher thread shutting down gracefully");
            });
            *manager
                .watcher_handle
                .lock()
                .unwrap_or_else(|p| p.into_inner()) = Some(handle);
        }

        Ok(manager)
    }

    pub fn is_model_loaded(&self) -> bool {
        let engine = self.engine.lock().unwrap_or_else(|p| p.into_inner());
        engine.is_some()
    }

    pub fn unload_model(&self) -> Result<()> {
        let unload_start = std::time::Instant::now();
        debug!("Starting to unload model");

        {
            let mut engine = self.engine.lock().unwrap_or_else(|p| p.into_inner());
            if let Some(ref mut loaded_engine) = *engine {
                match loaded_engine {
                    LoadedEngine::Whisper(ref mut whisper) => whisper.unload_model(),
                    LoadedEngine::Parakeet(ref mut parakeet) => parakeet.unload_model(),
                }
            }
            *engine = None; // Drop the engine to free memory
        }
        {
            let mut current_model = self
                .current_model_id
                .lock()
                .unwrap_or_else(|p| p.into_inner());
            *current_model = None;
        }

        // Emit unloaded event
        let _ = self.app_handle.emit(
            "model-state-changed",
            ModelStateEvent {
                event_type: "unloaded".to_string(),
                model_id: None,
                model_name: None,
                error: None,
            },
        );

        let unload_duration = unload_start.elapsed();
        debug!(
            "Model unloaded manually (took {}ms)",
            unload_duration.as_millis()
        );
        Ok(())
    }

    /// Unloads the model immediately if the setting is enabled and the model is loaded
    pub fn maybe_unload_immediately(&self, context: &str) {
        let settings = get_settings(&self.app_handle);
        if settings.model_unload_timeout == ModelUnloadTimeout::Immediately
            && self.is_model_loaded()
        {
            info!("Immediately unloading model after {}", context);
            if let Err(e) = self.unload_model() {
                warn!("Failed to immediately unload model: {}", e);
            }
        }
    }

    pub fn load_model(&self, model_id: &str) -> Result<()> {
        let load_start = std::time::Instant::now();
        debug!("Starting to load model: {}", model_id);

        // Emit loading started event
        let _ = self.app_handle.emit(
            "model-state-changed",
            ModelStateEvent {
                event_type: "loading_started".to_string(),
                model_id: Some(model_id.to_string()),
                model_name: None,
                error: None,
            },
        );

        let model_info = self
            .model_manager
            .get_model_info(model_id)
            .ok_or_else(|| anyhow::anyhow!("Model not found: {}", model_id))?;

        if !model_info.is_downloaded {
            let error_msg = "Model not downloaded";
            let _ = self.app_handle.emit(
                "model-state-changed",
                ModelStateEvent {
                    event_type: "loading_failed".to_string(),
                    model_id: Some(model_id.to_string()),
                    model_name: Some(model_info.name.clone()),
                    error: Some(error_msg.to_string()),
                },
            );
            return Err(anyhow::anyhow!(error_msg));
        }

        let model_path = self.model_manager.get_model_path(model_id)?;

        // Create appropriate engine based on model type
        let loaded_engine = match model_info.engine_type {
            EngineType::Diarization => {
                return Err(anyhow::anyhow!(
                    "Model '{}' is a diarization model and cannot be used for transcription",
                    model_id
                ));
            }
            EngineType::TranscribeCpp => {
                let mut engine = WhisperEngine::new();
                engine.load_model(&model_path).map_err(|e| {
                    let error_msg = format!("Failed to load whisper model {}: {}", model_id, e);
                    let _ = self.app_handle.emit(
                        "model-state-changed",
                        ModelStateEvent {
                            event_type: "loading_failed".to_string(),
                            model_id: Some(model_id.to_string()),
                            model_name: Some(model_info.name.clone()),
                            error: Some(error_msg.clone()),
                        },
                    );
                    anyhow::anyhow!(error_msg)
                })?;
                LoadedEngine::Whisper(engine)
            }
            EngineType::Parakeet => {
                let mut engine = ParakeetEngine::new();
                engine
                    .load_model_with_params(&model_path, ParakeetModelParams::int8())
                    .map_err(|e| {
                        let error_msg =
                            format!("Failed to load parakeet model {}: {}", model_id, e);
                        let _ = self.app_handle.emit(
                            "model-state-changed",
                            ModelStateEvent {
                                event_type: "loading_failed".to_string(),
                                model_id: Some(model_id.to_string()),
                                model_name: Some(model_info.name.clone()),
                                error: Some(error_msg.clone()),
                            },
                        );
                        anyhow::anyhow!(error_msg)
                    })?;
                LoadedEngine::Parakeet(engine)
            }
        };

        // Update the current engine and model ID
        {
            let mut engine = self.engine.lock().unwrap_or_else(|p| p.into_inner());
            *engine = Some(loaded_engine);
        }
        {
            let mut current_model = self
                .current_model_id
                .lock()
                .unwrap_or_else(|p| p.into_inner());
            *current_model = Some(model_id.to_string());
        }

        // Emit loading completed event
        let _ = self.app_handle.emit(
            "model-state-changed",
            ModelStateEvent {
                event_type: "loading_completed".to_string(),
                model_id: Some(model_id.to_string()),
                model_name: Some(model_info.name.clone()),
                error: None,
            },
        );

        let load_duration = load_start.elapsed();
        debug!(
            "Successfully loaded transcription model: {} (took {}ms)",
            model_id,
            load_duration.as_millis()
        );
        Ok(())
    }

    /// Kicks off the model loading in a background thread if it's not already loaded
    pub fn initiate_model_load(&self) {
        let mut is_loading = self.is_loading.lock().unwrap_or_else(|p| p.into_inner());
        if *is_loading || self.is_model_loaded() {
            return;
        }

        *is_loading = true;
        let self_clone = self.clone();
        thread::spawn(move || {
            let settings = get_settings(&self_clone.app_handle);
            // If selected_model is empty, fall back to first downloaded non-diarization model
            let model_id = if settings.selected_model.is_empty() {
                self_clone.model_manager
                    .get_available_models()
                    .into_iter()
                    .find(|m| m.is_downloaded
                        && !matches!(m.engine_type, crate::managers::model::EngineType::Diarization))
                    .map(|m| m.id)
                    .unwrap_or_default()
            } else {
                settings.selected_model.clone()
            };
            if model_id.is_empty() {
                error!("No model selected and no downloaded models found");
            } else if let Err(e) = self_clone.load_model(&model_id) {
                error!("Failed to load model '{}': {}", model_id, e);
            }
            let mut is_loading = self_clone
                .is_loading
                .lock()
                .unwrap_or_else(|p| p.into_inner());
            *is_loading = false;
            self_clone.loading_condvar.notify_all();
        });
    }

    /// Returns true if a model is currently being loaded.
    pub fn is_model_loading(&self) -> bool {
        let loading = self.is_loading.lock().unwrap_or_else(|p| p.into_inner());
        *loading
    }

    pub fn get_current_model(&self) -> Option<String> {
        let current_model = self
            .current_model_id
            .lock()
            .unwrap_or_else(|p| p.into_inner());
        current_model.clone()
    }

    pub fn transcribe(&self, audio: Vec<f32>) -> Result<String> {
        // Block until any in-flight transcription is done so calls remain
        // sequential per engine.
        while self
            .is_busy
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            thread::sleep(Duration::from_millis(5));
        }
        let result = self.transcribe_locked(audio);
        self.is_busy.store(false, Ordering::Release);
        result
    }

    /// Transcribe audio, optionally overriding the model and/or language for this single run.
    ///
    /// When `model_id` is provided the current model is unloaded, the requested model is loaded,
    /// transcription runs, and then the engine is unloaded so the normal selected model reloads
    /// lazily on the next call.
    ///
    /// When `language` is provided it is used instead of the value in settings.
    pub fn transcribe_with_override(
        &self,
        audio: Vec<f32>,
        model_id: Option<&str>,
        language: Option<&str>,
    ) -> Result<String> {
        while self
            .is_busy
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            thread::sleep(Duration::from_millis(5));
        }

        let result = if let Some(mid) = model_id {
            // Unload whatever is currently loaded and load the override model.
            let _ = self.unload_model();
            match self.load_model(mid) {
                Ok(()) => {
                    let r = self.transcribe_locked_with_language(audio, language);
                    // Unload the override so the regular model reloads next time.
                    let _ = self.unload_model();
                    r
                }
                Err(e) => Err(e),
            }
        } else {
            self.transcribe_locked_with_language(audio, language)
        };

        self.is_busy.store(false, Ordering::Release);
        result
    }

    /// Internal transcription with an optional language override.
    fn transcribe_locked_with_language(
        &self,
        audio: Vec<f32>,
        language_override: Option<&str>,
    ) -> Result<String> {
        self.last_activity.store(
            SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64,
            Ordering::Relaxed,
        );

        if audio.is_empty() {
            return Ok(String::new());
        }

        // Ensure model is loaded.
        if self
            .engine
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .is_none()
        {
            self.initiate_model_load();
        }

        {
            let mut is_loading = self.is_loading.lock().unwrap_or_else(|p| p.into_inner());
            while *is_loading {
                is_loading = self.loading_condvar.wait(is_loading).unwrap();
            }
            let engine_guard = self.engine.lock().unwrap_or_else(|p| p.into_inner());
            if engine_guard.is_none() {
                let s = get_settings(&self.app_handle);
                return Err(anyhow::anyhow!(
                    "Model is not loaded for transcription. \
                     selected_model='{}'. \
                     Please select and download a transcription model in Settings → Models.",
                    s.selected_model
                ));
            }
        }

        let settings = get_settings(&self.app_handle);

        let result = {
            let mut engine_guard = self.engine.lock().unwrap_or_else(|p| p.into_inner());
            let engine = engine_guard.as_mut().ok_or_else(|| {
                anyhow::anyhow!("Model failed to load. Please check your model settings.")
            })?;

            match engine {
                LoadedEngine::Whisper(whisper_engine) => {
                    let whisper_language = if let Some(lang) = language_override {
                        if lang == "auto" {
                            app_language_as_transcription_hint(&settings.app_language)
                        } else {
                            Some(normalize_whisper_language(lang))
                        }
                    } else if settings.selected_language == "auto" {
                        app_language_as_transcription_hint(&settings.app_language)
                    } else {
                        Some(normalize_whisper_language(&settings.selected_language))
                    };

                    let is_vietnamese = whisper_language.as_deref() == Some("vi");
                    let initial_prompt = if is_vietnamese {
                        Some(
                            "Đây là cuộc hội thoại tiếng Việt. Chép lại nguyên văn tiếng Việt, không dịch sang tiếng Anh."
                                .to_string(),
                        )
                    } else {
                        None
                    };

                    let params = WhisperInferenceParams {
                        language: whisper_language,
                        translate: settings.translate_to_english && !is_vietnamese,
                        initial_prompt,
                        ..Default::default()
                    };

                    whisper_engine
                        .transcribe_samples(audio, Some(params))
                        .map_err(|e| anyhow::anyhow!("Whisper transcription failed: {}", e))?
                }
                LoadedEngine::Parakeet(parakeet_engine) => {
                    let params = ParakeetInferenceParams {
                        timestamp_granularity: TimestampGranularity::Segment,
                        ..Default::default()
                    };
                    parakeet_engine
                        .transcribe_samples(audio, Some(params))
                        .map_err(|e| anyhow::anyhow!("Parakeet transcription failed: {}", e))?
                }
            }
        };

        let corrected_result = if !settings.custom_words.is_empty() {
            apply_custom_words(
                &result.text,
                &settings.custom_words,
                settings.word_correction_threshold,
            )
        } else {
            result.text
        };

        Ok(strip_noise_transcript(&corrected_result))
    }

    /// Transcribe a chunk using the dedicated lightweight live engine.
    /// Falls back to the main `transcribe()` if the live model isn't
    /// available (e.g. Whisper Small not downloaded).
    pub fn transcribe_live(&self, audio: Vec<f32>) -> Result<String> {
        if audio.is_empty() {
            return Ok(String::new());
        }

        // Pick the live model. Prefer "small" since it's small + fast.
        // If not downloaded, fall back to the main engine.
        let live_model_id = self.pick_live_model_id();
        let Some(live_model_id) = live_model_id else {
            warn!(
                "transcribe_live: no live model available (Whisper Small/Medium-Q5/Turbo-Q5 \
                 not downloaded). Falling back to main engine — live transcription will be SLOW."
            );
            return self.transcribe(audio);
        };
        info!(
            "transcribe_live: using model='{}' for {} samples ({:.1}s)",
            live_model_id,
            audio.len(),
            audio.len() as f32 / 16_000.0
        );

        // Lazy-load / reload the live engine if model changed.
        self.ensure_live_engine_loaded(&live_model_id)?;

        let settings = get_settings(&self.app_handle);
        let whisper_language = if settings.selected_language == "auto" {
            app_language_as_transcription_hint(&settings.app_language)
        } else {
            Some(normalize_whisper_language(&settings.selected_language))
        };
        let is_vietnamese = whisper_language.as_deref() == Some("vi");
        let initial_prompt = if is_vietnamese {
            Some(
                "Đây là cuộc hội thoại tiếng Việt. Chép lại nguyên văn tiếng Việt, không dịch sang tiếng Anh."
                    .to_string(),
            )
        } else {
            None
        };
        let params = WhisperInferenceParams {
            language: whisper_language,
            translate: settings.translate_to_english && !is_vietnamese,
            initial_prompt,
            ..Default::default()
        };

        let st = std::time::Instant::now();
        let mut guard = self.live_engine.lock().unwrap_or_else(|p| p.into_inner());
        let engine = guard
            .as_mut()
            .ok_or_else(|| anyhow::anyhow!("Live engine not loaded"))?;
        let result = engine
            .transcribe_samples(audio, Some(params))
            .map_err(|e| anyhow::anyhow!("Live transcription failed: {}", e))?;
        let text = result.text.trim().to_string();
        info!(
            "Live transcription completed in {}ms ({} chars)",
            st.elapsed().as_millis(),
            text.len()
        );
        let corrected = if settings.custom_words.is_empty() {
            text
        } else {
            apply_custom_words(
                &text,
                &settings.custom_words,
                settings.word_correction_threshold,
            )
        };
        Ok(corrected)
    }

    fn pick_live_model_id(&self) -> Option<String> {
        // Prefer a small fast Whisper model. Use whichever the user has
        // downloaded, ordered by preference.
        for candidate in ["small", "medium-q5", "turbo-q5"] {
            match self.model_manager.get_model_info(candidate) {
                Some(info) => {
                    info!(
                        "pick_live_model_id: '{}' is_downloaded={} is_downloading={}",
                        candidate, info.is_downloaded, info.is_downloading
                    );
                    if info.is_downloaded {
                        return Some(candidate.to_string());
                    }
                }
                None => {
                    info!("pick_live_model_id: '{}' not in registry", candidate);
                }
            }
        }
        None
    }

    fn ensure_live_engine_loaded(&self, model_id: &str) -> Result<()> {
        let mut current = self
            .live_engine_model_id
            .lock()
            .unwrap_or_else(|p| p.into_inner());
        if current.as_deref() == Some(model_id) {
            debug!("ensure_live_engine_loaded: '{}' already loaded", model_id);
            return Ok(());
        }

        info!(
            "ensure_live_engine_loaded: loading '{}' (previous='{:?}')",
            model_id, *current
        );
        let model_path = self.model_manager.get_model_path(model_id)?;
        info!(
            "ensure_live_engine_loaded: model path resolved to {:?}",
            model_path
        );
        let load_start = std::time::Instant::now();
        let mut engine = WhisperEngine::new();
        engine
            .load_model(&model_path)
            .map_err(|e| anyhow::anyhow!("Failed to load live model {}: {}", model_id, e))?;

        let mut guard = self.live_engine.lock().unwrap_or_else(|p| p.into_inner());
        *guard = Some(engine);
        *current = Some(model_id.to_string());
        info!(
            "Live engine loaded: {} (took {}ms)",
            model_id,
            load_start.elapsed().as_millis()
        );
        Ok(())
    }

    fn transcribe_locked(&self, audio: Vec<f32>) -> Result<String> {
        // Update last activity timestamp
        self.last_activity.store(
            SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64,
            Ordering::Relaxed,
        );

        let st = std::time::Instant::now();

        debug!("Audio vector length: {}", audio.len());

        if audio.is_empty() {
            debug!("Empty audio vector");
            self.maybe_unload_immediately("empty audio");
            return Ok(String::new());
        }

        // Ensure the selected model is loaded. Startup preloading normally
        // covers this, but meeting transcription can race app startup or run
        // after an immediate-unload setting, so make transcribe() robust by
        // kicking off a load and waiting for it here as well.
        if self
            .engine
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .is_none()
        {
            self.initiate_model_load();
        }

        {
            let mut is_loading = self.is_loading.lock().unwrap_or_else(|p| p.into_inner());
            while *is_loading {
                is_loading = self.loading_condvar.wait(is_loading).unwrap();
            }

            let engine_guard = self.engine.lock().unwrap_or_else(|p| p.into_inner());
            if engine_guard.is_none() {
                let settings = get_settings(&self.app_handle);
                return Err(anyhow::anyhow!(
                    "Model is not loaded for transcription. \
                     selected_model='{}'. \
                     Please select and download a transcription model in Settings → Models.",
                    settings.selected_model
                ));
            }
        }

        // Get current settings for configuration
        let settings = get_settings(&self.app_handle);

        // Perform transcription with the appropriate engine
        let result = {
            let mut engine_guard = self.engine.lock().unwrap_or_else(|p| p.into_inner());
            let engine = engine_guard.as_mut().ok_or_else(|| {
                anyhow::anyhow!(
                    "Model failed to load. Please check your model settings."
                )
            })?;

            match engine {
                LoadedEngine::Whisper(whisper_engine) => {
                    let whisper_language = if settings.selected_language == "auto" {
                        app_language_as_transcription_hint(&settings.app_language)
                    } else {
                        Some(normalize_whisper_language(&settings.selected_language))
                    };

                    let is_vietnamese = whisper_language.as_deref() == Some("vi");
                    let initial_prompt = if is_vietnamese {
                        Some(
                            "Đây là cuộc hội thoại tiếng Việt. Chép lại nguyên văn tiếng Việt, không dịch sang tiếng Anh."
                                .to_string(),
                        )
                    } else {
                        None
                    };

                    let params = WhisperInferenceParams {
                        language: whisper_language,
                        translate: settings.translate_to_english && !is_vietnamese,
                        initial_prompt,
                        ..Default::default()
                    };

                    whisper_engine
                        .transcribe_samples(audio, Some(params))
                        .or_else(|e| {
                            // Check if this is a UTF-8 error
                            let err_msg = e.to_string();
                            if err_msg.contains("Invalid UTF-8") {
                                warn!("Whisper returned invalid UTF-8, returning empty transcription: {}", err_msg);
                                // Return empty transcription result
                                Ok(transcribe_rs::TranscriptionResult {
                                    text: String::new(),
                                    segments: Some(vec![]),
                                })
                            } else {
                                Err(e)
                            }
                        })
                        .map_err(|e| anyhow::anyhow!("Whisper transcription failed: {}", e))?
                }
                LoadedEngine::Parakeet(parakeet_engine) => {
                    let params = ParakeetInferenceParams {
                        timestamp_granularity: TimestampGranularity::Segment,
                        ..Default::default()
                    };

                    info!(
                        "Running Parakeet transcription on {} audio samples",
                        audio.len()
                    );

                    parakeet_engine
                        .transcribe_samples(audio, Some(params))
                        .inspect_err(|e| {
                            error!("Parakeet transcription error details: {:?}", e);
                        })
                        .map_err(|e| anyhow::anyhow!("Parakeet transcription failed: {}", e))?
                }
            }
        };

        // Apply word correction if custom words are configured
        let corrected_result = if !settings.custom_words.is_empty() {
            apply_custom_words(
                &result.text,
                &settings.custom_words,
                settings.word_correction_threshold,
            )
        } else {
            result.text
        };

        let et = std::time::Instant::now();
        let translation_note = if settings.translate_to_english {
            " (translated)"
        } else {
            ""
        };
        info!(
            "Transcription completed in {}ms{}",
            (et - st).as_millis(),
            translation_note
        );

        let final_result = strip_noise_transcript(&corrected_result);

        if final_result.is_empty() {
            if corrected_result.trim().is_empty() {
                info!("Transcription result is empty");
            } else {
                debug!("Suppressed noisy transcription result");
            }
        } else {
            info!("Transcription result: {}", final_result);
        }

        self.maybe_unload_immediately("transcription");

        Ok(final_result)
    }
}

impl Drop for TranscriptionManager {
    fn drop(&mut self) {
        debug!("Shutting down TranscriptionManager");

        // Signal the watcher thread to shutdown
        self.shutdown_signal.store(true, Ordering::Relaxed);

        // Wait for the thread to finish gracefully
        if let Some(handle) = self
            .watcher_handle
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .take()
        {
            if let Err(e) = handle.join() {
                warn!("Failed to join idle watcher thread: {:?}", e);
            } else {
                debug!("Idle watcher thread joined successfully");
            }
        }
    }
}
