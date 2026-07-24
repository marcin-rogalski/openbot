//! Minimal client for an OpenAI-compatible chat completions server (oMLX / MLX
//! / anything compatible). Non-streaming for now; metrics are best-effort.

use std::time::Instant;

use serde::{Deserialize, Serialize};

use crate::bot::Metrics;
use crate::config::BotConfig;

/// A chat message in the OpenAI schema. Reused by `discord.rs` to build the
/// conversation sent to the model.
#[derive(Clone, Serialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

impl ChatMessage {
    pub fn system(content: impl Into<String>) -> Self {
        Self { role: "system".into(), content: content.into() }
    }
    pub fn user(content: impl Into<String>) -> Self {
        Self { role: "user".into(), content: content.into() }
    }
    pub fn assistant(content: impl Into<String>) -> Self {
        Self { role: "assistant".into(), content: content.into() }
    }
}

#[derive(Serialize)]
struct ChatRequest<'a> {
    model: &'a str,
    messages: &'a [ChatMessage],
    temperature: f32,
    stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<u32>,
}

#[derive(Deserialize)]
struct ChatResponse {
    #[serde(default)]
    choices: Vec<Choice>,
    #[serde(default)]
    usage: Option<Usage>,
}

#[derive(Deserialize)]
struct Choice {
    message: RespMessage,
}

#[derive(Deserialize)]
struct RespMessage {
    #[serde(default)]
    content: Option<String>,
}

#[derive(Deserialize)]
struct Usage {
    #[serde(default)]
    completion_tokens: Option<u64>,
    // Some MLX servers report throughput directly; use it when present.
    #[serde(default)]
    prompt_tps: Option<f64>,
    #[serde(default)]
    generation_tps: Option<f64>,
    #[serde(default)]
    completion_tps: Option<f64>,
}

/// Run a chat completion. Returns the reply text and best-effort throughput.
pub async fn chat(cfg: &BotConfig, messages: Vec<ChatMessage>) -> Result<(String, Metrics), String> {
    request(cfg, messages, None, 0.7).await
}

/// Cheap yes/no gate for the follow-up window: should the assistant jump into
/// this conversation? Yes if the new message is addressed to the assistant, is a
/// follow-up to something it said, or is clearly relevant to a topic it can help
/// with; no for general chatter between other people. Returns false on any error
/// (fail closed — don't butt in when unsure).
pub async fn should_engage(cfg: &BotConfig, context: &str, message: &str) -> bool {
    let messages = vec![
        ChatMessage::system(
            "You decide whether an assistant should reply to a new message in an ongoing \
             Discord conversation it was recently part of. Answer 'yes' if the message is \
             addressed to the assistant, follows up on something the assistant said, or is \
             clearly relevant to a question or topic the assistant can help with. Answer 'no' \
             for chatter between other people that isn't for the assistant. Reply with exactly \
             'yes' or 'no'.",
        ),
        ChatMessage::user(format!(
            "Recent conversation:\n{context}\n\nNew message:\n{message}\n\n\
             Should the assistant reply? Answer yes or no."
        )),
    ];
    match request(cfg, messages, Some(3), 0.0).await {
        Ok((text, _)) => text.trim().to_lowercase().starts_with("yes"),
        Err(_) => false,
    }
}

/// A tool call parsed out of the model's ReAct output.
pub struct ToolCall {
    pub tool: String,
    pub args: serde_json::Value,
}

/// Find a `TOOL_CALL { … }` directive in the model's text and extract the tool
/// name + args. Tolerates surrounding text and trailing content after the JSON.
pub fn parse_tool_call(text: &str) -> Option<ToolCall> {
    let idx = text.find("TOOL_CALL")?;
    let after = &text[idx + "TOOL_CALL".len()..];
    let brace = after.find('{')?;
    // Read exactly one JSON value starting at the first brace, ignoring the rest.
    let value = serde_json::Deserializer::from_str(&after[brace..])
        .into_iter::<serde_json::Value>()
        .next()?
        .ok()?;
    let tool = value.get("tool")?.as_str()?.to_string();
    let args = value.get("args").cloned().unwrap_or_else(|| serde_json::json!({}));
    Some(ToolCall { tool, args })
}

async fn request(
    cfg: &BotConfig,
    messages: Vec<ChatMessage>,
    max_tokens: Option<u32>,
    temperature: f32,
) -> Result<(String, Metrics), String> {
    let client = reqwest::Client::new();
    let body = ChatRequest {
        model: &cfg.model_name,
        messages: &messages,
        temperature,
        stream: false,
        max_tokens,
    };

    let mut req = client.post(cfg.chat_url()).json(&body);
    if !cfg.api_key.trim().is_empty() {
        req = req.bearer_auth(cfg.api_key.trim());
    }

    let started = Instant::now();
    let resp = req.send().await.map_err(|e| format!("request failed: {e}"))?;
    if !resp.status().is_success() {
        let status = resp.status();
        let detail = resp.text().await.unwrap_or_default();
        return Err(format!("model server returned {status}: {}", detail.trim()));
    }

    let parsed: ChatResponse = resp.json().await.map_err(|e| format!("bad response: {e}"))?;
    let elapsed = started.elapsed().as_secs_f64();

    let content = parsed
        .choices
        .into_iter()
        .next()
        .and_then(|c| c.message.content)
        .unwrap_or_default();

    Ok((content, derive_metrics(parsed.usage.as_ref(), elapsed)))
}

fn derive_metrics(usage: Option<&Usage>, elapsed_secs: f64) -> Metrics {
    let Some(usage) = usage else {
        return Metrics { prefill_tps: None, inference_tps: None };
    };
    let prefill_tps = usage.prompt_tps;
    let inference_tps = usage.generation_tps.or(usage.completion_tps).or_else(|| {
        match usage.completion_tokens {
            Some(tokens) if elapsed_secs > 0.0 => Some(tokens as f64 / elapsed_secs),
            _ => None,
        }
    });
    Metrics { prefill_tps, inference_tps }
}
