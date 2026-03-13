//! Ollama local LLM management.
//!
//! Provides detection, health checks, server lifecycle, and model management
//! for the Ollama local inference engine. All operations are non-blocking
//! and safe to call from Tauri commands.

use log::{debug, error, info, warn};
use serde::{Deserialize, Serialize};
use specta::Type;
use std::process::Command;
use std::time::Duration;
use tauri::{AppHandle, Emitter};

/// Status of the Ollama service
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum OllamaStatus {
    /// Ollama is not installed on this system
    NotInstalled,
    /// Ollama is installed but the server is not running
    Installed,
    /// Ollama server is running and ready
    Running,
}

/// Information about a locally available Ollama model
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct OllamaModelInfo {
    pub name: String,
    pub size: u64,
    pub modified_at: String,
}

/// Combined status response for the frontend
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct OllamaStatusResponse {
    pub status: OllamaStatus,
    pub models: Vec<OllamaModelInfo>,
    pub version: Option<String>,
    pub binary_path: Option<String>,
}

/// Progress event for model pull operations
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct OllamaPullProgress {
    pub model: String,
    pub status: String,
    pub total: Option<u64>,
    pub completed: Option<u64>,
    pub percent: Option<f64>,
}

/// Find the Ollama binary path, checking common install locations.
fn find_ollama_binary() -> Option<String> {
    // Check PATH first via `which`
    if let Ok(output) = Command::new("which").arg("ollama").output() {
        if output.status.success() {
            let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !path.is_empty() {
                return Some(path);
            }
        }
    }

    // Check common macOS/Linux locations
    let common_paths = [
        "/usr/local/bin/ollama",
        "/opt/homebrew/bin/ollama",
        "/usr/bin/ollama",
    ];

    for path in &common_paths {
        if std::path::Path::new(path).exists() {
            return Some(path.to_string());
        }
    }

    // macOS: check if Ollama.app is installed (GUI version)
    #[cfg(target_os = "macos")]
    {
        let app_binary = "/Applications/Ollama.app/Contents/Resources/ollama";
        if std::path::Path::new(app_binary).exists() {
            return Some(app_binary.to_string());
        }
    }

    None
}

/// Check if the Ollama API server is responding.
async fn check_ollama_health(base_url: &str) -> bool {
    let url = format!("{}/api/tags", base_url.trim_end_matches("/v1"));
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(3))
        .build();

    match client {
        Ok(client) => match client.get(&url).send().await {
            Ok(resp) => resp.status().is_success(),
            Err(_) => false,
        },
        Err(_) => false,
    }
}

/// Get the Ollama version string.
fn get_ollama_version(binary_path: &str) -> Option<String> {
    Command::new(binary_path)
        .arg("--version")
        .output()
        .ok()
        .and_then(|output| {
            if output.status.success() {
                let ver = String::from_utf8_lossy(&output.stdout).trim().to_string();
                if ver.is_empty() {
                    // Some versions print to stderr
                    let ver2 = String::from_utf8_lossy(&output.stderr).trim().to_string();
                    if ver2.is_empty() { None } else { Some(ver2) }
                } else {
                    Some(ver)
                }
            } else {
                None
            }
        })
}

/// List locally available models via the Ollama API.
async fn list_local_models(base_url: &str) -> Vec<OllamaModelInfo> {
    let url = format!("{}/api/tags", base_url.trim_end_matches("/v1"));
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .build();

    let client = match client {
        Ok(c) => c,
        Err(_) => return vec![],
    };

    match client.get(&url).send().await {
        Ok(resp) => {
            if let Ok(body) = resp.json::<serde_json::Value>().await {
                if let Some(models) = body.get("models").and_then(|m| m.as_array()) {
                    return models
                        .iter()
                        .filter_map(|m| {
                            let name = m.get("name")?.as_str()?.to_string();
                            let size = m.get("size").and_then(|s| s.as_u64()).unwrap_or(0);
                            let modified_at = m
                                .get("modified_at")
                                .and_then(|s| s.as_str())
                                .unwrap_or("")
                                .to_string();
                            Some(OllamaModelInfo {
                                name,
                                size,
                                modified_at,
                            })
                        })
                        .collect();
                }
            }
            vec![]
        }
        Err(_) => vec![],
    }
}

/// Start the Ollama server in the background.
fn start_ollama_server(binary_path: &str) -> Result<(), String> {
    info!("Starting Ollama server from: {}", binary_path);

    // On macOS, if the GUI app exists, use `open` to launch it
    #[cfg(target_os = "macos")]
    {
        if std::path::Path::new("/Applications/Ollama.app").exists() {
            info!("Launching Ollama.app");
            Command::new("open")
                .arg("-a")
                .arg("Ollama")
                .spawn()
                .map_err(|e| format!("Failed to launch Ollama.app: {}", e))?;
            return Ok(());
        }
    }

    // Start `ollama serve` as a detached background process
    Command::new(binary_path)
        .arg("serve")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map_err(|e| format!("Failed to start Ollama server: {}", e))?;

    Ok(())
}

