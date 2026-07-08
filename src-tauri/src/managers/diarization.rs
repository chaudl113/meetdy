//! Speaker diarization manager using sherpa-onnx.
//!
//! Provides "who spoke when" labels for meeting audio using
//! Pyannote segmentation + 3D-Speaker embedding via ONNX Runtime.

use anyhow::Result;
use log::{debug, info};
use serde::{Deserialize, Serialize};
use specta::Type;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Manager};

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct DiarizationSegment {
    pub start_sec: f32,
    pub end_sec: f32,
    pub speaker_id: usize,
}

#[derive(Clone)]
pub struct SpeakerDiarizationManager {
    app_handle: AppHandle,
    segmentation_model_path: Arc<Mutex<Option<PathBuf>>>,
    embedding_model_path: Arc<Mutex<Option<PathBuf>>>,
}

impl SpeakerDiarizationManager {
    pub fn new(app_handle: &AppHandle) -> Result<Self> {
        let models_dir = app_handle
            .path()
            .app_data_dir()
            .map_err(|e| anyhow::anyhow!("Failed to get app data dir: {}", e))?
            .join("models");

        let segmentation_path = models_dir.join("pyannote-segmentation-int8.onnx");
        let embedding_path = models_dir.join("3dspeaker-eres2net.onnx");

        let has_models = segmentation_path.exists() && embedding_path.exists();
        if has_models {
            info!("Speaker diarization models found, diarization available");
        } else {
            info!(
                "Speaker diarization models not found. \
                 Download from sherpa-onnx releases for speaker diarization."
            );
        }

        Ok(Self {
            app_handle: app_handle.clone(),
            segmentation_model_path: Arc::new(Mutex::new(
                if segmentation_path.exists() { Some(segmentation_path) } else { None }
            )),
            embedding_model_path: Arc::new(Mutex::new(
                if embedding_path.exists() { Some(embedding_path) } else { None }
            )),
        })
    }

    pub fn is_available(&self) -> bool {
        let seg = self.segmentation_model_path.lock().unwrap_or_else(|p| p.into_inner());
        let emb = self.embedding_model_path.lock().unwrap_or_else(|p| p.into_inner());
        seg.is_some() && emb.is_some()
    }

    /// Re-check whether diarization model files exist on disk and update internal state.
    /// Call this after a diarization model has been downloaded.
    pub fn reload_availability(&self) {
        let models_dir = self.app_handle
            .path()
            .app_data_dir()
            .ok()
            .map(|d| d.join("models"));

        if let Some(models_dir) = models_dir {
            let segmentation_path = models_dir.join("pyannote-segmentation-int8.onnx");
            let embedding_path = models_dir.join("3dspeaker-eres2net.onnx");

            {
                let mut seg = self.segmentation_model_path.lock().unwrap_or_else(|p| p.into_inner());
                *seg = if segmentation_path.exists() { Some(segmentation_path) } else { None };
            }
            {
                let mut emb = self.embedding_model_path.lock().unwrap_or_else(|p| p.into_inner());
                *emb = if embedding_path.exists() { Some(embedding_path) } else { None };
            }

            if self.is_available() {
                info!("Speaker diarization models reloaded, diarization now available");
            } else {
                info!("Speaker diarization models not yet complete after reload");
            }
        }
    }

    /// Run speaker diarization on a WAV file.
    /// Returns speaker-labeled time segments.
    /// The WAV must be 16kHz mono 16-bit.
    pub fn process(&self, wav_path: &Path) -> Result<Vec<DiarizationSegment>> {
        if !self.is_available() {
            return Err(anyhow::anyhow!(
                "Speaker diarization models not downloaded. \
                 Place pyannote-segmentation-int8.onnx and 3dspeaker-eres2net.onnx in the models directory."
            ));
        }

        let seg_path = self.segmentation_model_path
            .lock().unwrap_or_else(|p| p.into_inner())
            .clone()
            .ok_or_else(|| anyhow::anyhow!("Segmentation model not found"))?;
        let emb_path = self.embedding_model_path
            .lock().unwrap_or_else(|p| p.into_inner())
            .clone()
            .ok_or_else(|| anyhow::anyhow!("Embedding model not found"))?;

        info!("Running speaker diarization on {:?}", wav_path);

        let config = sherpa_onnx::OfflineSpeakerDiarizationConfig {
            segmentation: sherpa_onnx::OfflineSpeakerSegmentationModelConfig {
                pyannote: sherpa_onnx::OfflineSpeakerSegmentationPyannoteModelConfig {
                    model: Some(seg_path.to_string_lossy().to_string()),
                },
                ..Default::default()
            },
            embedding: sherpa_onnx::SpeakerEmbeddingExtractorConfig {
                model: Some(emb_path.to_string_lossy().to_string()),
                ..Default::default()
            },
            clustering: sherpa_onnx::FastClusteringConfig {
                num_clusters: -1, // auto-detect
                threshold: 0.5,
            },
            ..Default::default()
        };

        let sd = sherpa_onnx::OfflineSpeakerDiarization::create(&config)
            .ok_or_else(|| anyhow::anyhow!("Failed to create diarization engine"))?;

        let wave_path_str = wav_path.to_string_lossy().to_string();
        let wave = sherpa_onnx::Wave::read(&wave_path_str)
            .ok_or_else(|| anyhow::anyhow!("Failed to read WAV for diarization: {:?}", wav_path))?;

        let result = sd.process(wave.samples())
            .ok_or_else(|| anyhow::anyhow!("Diarization failed: no result"))?;

        info!(
            "Diarization complete: {} speakers, {} segments",
            result.num_speakers(),
            result.num_segments()
        );

        let segments: Vec<DiarizationSegment> = result
            .sort_by_start_time()
            .iter()
            .map(|s| DiarizationSegment {
                start_sec: s.start,
                end_sec: s.end,
                speaker_id: s.speaker as usize,
            })
            .collect();

        debug!(
            "Diarization segments: {:?}",
            segments.iter().map(|s| format!(
                "Speaker{}: {:.1}s-{:.1}s",
                s.speaker_id, s.start_sec, s.end_sec
            )).collect::<Vec<_>>()
        );

        Ok(segments)
    }
}
