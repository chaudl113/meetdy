//! Whisper speech recognition engine implementation.
//!
//! This module provides a Whisper-based transcription engine that uses
//! OpenAI's Whisper model for speech-to-text conversion. Whisper models
//! are provided as single GGML format files.
//!
//! # Model Format
//!
//! Whisper expects a single model file in GGML format, typically with names like:
//! - `whisper-tiny.bin`
//! - `whisper-base.bin`
//! - `whisper-small.bin`
//! - `whisper-medium.bin`
//! - `whisper-large.bin`
//! - Quantized variants like `whisper-medium-q4_1.bin`
//!
//! # Examples
//!
//! ## Basic Usage
//!
//! ```rust,no_run
//! use transcribe_rs::{TranscriptionEngine, engines::whisper::WhisperEngine};
//! use std::path::PathBuf;
//!
//! let mut engine = WhisperEngine::new();
//! engine.load_model(&PathBuf::from("models/whisper-medium-q4_1.bin"))?;
//!
//! let result = engine.transcribe_file(&PathBuf::from("audio.wav"), None)?;
//! println!("Transcription: {}", result.text);
//!
//! if let Some(segments) = result.segments {
//!     for segment in segments {
//!         println!("[{:.2}s - {:.2}s]: {}", segment.start, segment.end, segment.text);
//!     }
//! }
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
//!
//! ## With Custom Parameters and Initial Prompt
//!
//! ```rust,no_run
//! use transcribe_rs::{TranscriptionEngine, engines::whisper::{WhisperEngine, WhisperInferenceParams}};
//! use std::path::PathBuf;
//!
//! let mut engine = WhisperEngine::new();
//! engine.load_model(&PathBuf::from("models/whisper-medium-q4_1.bin"))?;
//!
//! let params = WhisperInferenceParams {
//!     language: Some("en".to_string()),
//!     translate: false,  // Set to true to translate to English (requires multilingual model)
//!     print_timestamps: true,
//!     suppress_blank: true,
//!     no_speech_thold: 0.6,
//!     initial_prompt: Some("This is a conversation about technology and AI.".to_string()),
//!     ..Default::default()
//! };
//!
//! let result = engine.transcribe_file(&PathBuf::from("audio.wav"), Some(params))?;
//! println!("Transcription: {}", result.text);
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

use crate::{TranscriptionEngine, TranscriptionResult, TranscriptionSegment};
use std::ffi::c_void;
use std::path::{Path, PathBuf};
use std::sync::Once;
use whisper_rs::{
    set_log_callback, FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters,
};

static WHISPER_LOG_INIT: Once = Once::new();

unsafe extern "C" fn silent_whisper_log_callback(
    _level: u32,
    _text: *const i8,
    _user_data: *mut c_void,
) {
}

fn silence_whisper_logs() {
    WHISPER_LOG_INIT.call_once(|| unsafe {
        set_log_callback(Some(silent_whisper_log_callback), std::ptr::null_mut());
    });
}

/// Parameters for configuring Whisper model loading.
///
/// Currently, Whisper model loading doesn't require additional parameters
/// beyond the model file path. This struct exists for API consistency
/// and future extensibility.
#[derive(Debug, Clone, Default)]
pub struct WhisperModelParams {}

/// Parameters for configuring Whisper inference behavior.
///
/// These parameters control various aspects of the transcription process,
/// including language detection, output formatting, and noise suppression.
#[derive(Debug, Clone)]
pub struct WhisperInferenceParams {
    /// Target language for transcription (e.g., "en", "es", "fr").
    /// If None, Whisper will auto-detect the language.
    pub language: Option<String>,

    /// Whether to translate the transcription to English.
    /// Only works with multilingual models (not .en models).
    pub translate: bool,

    /// Whether to print special tokens in the output
    pub print_special: bool,

    /// Whether to print progress information during transcription
    pub print_progress: bool,

    /// Whether to print results in real-time as they're generated
    pub print_realtime: bool,

    /// Whether to include timestamp information in the output
    pub print_timestamps: bool,

    /// Whether to disable timestamp tokens. This avoids whisper.cpp timestamp
    /// heuristics that can skip otherwise valid short chunks.
    pub no_timestamps: bool,

    /// Whether to calculate token-level timestamps.
    pub token_timestamps: bool,

    /// Whether to suppress blank/empty segments in the output
    pub suppress_blank: bool,

    /// Whether to suppress non-speech tokens (e.g., \[BLANK_AUDIO\])
    pub suppress_non_speech_tokens: bool,

    /// Threshold for detecting silence/no-speech segments (0.0-1.0).
    pub no_speech_thold: f32,

    /// Temperature for decoding.
    pub temperature: f32,

    /// Maximum initial timestamp.
    pub max_initial_ts: f32,

    /// Entropy threshold for filtering unstable outputs.
    pub entropy_thold: f32,

    /// Log-probability threshold for filtering low-confidence outputs.
    pub logprob_thold: f32,

    /// Maximum segment length.
    pub max_len: i32,

    /// Whether to force a single segment.
    pub single_segment: bool,

    /// Initial prompt to provide context to the model.
    /// This can be used to improve transcription accuracy by providing
    /// context, vocabulary hints, or style guidance to the model.
    /// Limited to 224 tokens maximum.
    pub initial_prompt: Option<String>,
}

