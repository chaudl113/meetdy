use anyhow::Result;
use log::{info, warn};
use reqwest::multipart;
use serde::Serialize;
use specta::Type;
use std::fs;
use std::fs::OpenOptions;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};

const FUNASR_RUNTIME_DIR: &str = "funasr-runtime";
const FUNASR_VENV_DIR: &str = "venv";
const FUNASR_INSTALL_MARKER: &str = ".meetdy-funasr-installed";
const FUNASR_SERVER_LOG: &str = "server.log";
const FUNASR_PACKAGES: &[&str] = &[
    "torch",
    "torchaudio",
    "funasr",
    "fastapi",
    "uvicorn",
    "python-multipart",
];

#[derive(Debug, Clone, Serialize, Type)]
pub struct FunasrRuntimeStatus {
    pub installed: bool,
    pub server_running: bool,
    pub download_percentage: Option<f32>,
    pub base_url: String,
    pub model: String,
    pub runtime_dir: String,
    pub log_path: String,
    pub message: String,
}

fn normalize_base_url(base_url: &str) -> Result<String> {
    let base_url = base_url
        .trim()
        .trim_end_matches('/')
        .trim_end_matches("/v1")
        .trim_end_matches('/');
    if base_url.is_empty() {
        return Err(anyhow::anyhow!("FunASR base URL is empty"));
    }
    Ok(base_url.to_string())
}

fn local_http_port(base_url: &str) -> Option<u16> {
    let rest = base_url.strip_prefix("http://")?;
    let authority = rest.split('/').next().unwrap_or(rest);
    let (host, port) = authority
        .rsplit_once(':')
        .map(|(host, port)| (host, port.parse::<u16>().ok()))
        .unwrap_or((authority, Some(8000)));

    match host {
        "localhost" | "127.0.0.1" => port,
        _ => None,
    }
}

async fn is_healthy(base_url: &str) -> bool {
    let Ok(base_url) = normalize_base_url(base_url) else {
        return false;
    };
    let Ok(client) = reqwest::Client::builder()
        .timeout(Duration::from_secs(2))
        .build()
    else {
        return false;
    };
    client
        .get(format!("{}/health", base_url))
        .send()
        .await
        .map(|response| response.status().is_success())
        .unwrap_or(false)
}

fn runtime_dir(app: &AppHandle) -> Result<PathBuf> {
    Ok(app.path().app_data_dir()?.join(FUNASR_RUNTIME_DIR))
}

fn venv_dir(app: &AppHandle) -> Result<PathBuf> {
    Ok(runtime_dir(app)?.join(FUNASR_VENV_DIR))
}

fn server_log_path(app: &AppHandle) -> Result<PathBuf> {
    Ok(runtime_dir(app)?.join(FUNASR_SERVER_LOG))
}

fn runtime_paths_ready(app: &AppHandle) -> bool {
    let Ok(runtime) = runtime_dir(app) else {
        return false;
    };
    let Ok(venv) = venv_dir(app) else {
        return false;
    };
    runtime.join(FUNASR_INSTALL_MARKER).exists()
        && venv_python(&venv).exists()
        && venv_server(&venv).exists()
}

fn parse_latest_download_percentage(log_path: &Path) -> Option<f32> {
    let content = fs::read_to_string(log_path).ok()?;
    let mut latest = None;
    for marker in content.match_indices('%') {
        let prefix = &content[..marker.0];
        let number = prefix
            .chars()
            .rev()
            .take_while(|c| c.is_ascii_digit() || *c == '.')
            .collect::<String>()
            .chars()
            .rev()
            .collect::<String>();
        if let Ok(value) = number.parse::<f32>() {
            if (0.0..=100.0).contains(&value) {
                latest = Some(value);
            }
        }
    }
    latest
}

#[cfg(windows)]
fn venv_python(venv: &Path) -> PathBuf {
    venv.join("Scripts").join("python.exe")
}

#[cfg(not(windows))]
fn venv_python(venv: &Path) -> PathBuf {
    venv.join("bin").join("python")
}

#[cfg(windows)]
fn venv_server(venv: &Path) -> PathBuf {
    venv.join("Scripts").join("funasr-server.exe")
}

#[cfg(not(windows))]
fn venv_server(venv: &Path) -> PathBuf {
    venv.join("bin").join("funasr-server")
}

fn find_python() -> Result<String> {
    let candidates = std::env::var("MEETDY_PYTHON")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .map(|value| vec![value])
        .unwrap_or_else(|| vec!["python3".to_string(), "python".to_string()]);

    for candidate in candidates {
        if Command::new(&candidate)
            .arg("--version")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map(|status| status.success())
            .unwrap_or(false)
        {
            return Ok(candidate);
        }
    }

    Err(anyhow::anyhow!(
        "Python 3 was not found. Install Python 3, or set MEETDY_PYTHON to a Python executable."
    ))
}

