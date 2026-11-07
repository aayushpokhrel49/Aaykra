use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use futures::AsyncReadExt as _;
use http_client::{AsyncBody, HttpClient, Method, Request};
use serde_json::{Value, json};

/// Configuration for the user's own OpenAI-compatible endpoint.
#[derive(Debug, Clone)]
pub struct ApiConfig {
    pub base_url: String,
    pub api_key: String,
    pub model: String,
}

/// Streams a chat completion from an OpenAI-compatible API, calling
/// `on_text` with each content delta as it arrives. `cancel` can be set to
/// stop the request early.
pub async fn stream_chat(
    http: Arc<dyn HttpClient>,
    config: ApiConfig,
    messages: &[(String, String)],
    cancel: &AtomicBool,
    mut on_text: impl FnMut(&str),
) -> anyhow::Result<()> {
    let url = format!("{}/chat/completions", config.base_url.trim_end_matches('/'));
    let body = json!({
        "model": config.model,
        "messages": messages
            .iter()
            .map(|(role, content)| json!({"role": role, "content": content}))
            .collect::<Vec<_>>(),
        "stream": true,
    });

    let mut builder = Request::builder()
        .method(Method::POST)
        .uri(&url)
        .header("Content-Type", "application/json")
        .header("Accept", "text/event-stream");
    if !config.api_key.is_empty() {
        builder = builder.header("Authorization", format!("Bearer {}", config.api_key));
    }

    let response = http.send(builder.body(AsyncBody::from(body.to_string()))?).await?;
    let status = response.status();
    if !status.is_success() {
        let mut error_text = String::new();
        response.into_body().read_to_string(&mut error_text).await?;
        anyhow::bail!("API returned {status}: {error_text}");
    }

    let mut body = response.into_body();
    let mut line = Vec::new();
    loop {
        if cancel.load(Ordering::Relaxed) {
            return Ok(());
        }
        let mut chunk = [0u8; 4096];
        let n = body.read(&mut chunk).await?;
        if n == 0 {
            break;
        }
        for &byte in &chunk[..n] {
            line.push(byte);
            if byte == b'\n' {
                process_sse_line(&line, &mut on_text)?;
                line.clear();
            }
        }
    }
    if !line.is_empty() {
        process_sse_line(&line, &mut on_text)?;
    }
    Ok(())
}

pub async fn fetch_models(
    http: Arc<dyn HttpClient>,
    config: &ApiConfig,
) -> anyhow::Result<Vec<String>> {
    let url = format!("{}/models", config.base_url.trim_end_matches('/'));
    let mut builder = Request::builder()
        .method(Method::GET)
        .uri(&url)
        .header("Accept", "application/json");
    if !config.api_key.is_empty() {
        builder = builder.header("Authorization", format!("Bearer {}", config.api_key));
    }
    let response = http.send(builder.body(AsyncBody::empty())?).await?;
    let status = response.status();
    if !status.is_success() {
        let mut error_text = String::new();
        response.into_body().read_to_string(&mut error_text).await?;
        anyhow::bail!("API returned {status}: {error_text}");
    }
    let mut text = String::new();
    response.into_body().read_to_string(&mut text).await?;
    let value: Value = serde_json::from_str(&text)?;
    Ok(value["data"]
        .as_array()
        .map(|models| {
            models
                .iter()
                .filter_map(|model| model["id"].as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default())
}

fn process_sse_line(
    line: &[u8],
    on_text: &mut impl FnMut(&str),
) -> anyhow::Result<()> {
    let line = String::from_utf8_lossy(line);
    let line = line.trim();
    if !line.starts_with("data:") {
        return Ok(());
    }
    let data = line["data:".len()..].trim();
    if data.is_empty() || data == "[DONE]" {
        return Ok(());
    }
    let value: Value = serde_json::from_str(data)?;
    if let Some(delta) = value["choices"][0]["delta"]["content"].as_str() {
        on_text(delta);
    }
    Ok(())
}
