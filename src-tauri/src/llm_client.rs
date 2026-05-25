use crate::settings::PostProcessProvider;
use futures_util::StreamExt;
use log::{debug, warn};
use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION, CONTENT_TYPE, REFERER, USER_AGENT};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use uuid::Uuid;

/// Provider id for the ChatGPT Plus subscription path. Kept in one place so
/// `send_chat_completion` and friends can branch on it.
pub const CHATGPT_PLUS_PROVIDER_ID: &str = "chatgpt_plus";

/// Default timeout for LLM API requests (2 minutes)
const LLM_REQUEST_TIMEOUT: Duration = Duration::from_secs(120);

#[derive(Debug, Serialize)]
struct ChatMessage {
    role: String,
    content: String,
}

#[derive(Debug, Serialize)]
struct ChatCompletionRequest {
    model: String,
    messages: Vec<ChatMessage>,
}

#[derive(Debug, Deserialize)]
struct ChatCompletionResponse {
    choices: Vec<ChatChoice>,
}

#[derive(Debug, Deserialize)]
struct ChatChoice {
    message: ChatMessageResponse,
}

#[derive(Debug, Deserialize)]
struct ChatMessageResponse {
    content: Option<String>,
}

/// Build headers for API requests based on provider type
fn build_headers(provider: &PostProcessProvider, api_key: &str) -> Result<HeaderMap, String> {
    let mut headers = HeaderMap::new();

    // Common headers
    headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
    headers.insert(
        REFERER,
        HeaderValue::from_static("https://github.com/chaudl113/meetdy"),
    );
    headers.insert(
        USER_AGENT,
        HeaderValue::from_static("Meetdy/1.0 (+https://github.com/chaudl113/meetdy)"),
    );
    headers.insert("X-Title", HeaderValue::from_static("Meetdy"));

    // Provider-specific auth headers
    if !api_key.is_empty() {
        if provider.id == "anthropic" {
            headers.insert(
                "x-api-key",
                HeaderValue::from_str(api_key)
                    .map_err(|e| format!("Invalid API key header value: {}", e))?,
            );
            headers.insert("anthropic-version", HeaderValue::from_static("2023-06-01"));
        } else {
            headers.insert(
                AUTHORIZATION,
                HeaderValue::from_str(&format!("Bearer {}", api_key))
                    .map_err(|e| format!("Invalid authorization header value: {}", e))?,
            );
        }
    }

    Ok(headers)
}

/// Create an HTTP client with provider-specific headers and timeout
fn create_client(provider: &PostProcessProvider, api_key: &str) -> Result<reqwest::Client, String> {
    let headers = build_headers(provider, api_key)?;
    reqwest::Client::builder()
        .default_headers(headers)
        .timeout(LLM_REQUEST_TIMEOUT)
        .build()
        .map_err(|e| format!("Failed to build HTTP client: {}", e))
}

/// Send a chat completion request to an OpenAI-compatible API
/// Returns Ok(Some(content)) on success, Ok(None) if response has no content,
/// or Err on actual errors (HTTP, parsing, etc.)
pub async fn send_chat_completion(
    provider: &PostProcessProvider,
    api_key: String,
    model: &str,
    prompt: String,
) -> Result<Option<String>, String> {
    // ChatGPT Plus uses the unofficial chatgpt.com web backend, not the
    // standard /chat/completions endpoint.
    if provider.id == CHATGPT_PLUS_PROVIDER_ID {
        return send_chatgpt_plus(api_key, model, prompt).await;
    }

    let base_url = provider.base_url.trim_end_matches('/');
    let url = format!("{}/chat/completions", base_url);

    debug!("Sending chat completion request to: {}", url);

    let client = create_client(provider, &api_key)?;

    let request_body = ChatCompletionRequest {
        model: model.to_string(),
        messages: vec![ChatMessage {
            role: "user".to_string(),
            content: prompt,
        }],
    };

    let response = client
        .post(&url)
        .json(&request_body)
        .send()
        .await
        .map_err(|e| format!("HTTP request failed: {}", e))?;

    let status = response.status();
    if !status.is_success() {
        let error_text = response
            .text()
            .await
            .unwrap_or_else(|_| "Failed to read error response".to_string());
        return Err(format!(
            "API request failed with status {}: {}",
            status, error_text
        ));
    }

    let completion: ChatCompletionResponse = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse API response: {}", e))?;

    Ok(completion
        .choices
        .first()
        .and_then(|choice| choice.message.content.clone()))
}

/// Fetch available models from an OpenAI-compatible API
/// Returns a list of model IDs
pub async fn fetch_models(
    provider: &PostProcessProvider,
    api_key: String,
) -> Result<Vec<String>, String> {
    let base_url = provider.base_url.trim_end_matches('/');
    let url = format!("{}/models", base_url);

    debug!("Fetching models from: {}", url);

    let client = create_client(provider, &api_key)?;

    let response = client
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("Failed to fetch models: {}", e))?;

    let status = response.status();
    if !status.is_success() {
        let error_text = response
            .text()
            .await
            .unwrap_or_else(|_| "Unknown error".to_string());
        return Err(format!(
            "Model list request failed ({}): {}",
            status, error_text
        ));
    }

    let parsed: serde_json::Value = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse response: {}", e))?;

    let mut models = Vec::new();

    // Handle OpenAI format: { data: [ { id: "..." }, ... ] }
    if let Some(data) = parsed.get("data").and_then(|d| d.as_array()) {
        for entry in data {
            if let Some(id) = entry.get("id").and_then(|i| i.as_str()) {
                models.push(id.to_string());
            } else if let Some(name) = entry.get("name").and_then(|n| n.as_str()) {
                models.push(name.to_string());
            }
        }
    }
    // Handle array format: [ "model1", "model2", ... ]
    else if let Some(array) = parsed.as_array() {
        for entry in array {
            if let Some(model) = entry.as_str() {
                models.push(model.to_string());
            }
        }
    }

    Ok(models)
}