fn command_error(command: &str, output: std::process::Output) -> anyhow::Error {
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let detail = if !stderr.trim().is_empty() {
        stderr.trim()
    } else {
        stdout.trim()
    };
    anyhow::anyhow!("{} failed: {}", command, detail)
}

#[cfg(unix)]
#[derive(Debug, Clone)]
struct ManagedServerProcess {
    pid: u32,
    command: String,
}

#[cfg(unix)]
fn managed_server_processes(port: u16) -> Vec<ManagedServerProcess> {
    let Ok(output) = Command::new("pgrep")
        .args(["-fl", "funasr-server"])
        .output()
    else {
        return Vec::new();
    };
    if !output.status.success() {
        return Vec::new();
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let port_arg = format!("--port {}", port);
    let port_eq_arg = format!("--port={}", port);
    stdout
        .lines()
        .filter_map(|line| {
            let (pid, command) = line.split_once(char::is_whitespace)?;
            let pid = pid.parse::<u32>().ok()?;
            let command = command.trim().to_string();
            if command.contains(FUNASR_RUNTIME_DIR)
                && (command.contains(&port_arg) || command.contains(&port_eq_arg))
            {
                Some(ManagedServerProcess { pid, command })
            } else {
                None
            }
        })
        .collect()
}

#[cfg(unix)]
fn managed_server_process_exists(port: u16) -> bool {
    !managed_server_processes(port).is_empty()
}

#[cfg(unix)]
fn managed_server_model(port: u16) -> Option<String> {
    managed_server_processes(port)
        .into_iter()
        .find_map(|process| parse_model_from_command(&process.command))
}

#[cfg(unix)]
fn can_verify_managed_server_process() -> bool {
    true
}

#[cfg(unix)]
fn parse_model_from_command(command: &str) -> Option<String> {
    let parts = command.split_whitespace().collect::<Vec<_>>();
    for (index, part) in parts.iter().enumerate() {
        if *part == "--model" {
            return parts.get(index + 1).map(|value| value.to_string());
        }
        if let Some(value) = part.strip_prefix("--model=") {
            return Some(value.to_string());
        }
    }
    None
}

#[cfg(unix)]
fn stop_managed_server_processes(port: u16) {
    for process in managed_server_processes(port) {
        info!(
            "Stopping managed FunASR server process {} before model switch",
            process.pid
        );
        if let Err(e) = Command::new("kill")
            .arg(process.pid.to_string())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
        {
            warn!(
                "Failed to stop managed FunASR server {}: {}",
                process.pid, e
            );
        }
    }
}

#[cfg(not(unix))]
fn managed_server_process_exists(_port: u16) -> bool {
    false
}

#[cfg(not(unix))]
fn managed_server_model(_port: u16) -> Option<String> {
    None
}

#[cfg(not(unix))]
fn can_verify_managed_server_process() -> bool {
    false
}

#[cfg(not(unix))]
fn stop_managed_server_processes(_port: u16) {}

fn ensure_managed_runtime(app: &AppHandle) -> Result<PathBuf> {
    let runtime = runtime_dir(app)?;
    let venv = venv_dir(app)?;
    let python = venv_python(&venv);
    let server = venv_server(&venv);
    let marker = runtime.join(FUNASR_INSTALL_MARKER);

    if marker.exists() && python.exists() && server.exists() {
        return Ok(server);
    }

    fs::create_dir_all(&runtime)?;

    if !python.exists() {
        let system_python = find_python()?;
        let _ = app.emit(
            "funasr_setup_status",
            "Creating managed FunASR Python environment...",
        );
        info!(
            "Creating managed FunASR Python environment at {}",
            venv.display()
        );
        let output = Command::new(system_python)
            .args(["-m", "venv"])
            .arg(&venv)
            .output()?;
        if !output.status.success() {
            return Err(command_error("python -m venv", output));
        }
    }

    let _ = app.emit(
        "funasr_setup_status",
        "Installing managed FunASR runtime. First run may take several minutes...",
    );
    info!("Installing managed FunASR runtime packages");
    let upgrade_output = Command::new(&python)
        .args(["-m", "pip", "install", "--upgrade", "pip"])
        .output()?;
    if !upgrade_output.status.success() {
        return Err(command_error("pip install --upgrade pip", upgrade_output));
    }

    let install_output = Command::new(&python)
        .args(["-m", "pip", "install"])
        .args(FUNASR_PACKAGES)
        .output()?;
    if !install_output.status.success() {
        return Err(command_error("pip install FunASR runtime", install_output));
    }

    if !server.exists() {
        return Err(anyhow::anyhow!(
            "FunASR installed, but funasr-server was not created at {}",
            server.display()
        ));
    }

    fs::write(
        marker,
        format!(
            "Managed by Meetdy. Packages: {}\n",
            FUNASR_PACKAGES.join(" ")
        ),
    )?;

    Ok(server)
}

async fn ensure_local_server_running_inner(
    app: &AppHandle,
    base_url: &str,
    model: &str,
    install_if_missing: bool,
) -> Result<()> {
    let base_url = normalize_base_url(base_url)?;
    let requested_model = model.trim();
    if requested_model.is_empty() {
        return Err(anyhow::anyhow!("FunASR model is empty"));
    }
    info!("Checking FunASR server health at {}", base_url);

    let Some(port) = local_http_port(&base_url) else {
        return Err(anyhow::anyhow!(
            "FunASR server is not reachable at {}. Auto-start only supports http://localhost or http://127.0.0.1.",
            base_url
        ));
    };

    if is_healthy(&base_url).await {
        if let Some(running_model) = managed_server_model(port) {
            if running_model == requested_model {
                info!(
                    "FunASR server is already healthy at {} with model '{}'",
                    base_url, requested_model
                );
                let _ = app.emit(
                    "funasr_setup_status",
                    format!("FunASR server verified with model '{}'.", requested_model),
                );
                return Ok(());
            }

            info!(
                "FunASR server is healthy but running model '{}'; switching to '{}'",
                running_model, requested_model
            );
            let _ = app.emit(
                "funasr_setup_status",
                format!(
                    "Switching FunASR server from '{}' to '{}'...",
                    running_model, requested_model
                ),
            );
            stop_managed_server_processes(port);
            tokio::time::sleep(Duration::from_secs(1)).await;
        } else if can_verify_managed_server_process() {
            let message = format!(
                "A server is responding at {}, but it is not the managed Meetdy FunASR server for model '{}'. Stop the process using port {} or change the FunASR base URL, then press Start/Verify again.",
                base_url, requested_model, port
            );
            warn!("{}", message);
            let _ = app.emit("funasr_setup_status", message.clone());
            return Err(anyhow::anyhow!(message));
        } else {
            warn!(
                "FunASR server is healthy at {}, but process ownership cannot be verified on this platform",
                base_url
            );
            let _ = app.emit(
                "funasr_setup_status",
                format!(
                    "FunASR server is reachable at {}. Process ownership could not be verified on this platform.",
                    base_url
                ),
            );
            return Ok(());
        }
    }

    if !runtime_paths_ready(app) && !install_if_missing {
        return Err(anyhow::anyhow!(
            "FunASR is not set up. Open Models and download/setup FunASR before using it in Meeting."
        ));
    }

    if managed_server_process_exists(port) {
        info!(
            "Managed FunASR server process is already starting on port {}; waiting for health",
            port
        );
        let _ = app.emit(
            "funasr_setup_status",
            "FunASR server is starting. Waiting for it to become ready...",
        );
        for _ in 0..600 {
            if is_healthy(&base_url).await {
                let _ = app.emit("funasr_setup_status", "FunASR server is ready.");
                return Ok(());
            }
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
        return Err(anyhow::anyhow!(
            "Timed out waiting for existing FunASR server at {}.",
            base_url
        ));
    }

    let server = ensure_managed_runtime(app)?;
    let log_path = server_log_path(app)?;
    let stdout_log = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)?;
    let stderr_log = stdout_log.try_clone()?;
    let _ = app.emit(
        "funasr_setup_status",
        format!(
            "Starting FunASR server with model '{}'. It may download/load the model before becoming ready...",
            requested_model
        ),
    );
    info!(
        "FunASR server is not reachable; starting {} with model '{}' on port {}",
        server.display(),
        requested_model,
        port
    );
    Command::new(&server)
        .arg("--model")
        .arg(requested_model)
        .arg("--device")
        .arg("cpu")
        .arg("--port")
        .arg(port.to_string())
        .stdin(Stdio::null())
        .stdout(Stdio::from(stdout_log))
        .stderr(Stdio::from(stderr_log))
        .spawn()
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                warn!("managed funasr-server executable was not found");
                anyhow::anyhow!(
                    "Managed funasr-server was not found at {}. Delete the FunASR runtime folder and try again: {}",
                    server.display(),
                    runtime_dir(app).map(|path| path.display().to_string()).unwrap_or_else(|_| FUNASR_RUNTIME_DIR.to_string())
                )
            } else {
                warn!("Failed to spawn funasr-server: {}", e);
                anyhow::anyhow!("Failed to start funasr-server: {}", e)
            }
        })?;

    for _ in 0..600 {
        if is_healthy(&base_url).await {
            let _ = app.emit(
                "funasr_setup_status",
                format!("FunASR server is ready with model '{}'.", requested_model),
            );
            info!("FunASR server became healthy at {}", base_url);
            return Ok(());
        }
        let _ = app.emit(
            "funasr_setup_status",
            format!(
                "Waiting for FunASR server. First run may download a large model. Log: {}",
                log_path.display()
            ),
        );
        tokio::time::sleep(Duration::from_secs(1)).await;
    }

    warn!(
        "Timed out waiting for FunASR server at {} with model '{}'",
        base_url, requested_model
    );
    Err(anyhow::anyhow!(
        "Timed out waiting for FunASR server at {}. It may still be downloading/loading model '{}'. Log: {}",
        base_url,
        requested_model,
        log_path.display()
    ))
}

