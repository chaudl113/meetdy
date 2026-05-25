//! Text translation command backed by Google Translate's free web endpoint.
//!
//! This is intentionally a thin wrapper around the public
//! `translate.googleapis.com/translate_a/single` endpoint which doesn't
//! require an API key. It is best-effort: Google may rate-limit or change the
//! response format, in which case the command returns an error and the UI
//! falls back to showing the original text.

use serde_json::Value;

const TRANSLATE_ENDPOINT: &str = "https://translate.googleapis.com/translate_a/single";
const MAX_CHUNK_CHARS: usize = 4500;

/// Translates `text` from `source` language to `target` language.
///
/// `source` may be "auto" to let Google detect the language. Language codes
/// follow ISO 639-1 (e.g. "en", "vi", "ja", "zh-CN").
#[tauri::command]
#[specta::specta]
pub async fn translate_text(
    text: String,
    source: String,
    target: String,
) -> Result<String, String> {
    if text.trim().is_empty() {
        return Ok(String::new());
    }

    let client = reqwest::Client::builder()
        .user_agent("Mozilla/5.0")
        .build()
        .map_err(|e| format!("Failed to build HTTP client: {}", e))?;

    // Google's free endpoint truncates long inputs, so split into chunks at
    // paragraph / sentence boundaries when needed and concatenate the results.
    let chunks = split_for_translation(&text, MAX_CHUNK_CHARS);
    let mut out = String::with_capacity(text.len());

    for (idx, chunk) in chunks.iter().enumerate() {
        let translated = translate_chunk(&client, chunk, &source, &target).await?;
        if idx > 0 {
            out.push('\n');
        }
        out.push_str(&translated);
    }

    Ok(out)
}

async fn translate_chunk(
    client: &reqwest::Client,
    text: &str,
    source: &str,
    target: &str,
) -> Result<String, String> {
    let resp = client
        .get(TRANSLATE_ENDPOINT)
        .query(&[
            ("client", "gtx"),
            ("sl", source),
            ("tl", target),
            ("dt", "t"),
            ("q", text),
        ])
        .send()
        .await
        .map_err(|e| format!("Translate request failed: {}", e))?;

    if !resp.status().is_success() {
        return Err(format!(
            "Translate request returned HTTP {}",
            resp.status().as_u16()
        ));
    }

    let body: Value = resp
        .json()
        .await
        .map_err(|e| format!("Failed to parse translate response: {}", e))?;

    // Response shape: [[[ "translated", "original", null, null, ... ], ...], ...]
    let segments = body
        .get(0)
        .and_then(Value::as_array)
        .ok_or_else(|| "Unexpected translate response shape".to_string())?;

    let mut result = String::new();
    for seg in segments {
        if let Some(piece) = seg.get(0).and_then(Value::as_str) {
            result.push_str(piece);
        }
    }

    Ok(result)
}

/// Splits `text` into chunks no larger than `max_chars`, preferring to break
/// on paragraph and sentence boundaries so the translation reads naturally.
fn split_for_translation(text: &str, max_chars: usize) -> Vec<String> {
    if text.chars().count() <= max_chars {
        return vec![text.to_string()];
    }

    let mut chunks: Vec<String> = Vec::new();
    let mut current = String::new();

    for paragraph in text.split('\n') {
        let para_len = paragraph.chars().count();

        if current.chars().count() + para_len + 1 > max_chars && !current.is_empty() {
            chunks.push(std::mem::take(&mut current));
        }

        if para_len > max_chars {
            // Paragraph itself is too long: fall back to sentence-ish splitting.
            for sentence in paragraph.split_inclusive(['.', '?', '!', '。', '？', '！']) {
                if current.chars().count() + sentence.chars().count() > max_chars
                    && !current.is_empty()
                {
                    chunks.push(std::mem::take(&mut current));
                }
                current.push_str(sentence);
            }
        } else {
            if !current.is_empty() {
                current.push('\n');
            }
            current.push_str(paragraph);
        }
    }

    if !current.is_empty() {
        chunks.push(current);
    }

    chunks
}
