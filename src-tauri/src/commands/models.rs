use crate::funasr_client::FunasrRuntimeStatus;
use crate::managers::diarization::SpeakerDiarizationManager;
use crate::managers::model::{EngineType, ModelInfo, ModelManager};
use crate::managers::transcription::TranscriptionManager;
use crate::settings::{get_settings, write_settings};
use std::fs;
use std::sync::Arc;
use tauri::{AppHandle, Manager, State};

#[tauri::command]
#[specta::specta]
pub async fn get_available_models(
    model_manager: State<'_, Arc<ModelManager>>,
) -> Result<Vec<ModelInfo>, String> {
    let models = model_manager.get_available_models();
    Ok(models)
}

#[tauri::command]
#[specta::specta]
pub async fn get_model_info(
    model_manager: State<'_, Arc<ModelManager>>,
    model_id: String,
) -> Result<Option<ModelInfo>, String> {
    Ok(model_manager.get_model_info(&model_id))
}

#[tauri::command]
#[specta::specta]
pub async fn download_model(
    app_handle: AppHandle,
    model_manager: State<'_, Arc<ModelManager>>,
    model_id: String,
) -> Result<(), String> {
    let engine_type = model_manager
        .get_model_info(&model_id)
        .map(|m| m.engine_type);

    model_manager
        .download_model(&model_id)
        .await
        .map_err(|e| e.to_string())?;

    // After downloading a diarization model, reload availability
    if matches!(engine_type, Some(EngineType::Diarization)) {
        if let Some(diarization_manager) = app_handle.try_state::<Arc<SpeakerDiarizationManager>>() {
            diarization_manager.reload_availability();
        }
    }

    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn delete_model(
    model_manager: State<'_, Arc<ModelManager>>,
    model_id: String,
) -> Result<(), String> {
    model_manager
        .delete_model(&model_id)
        .map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn set_active_model(
    app_handle: AppHandle,
    model_manager: State<'_, Arc<ModelManager>>,
    transcription_manager: State<'_, Arc<TranscriptionManager>>,
    model_id: String,
) -> Result<(), String> {
    // Check if model exists and is available
    let model_info = model_manager
        .get_model_info(&model_id)
        .ok_or_else(|| format!("Model not found: {}", model_id))?;

    if !model_info.is_downloaded {
        return Err(format!("Model not downloaded: {}", model_id));
    }

    // Load the model in the transcription manager
    transcription_manager
        .load_model(&model_id)
        .map_err(|e| e.to_string())?;

    // Update settings
    let mut settings = get_settings(&app_handle);
    settings.selected_model = model_id.clone();
    write_settings(&app_handle, settings);

    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn get_current_model(app_handle: AppHandle) -> Result<String, String> {
    let settings = get_settings(&app_handle);
    Ok(settings.selected_model)
}

#[tauri::command]
#[specta::specta]
pub async fn get_transcription_model_status(
    transcription_manager: State<'_, Arc<TranscriptionManager>>,
) -> Result<Option<String>, String> {
    Ok(transcription_manager.get_current_model())
}

#[tauri::command]
#[specta::specta]
pub async fn is_model_loading(
    transcription_manager: State<'_, Arc<TranscriptionManager>>,
) -> Result<bool, String> {
    Ok(transcription_manager.is_model_loading())
}

#[tauri::command]
#[specta::specta]
pub async fn has_any_models_available(
    model_manager: State<'_, Arc<ModelManager>>,
) -> Result<bool, String> {
    let models = model_manager.get_available_models();
    Ok(models
        .iter()
        .any(|m| m.is_downloaded && !matches!(m.engine_type, EngineType::Diarization)))
}

#[tauri::command]
#[specta::specta]
pub async fn has_any_models_or_downloads(
    model_manager: State<'_, Arc<ModelManager>>,
) -> Result<bool, String> {
    let models = model_manager.get_available_models();
    // Return true if any STT models are downloaded (exclude diarization)
    Ok(models
        .iter()
        .any(|m| m.is_downloaded && !matches!(m.engine_type, EngineType::Diarization)))
}

#[tauri::command]
#[specta::specta]
pub async fn cancel_download(
    model_manager: State<'_, Arc<ModelManager>>,
    model_id: String,
) -> Result<(), String> {
    model_manager
        .cancel_download(&model_id)
        .map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn get_recommended_first_model() -> Result<String, String> {
    // Recommend Parakeet V3 model for first-time users - fastest and most accurate
    Ok("parakeet-tdt-0.6b-v3".to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn update_funasr_runtime_config(
    app_handle: AppHandle,
    base_url: String,
    model: String,
) -> Result<(), String> {
    let trimmed_base_url = base_url.trim();
    let trimmed_model = model.trim();

    if trimmed_base_url.is_empty() {
        return Err("FunASR base URL cannot be empty".to_string());
    }

    if trimmed_model.is_empty() {
        return Err("FunASR model cannot be empty".to_string());
    }

    let mut settings = get_settings(&app_handle);
    settings.funasr_base_url = trimmed_base_url.to_string();
    settings.funasr_model = trimmed_model.to_string();
    write_settings(&app_handle, settings);

    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn get_funasr_runtime_status(
    app_handle: AppHandle,
) -> Result<FunasrRuntimeStatus, String> {
    let settings = get_settings(&app_handle);
    crate::funasr_client::runtime_status(
        &app_handle,
        &settings.funasr_base_url,
        &settings.funasr_model,
    )
    .await
    .map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn setup_funasr_runtime(app_handle: AppHandle) -> Result<(), String> {
    let settings = get_settings(&app_handle);
    crate::funasr_client::setup_local_runtime(
        &app_handle,
        &settings.funasr_base_url,
        &settings.funasr_model,
    )
    .await
    .map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn delete_funasr_runtime(app_handle: AppHandle) -> Result<(), String> {
    let status = crate::funasr_client::runtime_status(
        &app_handle,
        &get_settings(&app_handle).funasr_base_url,
        &get_settings(&app_handle).funasr_model,
    )
    .await
    .map_err(|e| e.to_string())?;

    let runtime_dir = std::path::PathBuf::from(status.runtime_dir);
    if runtime_dir.exists() {
        fs::remove_dir_all(runtime_dir).map_err(|e| e.to_string())?;
    }
    Ok(())
}