pub async fn ensure_local_server_running(
    app: &AppHandle,
    base_url: &str,
    model: &str,
) -> Result<()> {
    ensure_local_server_running_inner(app, base_url, model, false).await
}

pub async fn setup_local_runtime(app: &AppHandle, base_url: &str, model: &str) -> Result<()> {
    ensure_local_server_running_inner(app, base_url, model, true).await
}

pub fn is_runtime_installed(app: &AppHandle) -> bool {
    runtime_paths_ready(app)
}

pub async fn runtime_status(
    app: &AppHandle,
    base_url: &str,
    model: &str,
) -> Result<FunasrRuntimeStatus> {
    let base_url = normalize_base_url(base_url)?;
    let requested_model = model.trim();
    let runtime = runtime_dir(app)?;
    let log_path = server_log_path(app)?;
    let installed = runtime_paths_ready(app);
    let server_healthy = is_healthy(&base_url).await;
    let local_port = local_http_port(&base_url);
    let managed_model = local_port.and_then(managed_server_model);
    let server_running = if server_healthy && can_verify_managed_server_process() {
        managed_model.as_deref() == Some(requested_model)
    } else {
        server_healthy
    };
    let download_percentage = parse_latest_download_percentage(&log_path);
    let message = if server_running {
        if can_verify_managed_server_process() {
            format!("FunASR server is ready with model '{}'.", requested_model)
        } else {
            "FunASR server is ready.".to_string()
        }
    } else if server_healthy && can_verify_managed_server_process() {
        if let Some(running_model) = managed_model {
            format!(
                "FunASR server is running with model '{}'. Press Start/Verify to switch to '{}'.",
                running_model, requested_model
            )
        } else if let Some(port) = local_port {
            format!(
                "Port {} is used by another server. Stop it or change the FunASR base URL before using Meetdy-managed FunASR.",
                port
            )
        } else {
            format!(
                "FunASR server is reachable at {}, but Meetdy can only manage localhost URLs.",
                base_url
            )
        }
    } else if installed {
        download_percentage
            .map(|percent| format!("Downloading FunASR model: {:.0}%", percent))
            .unwrap_or_else(|| {
                "FunASR runtime is installed. Server will start when needed.".to_string()
            })
    } else {
        "FunASR runtime is not installed.".to_string()
    };

    Ok(FunasrRuntimeStatus {
        installed,
        server_running,
        download_percentage,
        base_url,
        model: requested_model.to_string(),
        runtime_dir: runtime.display().to_string(),
        log_path: log_path.display().to_string(),
        message,
    })
}

