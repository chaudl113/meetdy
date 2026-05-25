//! ChatGPT Plus subscription login.
//!
//! This module opens a Tauri webview window pointing at chatgpt.com so the
//! user can sign in with their ChatGPT Plus account. After login the injected
//! script polls `https://chatgpt.com/api/auth/session` (a public endpoint
//! used by the web app itself) to obtain the short-lived `accessToken` and
//! emits it back to the host app via a Tauri event.
//!
//! IMPORTANT: this relies on undocumented endpoints. OpenAI's ToS does not
//! cover this usage and the endpoints may change without notice. Use at your
//! own risk.

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindowBuilder};

const LOGIN_WINDOW_LABEL: &str = "chatgpt_login";
const LOGIN_URL: &str = "https://chatgpt.com/";

/// JS snippet injected into the login webview. After load it polls
/// `/api/auth/session` until an accessToken is returned, then forwards it to
/// the host via the Tauri IPC `window.__TAURI_INTERNALS__.invoke` API by
/// calling our `complete_chatgpt_login` command.
const INJECT_SCRIPT: &str = r#"
(function() {
  if (window.__meetdyChatgptLoginInstalled) return;
  window.__meetdyChatgptLoginInstalled = true;

  const log = (...args) => console.log('[meetdy-chatgpt-login]', ...args);

  async function pollSession() {
    try {
      const res = await fetch('/api/auth/session', {
        credentials: 'include',
        headers: { 'accept': 'application/json' },
      });
      if (!res.ok) {
        log('session fetch HTTP', res.status);
        return null;
      }
      const data = await res.json();
      if (data && data.accessToken) {
        return data.accessToken;
      }
      return null;
    } catch (err) {
      log('session fetch failed', err);
      return null;
    }
  }

  async function loop() {
    for (let i = 0; i < 600; i++) { // ~20 minutes max
      const token = await pollSession();
      if (token) {
        log('Got accessToken, length=', token.length);
        try {
          await window.__TAURI_INTERNALS__.invoke('complete_chatgpt_login', { accessToken: token });
        } catch (err) {
          log('Failed to send token to host:', err);
        }
        return;
      }
      await new Promise(r => setTimeout(r, 2000));
    }
    log('Timed out waiting for login');
  }

  loop();
})();
"#;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatgptLoginEvent {
    pub access_token: String,
}

/// Opens (or focuses) the ChatGPT login webview window. The user signs in
/// inside that window; once a session is established the injected script
/// captures the access token and calls back into `complete_chatgpt_login`.
#[tauri::command]
#[specta::specta]
pub async fn open_chatgpt_login(app: AppHandle) -> Result<(), String> {
    if let Some(existing) = app.get_webview_window(LOGIN_WINDOW_LABEL) {
        existing
            .set_focus()
            .map_err(|e| format!("Failed to focus login window: {}", e))?;
        return Ok(());
    }

    let url = WebviewUrl::External(
        LOGIN_URL
            .parse()
            .map_err(|e| format!("Invalid login URL: {}", e))?,
    );

    WebviewWindowBuilder::new(&app, LOGIN_WINDOW_LABEL, url)
        .title("Sign in to ChatGPT")
        .inner_size(520.0, 720.0)
        .resizable(true)
        .focused(true)
        .initialization_script(INJECT_SCRIPT)
        .build()
        .map_err(|e| format!("Failed to open login window: {}", e))?;

    Ok(())
}

/// Called by the injected script when it has extracted an access token from
/// the chatgpt.com session. Emits an app-level event so the settings UI can
/// persist the token, then closes the login window.
#[tauri::command]
#[specta::specta]
pub async fn complete_chatgpt_login(app: AppHandle, access_token: String) -> Result<(), String> {
    if access_token.trim().is_empty() {
        return Err("Empty access token".to_string());
    }

    app.emit(
        "chatgpt-login-success",
        ChatgptLoginEvent { access_token },
    )
    .map_err(|e| format!("Failed to emit login event: {}", e))?;

    if let Some(window) = app.get_webview_window(LOGIN_WINDOW_LABEL) {
        let _ = window.close();
    }

    Ok(())
}
