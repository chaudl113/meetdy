//! Soniox realtime STT bridge.
//! FE → Rust: raw PCM s16le 16kHz mono bytes via send_audio command
//! Rust → Soniox WS: binary PCM
//! Soniox → Rust: JSON token events
//! Rust → FE: typed SonioxEvent via Tauri Channel

use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use specta::Type;
use std::collections::HashMap;
use std::sync::Mutex;
use tauri::ipc::Channel;
use tauri::State;
use tokio::sync::mpsc;
use tokio_tungstenite::{connect_async, tungstenite::Message};

const SONIOX_ENDPOINT: &str = "wss://stt-rt.soniox.com/transcribe-websocket";

#[derive(Debug, Deserialize, Type)]
pub struct SonioxConfig {
    pub api_key: String,
    /// Source language hint, e.g. "vi", "en", "auto"
    pub source_language: String,
    /// Target language for translation, e.g. "en". Empty = no translation.
    pub target_language: Option<String>,
    /// Session ID for event correlation
    pub session_id: String,
}

#[derive(Debug, Serialize, Clone, Type)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SonioxEvent {
    Status {
        state: String,
    },
    Transcript {
        session_id: String,
        text: String,
        chunk_text: String,
        is_final: bool,
        speaker: Option<String>,
    },
    Error {
        message: String,
    },
    Closed {
        reason: String,
    },
}

struct Session {
    audio_tx: mpsc::UnboundedSender<Vec<u8>>,
    stop_tx: mpsc::UnboundedSender<()>,
}

#[derive(Default)]
pub struct SonioxState {
    sessions: Mutex<HashMap<u64, Session>>,
    next_id: Mutex<u64>,
}

#[tauri::command]
#[specta::specta]
pub async fn soniox_start(
    config: SonioxConfig,
    on_event: Channel<SonioxEvent>,
    state: State<'_, SonioxState>,
) -> Result<u64, String> {
    if config.api_key.trim().is_empty() {
        return Err("Soniox API key is empty".into());
    }

    let session_id = {
        let mut id = state.next_id.lock().unwrap();
        *id += 1;
        *id
    };

    let (audio_tx, audio_rx) = mpsc::unbounded_channel::<Vec<u8>>();
    let (stop_tx, stop_rx) = mpsc::unbounded_channel::<()>();

    state
        .sessions
        .lock()
        .unwrap()
        .insert(session_id, Session { audio_tx, stop_tx });

    let event_ch = on_event.clone();
    tokio::spawn(async move {
        let _ = event_ch.send(SonioxEvent::Status {
            state: "connecting".into(),
        });
        if let Err(e) = run_soniox_session(config, audio_rx, stop_rx, event_ch.clone()).await {
            let _ = event_ch.send(SonioxEvent::Error { message: e });
        }
        let _ = event_ch.send(SonioxEvent::Closed {
            reason: "session_ended".into(),
        });
    });

    Ok(session_id)
}