impl Default for WhisperInferenceParams {
    fn default() -> Self {
        Self {
            language: None,
            translate: false,
            print_special: false,
            print_progress: false,
            print_realtime: false,
            print_timestamps: false,
            no_timestamps: true,
            token_timestamps: true,
            suppress_blank: true,
            suppress_non_speech_tokens: true,
            no_speech_thold: 0.55,
            temperature: 0.3,
            max_initial_ts: 1.0,
            entropy_thold: 2.4,
            logprob_thold: -1.0,
            max_len: 200,
            single_segment: false,
            initial_prompt: None,
        }
    }
}

/// Whisper speech recognition engine.
///
/// This engine uses OpenAI's Whisper model for speech-to-text transcription.
/// It supports various Whisper model sizes and quantization levels.
///
/// # Model Requirements
///
/// - **Format**: Single GGML format file (`.bin`)
/// - **Models**: tiny, base, small, medium, large variants
/// - **Quantization**: Supports quantized models (e.g., q4_1, q8_0)
///
/// # Examples
///
/// ```rust,no_run
/// use transcribe_rs::engines::whisper::WhisperEngine;
///
/// let mut engine = WhisperEngine::new();
/// // Engine is ready to load a model
/// ```
pub struct WhisperEngine {
    loaded_model_path: Option<PathBuf>,
    state: Option<whisper_rs::WhisperState>,
    context: Option<whisper_rs::WhisperContext>,
}

impl Default for WhisperEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl WhisperEngine {
    /// Create a new Whisper engine instance.
    ///
    /// The engine starts unloaded - you must call `load_model()` before
    /// performing transcription operations.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use transcribe_rs::engines::whisper::WhisperEngine;
    ///
    /// let engine = WhisperEngine::new();
    /// // Engine is ready to load a model
    /// ```
    pub fn new() -> Self {
        silence_whisper_logs();
        Self {
            loaded_model_path: None,
            state: None,
            context: None,
        }
    }
}

impl Drop for WhisperEngine {
    fn drop(&mut self) {
        self.unload_model();
    }
}

impl TranscriptionEngine for WhisperEngine {
    type InferenceParams = WhisperInferenceParams;
    type ModelParams = WhisperModelParams;

    fn load_model_with_params(
        &mut self,
        model_path: &Path,
        _params: Self::ModelParams,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // Create new context and state following your working pattern
        let context = WhisperContext::new_with_params(
            model_path.to_str().unwrap(),
            WhisperContextParameters::default(),
        )?;

        let state = context.create_state()?;

        self.context = Some(context);
        self.state = Some(state);

        self.loaded_model_path = Some(model_path.to_path_buf());
        Ok(())
    }

    fn unload_model(&mut self) {
        self.loaded_model_path = None;
        self.state = None;
        self.context = None;
    }

    fn transcribe_samples(
        &mut self,
        samples: Vec<f32>,
        params: Option<Self::InferenceParams>,
    ) -> Result<TranscriptionResult, Box<dyn std::error::Error>> {
        let state = self
            .state
            .as_mut()
            .ok_or("Model not loaded. Call load_model() first.")?;

        let whisper_params = params.unwrap_or_default();

        let mut full_params = FullParams::new(SamplingStrategy::BeamSearch {
            beam_size: 3,
            patience: -1.0,
        });
        full_params.set_language(whisper_params.language.as_deref());
        full_params.set_translate(whisper_params.translate);
        full_params.set_print_special(whisper_params.print_special);
        full_params.set_print_progress(whisper_params.print_progress);
        full_params.set_print_realtime(whisper_params.print_realtime);
        full_params.set_print_timestamps(whisper_params.print_timestamps);
        full_params.set_no_timestamps(whisper_params.no_timestamps);
        full_params.set_token_timestamps(whisper_params.token_timestamps);
        full_params.set_suppress_blank(whisper_params.suppress_blank);
        full_params.set_suppress_non_speech_tokens(whisper_params.suppress_non_speech_tokens);
        full_params.set_no_speech_thold(whisper_params.no_speech_thold);
        full_params.set_temperature(whisper_params.temperature);
        full_params.set_max_initial_ts(whisper_params.max_initial_ts);
        full_params.set_entropy_thold(whisper_params.entropy_thold);
        full_params.set_logprob_thold(whisper_params.logprob_thold);
        full_params.set_max_len(whisper_params.max_len);
        full_params.set_single_segment(whisper_params.single_segment);
        
        if let Some(ref prompt) = whisper_params.initial_prompt {
            full_params.set_initial_prompt(prompt);
        }

        state.full(full_params, &samples)?;

        let num_segments = state
            .full_n_segments()
            .expect("failed to get number of segments");

        let mut segments = Vec::new();
        let mut full_text = String::new();

        for i in 0..num_segments {
            let text = state.full_get_segment_text(i)?;
            let start = state.full_get_segment_t0(i)? as f32 / 100.0;
            let end = state.full_get_segment_t1(i)? as f32 / 100.0;

            segments.push(TranscriptionSegment {
                start,
                end,
                text: text.clone(),
                speaker_id: None,
            });
            full_text.push_str(&text);
        }

        Ok(TranscriptionResult {
            text: full_text.trim().to_string(),
            segments: Some(segments),
        })
    }
}
