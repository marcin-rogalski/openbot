//! Multi-bot lifecycle + activity streams.
//!
//! [`BotManager`] tracks which bots are currently running (each its own serenity
//! client). Every emitted event carries the `botId` it belongs to, so the UI can
//! scope streams to the selected bot. Tool-approval requests are global (keyed by
//! request id) but also carry the requesting bot.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::Serialize;
use serde_json::{json, Value};
use serenity::all::ShardManager;
use tauri::async_runtime::JoinHandle;
use tauri::{AppHandle, Emitter, Manager, State};
use tokio::sync::oneshot;

use crate::config::{self, BotConfig};

/// `{ botId, running }` whenever a bot starts or stops.
pub const STATUS_EVENT: &str = "bot://status";
/// One activity-feed entry (carries `botId`).
pub const ACTIVITY_EVENT: &str = "bot://activity";
/// Live token stream for an in-progress model call: `{ botId, id, content }` —
/// the UI replaces the matching activity entry's content as it grows.
pub const STREAM_EVENT: &str = "bot://stream";
/// The bot's live activity label for the status bar: `{ botId, label }`, where
/// `label` is a short string while it works (inference or a tool) and `null`
/// when it goes idle.
pub const BUSY_EVENT: &str = "bot://busy";
/// Throughput numbers for the status bar (carries `botId`).
pub const METRICS_EVENT: &str = "bot://metrics";
/// Tool-approval request: `{ id, botId, tool, args }`.
pub const TOOL_APPROVAL_EVENT: &str = "bot://tool-approval";
/// Approval resolved/timed out: `{ id }` — the UI drops the card.
pub const TOOL_APPROVAL_RESOLVED_EVENT: &str = "bot://tool-approval-resolved";

const APPROVAL_TIMEOUT_SECS: u64 = 120;

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ActivityEvent {
    pub bot_id: String,
    pub id: String,
    pub ts: u64,
    pub kind: ActivityKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub channel: Option<String>,
    pub content: String,
    /// A friendly one-liner shown in non-verbose mode (tool calls). When
    /// absent, `content` is shown in both modes.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ActivityKind {
    Message,
    ModelCall,
    ToolCall,
    Reply,
    Log,
}

/// Model throughput in tokens/second. `None` means "not measured".
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Metrics {
    pub prefill_tps: Option<f64>,
    pub inference_tps: Option<f64>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct StatusEvent<'a> {
    bot_id: &'a str,
    running: bool,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct MetricsEvent<'a> {
    bot_id: &'a str,
    prefill_tps: Option<f64>,
    inference_tps: Option<f64>,
}

/// Per-running-bot handles used to shut it down.
#[derive(Default)]
struct BotRuntime {
    /// Identifies this specific client instance; the gateway handler ignores
    /// messages once it's no longer the current epoch for its bot (so a
    /// still-disconnecting old client can't double-process).
    epoch: u64,
    shard_manager: Option<Arc<ShardManager>>,
    task: Option<JoinHandle<()>>,
}

#[derive(Default)]
struct Inner {
    /// Only currently-running bots are present, keyed by bot id.
    bots: HashMap<String, BotRuntime>,
    /// In-flight tool-approval requests, keyed by request id.
    approvals: HashMap<String, oneshot::Sender<Decision>>,
}

#[derive(Default)]
pub struct BotManager {
    inner: Mutex<Inner>,
}

impl BotManager {
    pub fn new() -> Self {
        Self::default()
    }

    /// Called by a bot's supervisor once its client exists, so a later stop can
    /// shut the gateway down cleanly.
    pub fn set_shard_manager(&self, bot_id: &str, manager: Arc<ShardManager>) {
        if let Some(runtime) = self.inner.lock().unwrap().bots.get_mut(bot_id) {
            runtime.shard_manager = Some(manager);
        }
    }
}

/// Resolved policy for a tool call.
#[derive(Clone, Copy, PartialEq)]
pub enum Policy {
    Allow,
    Ask,
    Deny,
}

/// User's answer to an approval prompt.
#[derive(Clone, Copy)]
pub enum Decision {
    Approve,
    Deny,
    AlwaysAllow,
    AlwaysDeny,
}

// --- Lifecycle --------------------------------------------------------------

/// Monotonic id for each started client, so a superseded client can tell it's
/// no longer the active one for its bot.
static EPOCH: AtomicU64 = AtomicU64::new(1);

