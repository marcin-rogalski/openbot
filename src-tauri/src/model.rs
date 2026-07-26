//! Minimal client for an OpenAI-compatible chat completions server (oMLX / MLX
//! / anything compatible). Non-streaming for now; metrics are best-effort.

use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use crate::bot::Metrics;
use crate::config::BotConfig;

/// Cap reply generation so a runaway/looping model can't generate forever.
const MAX_REPLY_TOKENS: u32 = 5000;
/// Hard timeout for a model HTTP call, so a stuck server can't hang a bot.
const REQUEST_TIMEOUT_SECS: u64 = 300;
/// Repetition-loop guard: examine the last `REPEAT_WINDOW` chars every
/// `REPEAT_CHECK_EVERY` new chars, and abort if they're a short cycle repeated.
const REPEAT_WINDOW: usize = 160;
const REPEAT_CHECK_EVERY: usize = 40;

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
    #[serde(skip_serializing_if = "Option::is_none")]
    stream_options: Option<StreamOptions>,
    /// Discourage repetition loops (whitespace/token runs) during long replies.
    #[serde(skip_serializing_if = "Option::is_none")]
    frequency_penalty: Option<f32>,
}

#[derive(Serialize)]
struct StreamOptions {
    include_usage: bool,
}

// --- Streaming (SSE) chunk shapes ------------------------------------------

#[derive(Deserialize)]
struct StreamChunk {
    #[serde(default)]
    choices: Vec<StreamChoice>,
    #[serde(default)]
    usage: Option<Usage>,
}

#[derive(Deserialize)]
struct StreamChoice {
    #[serde(default)]
    delta: Delta,
}

#[derive(Deserialize, Default)]
struct Delta {
    #[serde(default)]
    content: Option<String>,
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

/// Run a streaming chat completion. `on_progress` is called with the full text
/// accumulated so far as tokens arrive (for live display). Returns the final
/// reply text and best-effort throughput.
pub async fn chat(
    cfg: &BotConfig,
    messages: Vec<ChatMessage>,
    mut on_progress: impl FnMut(&str),
) -> Result<(String, Metrics), String> {
    use futures_util::StreamExt;

    let body = ChatRequest {
        model: &cfg.model.model_name,
        messages: &messages,
        temperature: 0.7,
        stream: true,
        max_tokens: Some(MAX_REPLY_TOKENS),
        stream_options: Some(StreamOptions { include_usage: true }),
        frequency_penalty: Some(0.3),
    };

    let mut req = reqwest::Client::new()
        .post(cfg.chat_url())
        .timeout(Duration::from_secs(REQUEST_TIMEOUT_SECS))
        .json(&body);
    if !cfg.model.api_key.trim().is_empty() {
        req = req.bearer_auth(cfg.model.api_key.trim());
    }

    let started = Instant::now();
    let resp = req.send().await.map_err(|e| format!("request failed: {e}"))?;
    if !resp.status().is_success() {
        let status = resp.status();
        let detail = resp.text().await.unwrap_or_default();
        return Err(format!("model server returned {status}: {}", detail.trim()));
    }

    let mut stream = resp.bytes_stream();
    let mut buffer = String::new();
    let mut full = String::new();
    let mut usage: Option<Usage> = None;
    // Guard against a degenerate repetition loop (any repeated char/phrase).
    let mut aborted = false;
    let mut last_check = 0usize;

    'outer: while let Some(item) = stream.next().await {
        let bytes = item.map_err(|e| format!("stream error: {e}"))?;
        buffer.push_str(&String::from_utf8_lossy(&bytes));
        // Process complete SSE lines; keep any partial tail in `buffer`.
        while let Some(nl) = buffer.find('\n') {
            let line: String = buffer.drain(..=nl).collect();
            let Some(data) = line.trim().strip_prefix("data:") else {
                continue;
            };
            let data = data.trim();
            if data == "[DONE]" {
                continue;
            }
            let Ok(chunk) = serde_json::from_str::<StreamChunk>(data) else {
                continue;
            };
            if let Some(delta) = chunk.choices.into_iter().next().and_then(|c| c.delta.content) {
                if !delta.is_empty() {
                    full.push_str(&delta);
                    on_progress(&full);
                    if full.len() >= last_check + REPEAT_CHECK_EVERY {
                        last_check = full.len();
                        if looks_repetitive(&full) {
                            aborted = true;
                            break 'outer;
                        }
                    }
                }
            }
            if chunk.usage.is_some() {
                usage = chunk.usage;
            }
        }
    }