// ---------------------------------------------------------------------------
// ChatGPT Plus (unofficial chatgpt.com web endpoint)
// ---------------------------------------------------------------------------

const CHATGPT_PLUS_CONVERSATION_URL: &str = "https://chatgpt.com/backend-api/conversation";
const CHATGPT_PLUS_USER_AGENT: &str =
    "Mozilla/5.0 (Macintosh; Intel Mac OS X 14_0) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0 Safari/537.36";

/// Sends a single-turn request to chatgpt.com using a captured Plus session
/// access token. The endpoint streams Server-Sent Events; we accumulate the
/// final assistant message and return it.
///
/// `model` should match a slug visible in the ChatGPT UI (e.g. "gpt-4o",
/// "gpt-4o-mini", "auto"). The web backend will silently fall back if the
/// account doesn't have access to the requested slug.
async fn send_chatgpt_plus(
    access_token: String,
    model: &str,
    prompt: String,
) -> Result<Option<String>, String> {
    if access_token.trim().is_empty() {
        return Err("ChatGPT Plus is not logged in. Use 'Login with ChatGPT' first.".to_string());
    }

    let client = reqwest::Client::builder()
        .timeout(LLM_REQUEST_TIMEOUT)
        .user_agent(CHATGPT_PLUS_USER_AGENT)
        .build()
        .map_err(|e| format!("Failed to build HTTP client: {}", e))?;

    let message_id = Uuid::new_v4().to_string();
    let parent_id = Uuid::new_v4().to_string();

    let body = serde_json::json!({
        "action": "next",
        "messages": [{
            "id": message_id,
            "author": { "role": "user" },
            "content": { "content_type": "text", "parts": [prompt] },
            "metadata": {},
        }],
        "parent_message_id": parent_id,
        "model": model,
        "timezone_offset_min": 0,
        "history_and_training_disabled": false,
        "conversation_mode": { "kind": "primary_assistant" },
        "force_paragen": false,
        "force_paragen_model_slug": "",
        "force_rate_limit": false,
        "suggestions": [],
    });

    let response = client
        .post(CHATGPT_PLUS_CONVERSATION_URL)
        .bearer_auth(&access_token)
        .header(CONTENT_TYPE, "application/json")
        .header("Accept", "text/event-stream")
        .header("Origin", "https://chatgpt.com")
        .header(REFERER, "https://chatgpt.com/")
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("ChatGPT request failed: {}", e))?;

    let status = response.status();
    if !status.is_success() {
        let error_text = response
            .text()
            .await
            .unwrap_or_else(|_| "Failed to read error response".to_string());
        if status == reqwest::StatusCode::UNAUTHORIZED || status == reqwest::StatusCode::FORBIDDEN {
            return Err(format!(
                "ChatGPT session expired or unauthorized ({}). Please log in again. Details: {}",
                status, error_text
            ));
        }
        return Err(format!(
            "ChatGPT request failed with status {}: {}",
            status, error_text
        ));
    }

    // Stream the SSE response and keep only the final assistant text.
    let mut stream = response.bytes_stream();
    let mut buffer = String::new();
    let mut final_text: Option<String> = None;

    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| format!("Failed to read stream chunk: {}", e))?;
        let chunk_str = std::str::from_utf8(&chunk)
            .map_err(|e| format!("Invalid UTF-8 in stream: {}", e))?;
        buffer.push_str(chunk_str);

        // SSE events are separated by blank lines.
        while let Some(idx) = buffer.find("\n\n") {
            let event_block = buffer[..idx].to_string();
            buffer.drain(..idx + 2);

            for line in event_block.lines() {
                let Some(data) = line.strip_prefix("data:") else {
                    continue;
                };
                let data = data.trim();
                if data == "[DONE]" {
                    continue;
                }
                let Ok(value) = serde_json::from_str::<serde_json::Value>(data) else {
                    continue;
                };

                // The web backend emits assistant deltas inside
                // value["message"]["content"]["parts"][0]. We replace
                // final_text on each event because parts are cumulative.
                if let Some(text) = extract_assistant_text(&value) {
                    final_text = Some(text);
                }
            }
        }
    }

    if final_text.is_none() {
        warn!("ChatGPT stream finished without assistant message");
    }

    Ok(final_text)
}

fn extract_assistant_text(event: &serde_json::Value) -> Option<String> {
    let message = event.get("message")?;
    let role = message
        .pointer("/author/role")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    if role != "assistant" {
        return None;
    }
    let parts = message.pointer("/content/parts")?.as_array()?;
    let mut out = String::new();
    for part in parts {
        if let Some(s) = part.as_str() {
            out.push_str(s);
        }
    }
    if out.is_empty() {
        None
    } else {
        Some(out)
    }
}