pub fn start(app: &AppHandle, bot_id: &str) {
    let manager = app.state::<BotManager>();

    let Some(bot) = config::load_bot(app, bot_id) else {
        emit_log(app, bot_id, "Bot config not found.");
        return;
    };
    if !bot.is_ready() {
        emit_log(
            app,
            bot_id,
            "Set the Discord token and model (base URL + name) in this bot's settings, then start.",
        );
        return;
    }
    let global = config::load_global(app);

    // Atomically claim the running slot: check + insert under one lock, so two
    // near-simultaneous starts can't each spawn a client for the same bot.
    let epoch = EPOCH.fetch_add(1, Ordering::Relaxed);
    {
        let mut inner = manager.inner.lock().unwrap();
        if inner.bots.contains_key(bot_id) {
            return; // already running
        }
        inner.bots.insert(
            bot_id.to_string(),
            BotRuntime {
                epoch,
                ..Default::default()
            },
        );
    }
    let _ = app.emit(
        STATUS_EVENT,
        StatusEvent {
            bot_id,
            running: true,
        },
    );

    let app_for_task = app.clone();
    let id = bot_id.to_string();
    let task = tauri::async_runtime::spawn(async move {
        crate::discord::run(app_for_task, id, epoch, bot, global).await;
    });
    {
        let mut inner = manager.inner.lock().unwrap();
        if let Some(runtime) = inner.bots.get_mut(bot_id) {
            runtime.task = Some(task);
        }
    }
}

/// The epoch of the currently-running client for `bot_id`, or `None` if it isn't
/// running. A gateway handler compares this to its own epoch to see if it's still
/// the active client.
pub fn current_epoch(app: &AppHandle, bot_id: &str) -> Option<u64> {
    app.state::<BotManager>()
        .inner
        .lock()
        .unwrap()
        .bots
        .get(bot_id)
        .map(|r| r.epoch)
}

pub fn stop(app: &AppHandle, bot_id: &str) {
    let runtime = app
        .state::<BotManager>()
        .inner
        .lock()
        .unwrap()
        .bots
        .remove(bot_id);
    let Some(runtime) = runtime else {
        return; // wasn't running
    };
    let _ = app.emit(
        STATUS_EVENT,
        StatusEvent {
            bot_id,
            running: false,
        },
    );
    tauri::async_runtime::spawn(async move {
        if let Some(shard_manager) = runtime.shard_manager {
            shard_manager.shutdown_all().await;
        } else if let Some(task) = runtime.task {
            task.abort();
        }
    });
}

// --- Event emission helpers -------------------------------------------------

static SEQ: AtomicU64 = AtomicU64::new(0);

pub fn emit_activity(
    app: &AppHandle,
    bot_id: &str,
    kind: ActivityKind,
    author: Option<String>,
    channel: Option<String>,
    content: impl Into<String>,
) {
    let event = ActivityEvent {
        bot_id: bot_id.to_string(),
        id: format!("ev-{}", SEQ.fetch_add(1, Ordering::Relaxed)),
        ts: now_ms(),
        kind,
        author,
        channel,
        content: content.into(),
        summary: None,
    };
    let _ = app.emit(ACTIVITY_EVENT, event);
}

pub fn emit_log(app: &AppHandle, bot_id: &str, content: impl Into<String>) {
    emit_activity(app, bot_id, ActivityKind::Log, None, None, content);
}

/// Start a live "thinking" activity for a model call and return its id. The
/// content fills in via [`stream_update`] as tokens arrive.
pub fn stream_start(app: &AppHandle, bot_id: &str) -> String {
    let id = format!("ev-{}", SEQ.fetch_add(1, Ordering::Relaxed));
    let event = ActivityEvent {
        bot_id: bot_id.to_string(),
        id: id.clone(),
        ts: now_ms(),
        kind: ActivityKind::ModelCall,
        author: None,
        channel: None,
        content: String::new(),
        summary: None,
    };
    let _ = app.emit(ACTIVITY_EVENT, event);
    id
}

/// Push the latest accumulated content for a streaming model call to the UI.
pub fn stream_update(app: &AppHandle, bot_id: &str, id: &str, content: &str) {
    let _ = app.emit(
        STREAM_EVENT,
        json!({ "botId": bot_id, "id": id, "content": content }),
    );
}

/// Set the bot's live activity label for the status bar: `Some(label)` while it
/// works (inference or a tool — e.g. "Searching the web…", "Transcribing 20%"),
/// `None` when it goes idle.
pub fn emit_busy(app: &AppHandle, bot_id: &str, label: Option<&str>) {
    let _ = app.emit(BUSY_EVENT, json!({ "botId": bot_id, "label": label }));
}

/// A tool-call activity carrying both the raw detail (`content`, shown in
/// verbose mode) and a friendly `summary` (shown when folded).
pub fn emit_tool_activity(
    app: &AppHandle,
    bot_id: &str,
    content: impl Into<String>,
    summary: impl Into<String>,
) {
    let event = ActivityEvent {
        bot_id: bot_id.to_string(),
        id: format!("ev-{}", SEQ.fetch_add(1, Ordering::Relaxed)),
        ts: now_ms(),
        kind: ActivityKind::ToolCall,
        author: None,
        channel: None,
        content: content.into(),
        summary: Some(summary.into()),
    };
    let _ = app.emit(ACTIVITY_EVENT, event);
}