#[tauri::command]
#[specta::specta]
pub async fn soniox_send_audio(
    session_id: u64,
    pcm: Vec<u8>,
    state: State<'_, SonioxState>,
) -> Result<(), String> {
    let sessions = state.sessions.lock().unwrap();
    let session = sessions
        .get(&session_id)
        .ok_or_else(|| format!("Session {} not found", session_id))?;
    session
        .audio_tx
        .send(pcm)
        .map_err(|e| format!("send audio: {}", e))?;
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn soniox_stop(session_id: u64, state: State<'_, SonioxState>) -> Result<(), String> {
    let mut sessions = state.sessions.lock().unwrap();
    if let Some(session) = sessions.remove(&session_id) {
        let _ = session.stop_tx.send(());
    }
    Ok(())
}

async fn run_soniox_session(
    cfg: SonioxConfig,
    mut audio_rx: mpsc::UnboundedReceiver<Vec<u8>>,
    mut stop_rx: mpsc::UnboundedReceiver<()>,
    event_ch: Channel<SonioxEvent>,
) -> Result<(), String> {
    let (ws_stream, _) = connect_async(SONIOX_ENDPOINT)
        .await
        .map_err(|e| format!("connect failed: {}", e))?;

    let (mut ws_sink, mut ws_stream) = ws_stream.split();

    // Send config message
    let mut config_msg = serde_json::json!({
        "api_key": cfg.api_key,
        "model": "stt-rt-v4",
        "audio_format": "pcm_s16le",
        "sample_rate": 16000,
        "num_channels": 1,
        "enable_endpoint_detection": true,
        "max_endpoint_delay_ms": 3000,
        "enable_speaker_diarization": false,
        "enable_language_identification": false,
    });

    if !cfg.source_language.is_empty() && cfg.source_language != "auto" {
        config_msg["language_hints"] = serde_json::json!([cfg.source_language]);
    }

    if let Some(ref tgt) = cfg.target_language {
        if !tgt.is_empty() {
            config_msg["translation"] = serde_json::json!({
                "type": "one_way",
                "target_language": tgt,
            });
        }
    }

    ws_sink
        .send(Message::Text(config_msg.to_string().into()))
        .await
        .map_err(|e| format!("send config: {}", e))?;

    let _ = event_ch.send(SonioxEvent::Status {
        state: "ready".into(),
    });

    let mut full_text = String::new();
    let session_id = cfg.session_id.clone();

    loop {
        tokio::select! {
            biased;

            _ = stop_rx.recv() => {
                let _ = ws_sink.send(Message::Binary(vec![].into())).await;
                let _ = ws_sink.send(Message::Close(None)).await;
                break;
            }

            Some(pcm) = audio_rx.recv() => {
                if let Err(e) = ws_sink.send(Message::Binary(pcm.into())).await {
                    return Err(format!("send audio: {}", e));
                }
            }

            msg = ws_stream.next() => {
                match msg {
                    Some(Ok(Message::Text(text))) => {
                        handle_soniox_event(&text, &event_ch, &mut full_text, &session_id);
                    }
                    Some(Ok(Message::Close(frame))) => {
                        let reason = frame
                            .map(|f| format!("{}: {}", f.code, f.reason))
                            .unwrap_or_else(|| "remote_close".into());
                        let _ = event_ch.send(SonioxEvent::Closed { reason });
                        break;
                    }
                    Some(Err(e)) => return Err(format!("ws error: {}", e)),
                    None => break,
                    _ => {}
                }
            }
        }
    }

    Ok(())
}

fn handle_soniox_event(
    text: &str,
    event_ch: &Channel<SonioxEvent>,
    full_text: &mut String,
    session_id: &str,
) {
    let value: serde_json::Value = match serde_json::from_str(text) {
        Ok(v) => v,
        Err(_) => return,
    };

    if let Some(code) = value.get("error_code") {
        let msg = value
            .get("error_message")
            .and_then(|v| v.as_str())
            .unwrap_or("Unknown error");
        let _ = event_ch.send(SonioxEvent::Error {
            message: format!("Soniox error {}: {}", code, msg),
        });
        return;
    }

    let tokens = match value.get("tokens").and_then(|v| v.as_array()) {
        Some(t) => t,
        None => return,
    };

    let mut original_text = String::new();
    let mut has_end = false;
    let mut speaker: Option<String> = None;

    for token in tokens {
        let token_text = token.get("text").and_then(|v| v.as_str()).unwrap_or("");
        if token_text == "<end>" {
            has_end = true;
            continue;
        }

        let is_final = token
            .get("is_final")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let trans_status = token
            .get("translation_status")
            .and_then(|v| v.as_str())
            .unwrap_or("original");

        if let Some(sp) = token.get("speaker").and_then(|v| v.as_str()) {
            if speaker.is_none() {
                speaker = Some(sp.to_string());
            }
        }

        if trans_status == "original" && is_final {
            original_text.push_str(token_text);
        }
    }

    if !original_text.is_empty() {
        if !full_text.is_empty() {
            full_text.push(' ');
        }
        full_text.push_str(&original_text);
        let _ = event_ch.send(SonioxEvent::Transcript {
            session_id: session_id.to_string(),
            text: full_text.clone(),
            chunk_text: original_text,
            is_final: has_end,
            speaker,
        });
    }
}