    if aborted {
        // Keep whatever coherent text preceded the loop; drop the repeating tail.
        full = full.trim_end().to_string();
    }

    let elapsed = started.elapsed().as_secs_f64();
    // Estimate tokens if the server didn't report usage, so the speed readout
    // still works.
    let usage = usage.or(Some(Usage {
        completion_tokens: Some((full.chars().count() as u64) / 4),
        prompt_tps: None,
        generation_tps: None,
        completion_tps: None,
    }));
    Ok((full, derive_metrics(usage.as_ref(), elapsed)))
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

/// Fold newer messages into a running conversation summary, so context can stay
/// bounded (recent window + this summary of the tail).
pub async fn summarize_conversation(
    cfg: &BotConfig,
    previous: &str,
    new_messages: &str,
) -> Result<String, String> {
    let prev = if previous.trim().is_empty() { "(none yet)" } else { previous };
    let messages = vec![
        ChatMessage::system(
            "You maintain a running summary of a Discord conversation so an assistant can recall \
             earlier context that has scrolled out of view. Given the previous summary and newer \
             messages, produce an updated, concise summary (a few short paragraphs at most) that \
             captures key facts, questions, decisions, names, and unresolved threads. Drop small \
             talk. Output only the summary.",
        ),
        ChatMessage::user(format!(
            "Previous summary:\n{prev}\n\nNewer messages:\n{new_messages}\n\nUpdated summary:"
        )),
    ];
    let (text, _) = request(cfg, messages, Some(600), 0.3).await?;
    Ok(text.trim().to_string())
}

/// Intent gate for archiving an attachment: should the bot grab this file? Yes
/// if the user explicitly asked to save it (visible in `context`) or it matches
/// the bot's standing rules (`guidance`). Fail closed (don't archive on error).
pub async fn should_archive(
    cfg: &BotConfig,
    guidance: &str,
    context: &str,
    filename: &str,
    content_type: &str,
) -> bool {
    let rules = if guidance.trim().is_empty() {
        "(none)".to_string()
    } else {
        guidance.to_string()
    };
    let messages = vec![
        ChatMessage::system(
            "You decide whether to archive a file attachment from a Discord conversation into a \
             knowledge base. Answer 'yes' if the user explicitly asked to save/keep it, or if it \
             matches the standing archiving rules. Answer 'no' for casual images, memes, or files \
             not asked for and not covered by a rule. Reply with exactly 'yes' or 'no'.",
        ),
        ChatMessage::user(format!(
            "Standing archiving rules:\n{rules}\n\nMessage context:\n{context}\n\n\
             Attachment: name=\"{filename}\" type={content_type}\n\n\
             Archive this attachment? Answer yes or no."
        )),
    ];
    match request(cfg, messages, Some(3), 0.0).await {
        Ok((text, _)) => text.trim().to_lowercase().starts_with("yes"),
        Err(_) => false,
    }
}

// --- Embeddings -------------------------------------------------------------

#[derive(Serialize)]
struct EmbeddingRequest<'a> {
    model: &'a str,
    input: &'a [String],
}

#[derive(Deserialize)]
struct EmbeddingResponse {
    #[serde(default)]
    data: Vec<EmbeddingData>,
}

#[derive(Deserialize)]
struct EmbeddingData {
    #[serde(default)]
    embedding: Vec<f32>,
}