pub fn emit_metrics(app: &AppHandle, bot_id: &str, metrics: Metrics) {
    let _ = app.emit(
        METRICS_EVENT,
        MetricsEvent {
            bot_id,
            prefill_tps: metrics.prefill_tps,
            inference_tps: metrics.inference_tps,
        },
    );
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

// --- Tool policy + approval -------------------------------------------------

/// Resolve a policy: an explicit per-bot entry wins; otherwise write tools
/// default to `ask` and read tools to `allow`.
pub fn policy_for(bot: &BotConfig, policy_key: &str, is_write: bool) -> Policy {
    match bot.tool_policies.get(policy_key).map(String::as_str) {
        Some("allow") => Policy::Allow,
        Some("deny") => Policy::Deny,
        Some("ask") => Policy::Ask,
        _ if is_write => Policy::Ask,
        _ => Policy::Allow,
    }
}

/// Ask the UI to approve a tool call and wait for the answer (deny on timeout).
pub async fn request_approval(app: &AppHandle, bot_id: &str, tool: &str, args: &Value) -> Decision {
    let id = format!("ap-{}", SEQ.fetch_add(1, Ordering::Relaxed));
    let (tx, rx) = oneshot::channel();
    app.state::<BotManager>()
        .inner
        .lock()
        .unwrap()
        .approvals
        .insert(id.clone(), tx);
    let _ = app.emit(
        TOOL_APPROVAL_EVENT,
        json!({ "id": id, "botId": bot_id, "tool": tool, "args": args }),
    );

    let decision = match tokio::time::timeout(Duration::from_secs(APPROVAL_TIMEOUT_SECS), rx).await
    {
        Ok(Ok(decision)) => decision,
        _ => {
            app.state::<BotManager>()
                .inner
                .lock()
                .unwrap()
                .approvals
                .remove(&id);
            Decision::Deny
        }
    };
    let _ = app.emit(TOOL_APPROVAL_RESOLVED_EVENT, json!({ "id": id }));
    decision
}

// --- Commands ---------------------------------------------------------------

#[tauri::command]
pub fn start_bot(app: AppHandle, bot_id: String) {
    start(&app, &bot_id);
}

#[tauri::command]
pub fn stop_bot(app: AppHandle, bot_id: String) {
    stop(&app, &bot_id);
}

/// Restart a bot to apply saved settings — only if it's currently running.
#[tauri::command]
pub async fn restart_bot(app: AppHandle, bot_id: String) {
    let runtime = app
        .state::<BotManager>()
        .inner
        .lock()
        .unwrap()
        .bots
        .remove(&bot_id);
    let was_running = runtime.is_some();
    if let Some(runtime) = runtime {
        let _ = app.emit(
            STATUS_EVENT,
            StatusEvent {
                bot_id: &bot_id,
                running: false,
            },
        );
        if let Some(shard_manager) = runtime.shard_manager {
            shard_manager.shutdown_all().await;
        } else if let Some(task) = runtime.task {
            task.abort();
        }
    }
    if was_running {
        start(&app, &bot_id);
    }
}

#[tauri::command]
pub fn get_running_bots(manager: State<'_, BotManager>) -> Vec<String> {
    manager.inner.lock().unwrap().bots.keys().cloned().collect()
}

/// Ids of currently-running bots, for the control API.
pub fn running_ids(app: &AppHandle) -> Vec<String> {
    app.state::<BotManager>()
        .inner
        .lock()
        .unwrap()
        .bots
        .keys()
        .cloned()
        .collect()
}

#[tauri::command]
pub fn resolve_tool_approval(app: AppHandle, id: String, decision: String) {
    let decision = match decision.as_str() {
        "approve" => Decision::Approve,
        "always_allow" => Decision::AlwaysAllow,
        "always_deny" => Decision::AlwaysDeny,
        _ => Decision::Deny,
    };
    if let Some(tx) = app
        .state::<BotManager>()
        .inner
        .lock()
        .unwrap()
        .approvals
        .remove(&id)
    {
        let _ = tx.send(decision);
    }
    let _ = app.emit(TOOL_APPROVAL_RESOLVED_EVENT, json!({ "id": id }));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::BotConfig;

    fn bot_with(policies: &[(&str, &str)]) -> BotConfig {
        let mut b = BotConfig::default();
        for (k, v) in policies {
            b.tool_policies.insert((*k).to_string(), (*v).to_string());
        }
        b
    }

    #[test]
    fn default_policy_read_allow_write_ask() {
        let b = BotConfig::default();
        assert!(matches!(policy_for(&b, "x/op", false), Policy::Allow));
        assert!(matches!(policy_for(&b, "x/op", true), Policy::Ask));
    }

    #[test]
    fn explicit_policy_overrides_default() {
        let b = bot_with(&[("t/read", "deny"), ("t/write", "allow")]);
        assert!(matches!(policy_for(&b, "t/read", false), Policy::Deny));
        assert!(matches!(policy_for(&b, "t/write", true), Policy::Allow));
        assert!(matches!(policy_for(&b, "t/unset", false), Policy::Allow));
    }
}