pub async fn transcribe_file(
    app: &AppHandle,
    base_url: &str,
    model: &str,
    language: Option<&str>,
    audio_path: &Path,
) -> Result<String> {
    let base_url = normalize_base_url(base_url)?;
    if model.trim().is_empty() {
        return Err(anyhow::anyhow!("FunASR model is empty"));
    }
    ensure_local_server_running(app, &base_url, model).await?;

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(30 * 60))
        .build()?;
    let endpoint = format!("{}/v1/audio/transcriptions", base_url);

    let mut form = multipart::Form::new()
        .file("file", audio_path)
        .await?
        .text("model", model.trim().to_string())
        .text("response_format", "json");

    if let Some(language) = language {
        let language = language.trim();
        if !language.is_empty() && language != "auto" {
            form = form.text("language", language.to_string());
        }
    }

    let response = client.post(endpoint).multipart(form).send().await?;
    let status = response.status();
    let body = response.text().await?;
    if !status.is_success() {
        return Err(anyhow::anyhow!(
            "FunASR request failed with status {}: {}",
            status,
            body
        ));
    }

    let parsed: serde_json::Value = serde_json::from_str(&body)
        .map_err(|e| anyhow::anyhow!("Failed to parse FunASR response: {} ({})", e, body))?;
    Ok(parsed
        .get("text")
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .trim()
        .to_string())
}