/// Embed a batch of texts via the OpenAI-compatible `/embeddings` endpoint on the
/// bot's model server. Returns one vector per input (order preserved).
pub async fn embed(cfg: &BotConfig, inputs: &[String]) -> Result<Vec<Vec<f32>>, String> {
    if inputs.is_empty() {
        return Ok(Vec::new());
    }
    let url = format!("{}/embeddings", cfg.model.base_url.trim_end_matches('/'));
    let body = EmbeddingRequest { model: &cfg.model.embedding_model, input: inputs };

    let mut req = reqwest::Client::new()
        .post(url)
        .timeout(Duration::from_secs(REQUEST_TIMEOUT_SECS))
        .json(&body);
    if !cfg.model.api_key.trim().is_empty() {
        req = req.bearer_auth(cfg.model.api_key.trim());
    }

    let resp = req.send().await.map_err(|e| format!("request failed: {e}"))?;
    if !resp.status().is_success() {
        let status = resp.status();
        let detail = resp.text().await.unwrap_or_default();
        return Err(format!("embeddings server returned {status}: {}", detail.trim()));
    }

    let parsed: EmbeddingResponse = resp.json().await.map_err(|e| format!("bad response: {e}"))?;
    if parsed.data.len() != inputs.len() {
        return Err(format!(
            "embeddings count mismatch: got {}, expected {}",
            parsed.data.len(),
            inputs.len()
        ));
    }
    Ok(parsed.data.into_iter().map(|d| d.embedding).collect())
}

/// Pick the best-fitting folder for a file among `folders`, guided by the user's
/// standing rules. Returns an exact folder name from the list, or `None` for
/// "no clear match" (file it at the root). Fail-open to `None`.
pub async fn pick_folder(
    cfg: &BotConfig,
    guidance: &str,
    context: &str,
    filename: &str,
    folders: &[String],
) -> Option<String> {
    if folders.is_empty() {
        return None;
    }
    let list = folders.iter().map(|f| format!("- {f}")).collect::<Vec<_>>().join("\n");
    let rules = if guidance.trim().is_empty() { "(none)".to_string() } else { guidance.to_string() };
    let messages = vec![
        ChatMessage::system(
            "You file an attachment into the best-matching folder. Reply with EXACTLY one folder \
             name from the list, or 'ROOT' if none clearly fits. Output only that — no other text.",
        ),
        ChatMessage::user(format!(
            "Folders:\n{list}\n\nStanding filing rules:\n{rules}\n\nMessage context:\n{context}\n\n\
             Attachment: \"{filename}\"\n\nBest folder (exact name, or ROOT):"
        )),
    ];
    match request(cfg, messages, Some(20), 0.0).await {
        Ok((text, _)) => {
            let answer = text.trim().trim_matches('"').trim();
            folders.iter().find(|f| f.eq_ignore_ascii_case(answer)).cloned()
        }
        Err(_) => None,
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
        model: &cfg.model.model_name,
        messages: &messages,
        temperature,
        stream: false,
        max_tokens,
        stream_options: None,
        frequency_penalty: None,
    };

    let mut req = client
        .post(cfg.chat_url())
        .timeout(Duration::from_secs(REQUEST_TIMEOUT_SECS))
        .json(&body);
    if !cfg.model.api_key.trim().is_empty() {
        req = req.bearer_auth(cfg.model.api_key.trim());
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

/// True if the tail of `s` is a short cycle repeated — a degenerate loop
/// (whitespace, a single char, or a short phrase). Catches exact repetition.
fn looks_repetitive(s: &str) -> bool {
    let chars: Vec<char> = s.chars().collect();
    if chars.len() < REPEAT_WINDOW {
        return false;
    }
    let tail = &chars[chars.len() - REPEAT_WINDOW..];
    for period in 1..=48 {
        if REPEAT_WINDOW / period < 4 {
            break; // need at least 4 repeats of the cycle to call it a loop
        }
        if tail.iter().enumerate().all(|(i, &c)| c == tail[i % period]) {
            return true;
        }
    }
    false
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