/// Wait for the Ollama server to become healthy, with retries.
async fn wait_for_ollama(base_url: &str, max_wait_secs: u64) -> bool {
    let start = std::time::Instant::now();
    let timeout = Duration::from_secs(max_wait_secs);

    while start.elapsed() < timeout {
        if check_ollama_health(base_url).await {
            return true;
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
    }

    false
}

// ──────────────────────────────────────────────────────────────
// Tauri Commands
// ──────────────────────────────────────────────────────────────

/// Check the full status of Ollama: installed? running? which models?
#[tauri::command]
#[specta::specta]
pub async fn check_ollama_status() -> OllamaStatusResponse {
    let binary_path = find_ollama_binary();

    if binary_path.is_none() {
        return OllamaStatusResponse {
            status: OllamaStatus::NotInstalled,
            models: vec![],
            version: None,
            binary_path: None,
        };
    }

    let binary = binary_path.clone().unwrap();
    let version = get_ollama_version(&binary);
    let base_url = "http://localhost:11434";

    let is_running = check_ollama_health(base_url).await;

    if is_running {
        let models = list_local_models(base_url).await;
        OllamaStatusResponse {
            status: OllamaStatus::Running,
            models,
            version,
            binary_path,
        }
    } else {
        OllamaStatusResponse {
            status: OllamaStatus::Installed,
            models: vec![],
            version,
            binary_path,
        }
    }
}

/// Start the Ollama server and wait for it to be ready.
/// Returns true if server started successfully.
#[tauri::command]
#[specta::specta]
pub async fn start_ollama() -> Result<bool, String> {
    let binary = find_ollama_binary()
        .ok_or_else(|| "Ollama is not installed. Please install from https://ollama.com".to_string())?;

    // Check if already running
    if check_ollama_health("http://localhost:11434").await {
        info!("Ollama is already running");
        return Ok(true);
    }

    // Start the server
    start_ollama_server(&binary)?;

    // Wait for server to become ready (up to 15 seconds)
    let ready = wait_for_ollama("http://localhost:11434", 15).await;

    if ready {
        info!("Ollama server started successfully");
        Ok(true)
    } else {
        warn!("Ollama server started but not responding after 15s");
        Err("Ollama server started but is not responding. Please try again.".to_string())
    }
}

/// Pull (download) a model with progress events emitted to frontend.
/// This is a long-running operation that streams progress updates.
#[tauri::command]
#[specta::specta]
pub async fn pull_ollama_model(app: AppHandle, model: String) -> Result<(), String> {
    info!("Pulling Ollama model: {}", model);

    // Ensure Ollama is running
    if !check_ollama_health("http://localhost:11434").await {
        return Err("Ollama server is not running. Please start it first.".to_string());
    }

    let url = "http://localhost:11434/api/pull";
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(3600)) // 1 hour timeout for large models
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {}", e))?;

    let body = serde_json::json!({
        "name": model,
        "stream": true
    });

    let response = client
        .post(url)
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("Failed to start model download: {}", e))?;

    if !response.status().is_success() {
        let err = response
            .text()
            .await
            .unwrap_or_else(|_| "Unknown error".to_string());
        return Err(format!("Model pull failed: {}", err));
    }

    // Stream the NDJSON response for progress updates
    use futures_util::StreamExt;
    let mut stream = response.bytes_stream();
    let mut buffer = String::new();

    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| format!("Stream error: {}", e))?;
        buffer.push_str(&String::from_utf8_lossy(&chunk));

        // Process complete JSON lines
        while let Some(newline_pos) = buffer.find('\n') {
            let line = buffer[..newline_pos].trim().to_string();
            buffer = buffer[newline_pos + 1..].to_string();

            if line.is_empty() {
                continue;
            }

            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&line) {
                let status = json
                    .get("status")
                    .and_then(|s| s.as_str())
                    .unwrap_or("unknown")
                    .to_string();

                let total = json.get("total").and_then(|t| t.as_u64());
                let completed = json.get("completed").and_then(|c| c.as_u64());

                let percent = match (total, completed) {
                    (Some(t), Some(c)) if t > 0 => Some((c as f64 / t as f64) * 100.0),
                    _ => None,
                };

                let progress = OllamaPullProgress {
                    model: model.clone(),
                    status: status.clone(),
                    total,
                    completed,
                    percent,
                };

                // Emit progress event to frontend
                let _ = app.emit("ollama_pull_progress", &progress);

                debug!(
                    "Ollama pull progress: {} - {:.1}%",
                    status,
                    percent.unwrap_or(0.0)
                );

                // Check for error in response
                if let Some(err) = json.get("error").and_then(|e| e.as_str()) {
                    error!("Ollama pull error: {}", err);
                    return Err(format!("Model pull error: {}", err));
                }
            }
        }
    }

    info!("Successfully pulled Ollama model: {}", model);

    // Emit completion event
    let _ = app.emit(
        "ollama_pull_progress",
        OllamaPullProgress {
            model: model.clone(),
            status: "success".to_string(),
            total: None,
            completed: None,
            percent: Some(100.0),
        },
    );

    Ok(())
}

/// Get the install URL for Ollama based on the current platform.
#[tauri::command]
#[specta::specta]
pub fn get_ollama_install_url() -> String {
    #[cfg(target_os = "macos")]
    {
        "https://ollama.com/download/mac".to_string()
    }

    #[cfg(target_os = "windows")]
    {
        "https://ollama.com/download/windows".to_string()
    }

    #[cfg(target_os = "linux")]
    {
        "https://ollama.com/download/linux".to_string()
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
    {
        "https://ollama.com/download".to_string()
    }
}
