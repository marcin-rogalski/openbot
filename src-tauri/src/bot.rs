//! Bot lifecycle + activity stream.
//!
//! [`BotManager`] is the single source of truth for whether the bot is running.
//! Both the window (via commands) and the tray menu funnel through
//! [`set_running`], which starts/stops the Discord client, keeps the tray menu
//! items in sync, and emits [`STATUS_EVENT`] so every surface reacts to the same
//! change. The actual Discord + model work lives in [`crate::discord`].

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::Serialize;
use serde_json::{json, Value};
use serenity::all::ShardManager;
use tauri::async_runtime::JoinHandle;
use tauri::menu::MenuItem;
use tauri::{AppHandle, Emitter, Manager, State, Wry};
use tokio::sync::oneshot;

use crate::config::BotConfig;

/// Emitted whenever the running state flips: `{ running: bool }`.
pub const STATUS_EVENT: &str = "bot://status";
/// Emitted for each piece of bot activity shown in the chat preview.
pub const ACTIVITY_EVENT: &str = "bot://activity";
/// Emitted with throughput numbers for the status bar (prefill / inference).
pub const METRICS_EVENT: &str = "bot://metrics";
/// Emitted to request approval for a tool call: `{ id, tool, args }`.
pub const TOOL_APPROVAL_EVENT: &str = "bot://tool-approval";
/// Emitted when an approval is resolved or times out, so the UI drops its card.
pub const TOOL_APPROVAL_RESOLVED_EVENT: &str = "bot://tool-approval-resolved";

const APPROVAL_TIMEOUT_SECS: u64 = 120;

#[derive(Clone, Serialize)]
pub struct BotStatus {
    pub running: bool,
}

/// One entry in the read-only activity feed. `kind` drives how the UI renders
/// it; the shape is stable so the frontend never changes as sources evolve.
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ActivityEvent {
    pub id: String,
    pub ts: u64,
    pub kind: ActivityKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub channel: Option<String>,
    pub content: String,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ActivityKind {
    Message,
    ModelCall,
    // Reserved for MCP tool calls (next slice); already rendered by the UI.
    #[allow(dead_code)]
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

#[derive(Default)]
struct Inner {
    running: bool,
    shard_manager: Option<Arc<ShardManager>>,
    task: Option<JoinHandle<()>>,
    start_item: Option<MenuItem<Wry>>,
    stop_item: Option<MenuItem<Wry>>,
    /// In-flight tool-approval requests, keyed by request id.
    approvals: HashMap<String, oneshot::Sender<Decision>>,
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

/// Managed Tauri state. Cheap to construct; lives for the app's lifetime.
#[derive(Default)]
pub struct BotManager {
    inner: Mutex<Inner>,
}

impl BotManager {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register the tray's Start/Stop menu items so [`set_running`] can keep
    /// them enabled/disabled. Called once, from the tray setup.
    pub fn set_tray_items(&self, start: MenuItem<Wry>, stop: MenuItem<Wry>) {
        let mut inner = self.inner.lock().unwrap();
        let _ = start.set_enabled(!inner.running);
        let _ = stop.set_enabled(inner.running);
        inner.start_item = Some(start);
        inner.stop_item = Some(stop);
    }

    /// Called by the Discord supervisor once the client exists, so a later stop
    /// can shut the gateway down cleanly.
    pub fn set_shard_manager(&self, manager: Arc<ShardManager>) {
        self.inner.lock().unwrap().shard_manager = Some(manager);
    }
}

fn sync_tray(inner: &Inner, running: bool) {
    if let Some(item) = &inner.start_item {
        let _ = item.set_enabled(!running);
    }
    if let Some(item) = &inner.stop_item {
        let _ = item.set_enabled(running);
    }
}

/// Flip the running state. Idempotent. This is the only place run-state changes,
/// so the window and tray can never disagree.
pub fn set_running(app: &AppHandle, running: bool) {
    if running {
        start(app);
    } else {
        stop(app);
    }
}

fn start(app: &AppHandle) {
    let manager = app.state::<BotManager>();
    if manager.inner.lock().unwrap().running {
        return;
    }

    // Starting requires a usable config; refuse (and say why) otherwise.
    let config = crate::config::load(app);
    if !config.is_ready() {
        emit_log(
            app,
            "Set the Discord token, model URL, and model name in Settings, then Start.",
        );
        return;
    }

    {
        let mut inner = manager.inner.lock().unwrap();
        inner.running = true;
        sync_tray(&inner, true);
    }
    let _ = app.emit(STATUS_EVENT, BotStatus { running: true });

    let app_for_task = app.clone();
    let task = tauri::async_runtime::spawn(async move {
        crate::discord::run(app_for_task, config).await;
    });
    manager.inner.lock().unwrap().task = Some(task);
}

fn stop(app: &AppHandle) {
    let (shard_manager, task) = begin_stop(app);
    if shard_manager.is_none() && task.is_none() {
        return; // was already stopped
    }
    tauri::async_runtime::spawn(finish_stop(shard_manager, task));
}

/// Flip to stopped and hand back whatever needs shutting down. Returns
/// `(None, None)` if it was already stopped.
fn begin_stop(app: &AppHandle) -> (Option<Arc<ShardManager>>, Option<JoinHandle<()>>) {
    let manager = app.state::<BotManager>();
    let mut inner = manager.inner.lock().unwrap();
    if !inner.running {
        return (None, None);
    }
    inner.running = false;
    sync_tray(&inner, false);
    let handles = (inner.shard_manager.take(), inner.task.take());
    drop(inner);
    let _ = app.emit(STATUS_EVENT, BotStatus { running: false });
    handles
}

async fn finish_stop(shard_manager: Option<Arc<ShardManager>>, task: Option<JoinHandle<()>>) {
    if let Some(shard_manager) = shard_manager {
        shard_manager.shutdown_all().await;
    } else if let Some(task) = task {
        // Still connecting — no gateway to shut down yet.
        task.abort();
    }
}

// --- Event emission helpers (used by discord.rs) ----------------------------

static SEQ: AtomicU64 = AtomicU64::new(0);

pub fn emit_activity(
    app: &AppHandle,
    kind: ActivityKind,
    author: Option<String>,
    channel: Option<String>,
    content: impl Into<String>,
) {
    let event = ActivityEvent {
        id: format!("ev-{}", SEQ.fetch_add(1, Ordering::Relaxed)),
        ts: now_ms(),
        kind,
        author,
        channel,
        content: content.into(),
    };
    let _ = app.emit(ACTIVITY_EVENT, event);
}

pub fn emit_log(app: &AppHandle, content: impl Into<String>) {
    emit_activity(app, ActivityKind::Log, None, None, content);
}

pub fn emit_metrics(app: &AppHandle, metrics: Metrics) {
    let _ = app.emit(METRICS_EVENT, metrics);
}

// --- Tool policy + approval -------------------------------------------------

/// Resolve a tool's policy: an explicit config entry wins; otherwise write tools
/// default to `ask` and read tools to `allow`.
pub fn policy_for(cfg: &BotConfig, tool: &str, is_write: bool) -> Policy {
    match cfg.tool_policies.get(tool).map(String::as_str) {
        Some("allow") => Policy::Allow,
        Some("deny") => Policy::Deny,
        Some("ask") => Policy::Ask,
        _ if is_write => Policy::Ask,
        _ => Policy::Allow,
    }
}

/// Ask the UI to approve a tool call and wait for the answer (or deny on a 60s
/// timeout / if no one responds).
pub async fn request_approval(app: &AppHandle, tool: &str, args: &Value) -> Decision {
    let id = format!("ap-{}", SEQ.fetch_add(1, Ordering::Relaxed));
    let (tx, rx) = oneshot::channel();
    app.state::<BotManager>()
        .inner
        .lock()
        .unwrap()
        .approvals
        .insert(id.clone(), tx);
    let _ = app.emit(TOOL_APPROVAL_EVENT, json!({ "id": id, "tool": tool, "args": args }));

    let decision = match tokio::time::timeout(Duration::from_secs(APPROVAL_TIMEOUT_SECS), rx).await
    {
        Ok(Ok(decision)) => decision,
        _ => {
            app.state::<BotManager>().inner.lock().unwrap().approvals.remove(&id);
            Decision::Deny
        }
    };
    // Tell the UI to drop the card (handles the timeout case too).
    let _ = app.emit(TOOL_APPROVAL_RESOLVED_EVENT, json!({ "id": id }));
    decision
}

#[tauri::command]
pub fn resolve_tool_approval(app: AppHandle, id: String, decision: String) {
    let decision = match decision.as_str() {
        "approve" => Decision::Approve,
        "always_allow" => Decision::AlwaysAllow,
        "always_deny" => Decision::AlwaysDeny,
        _ => Decision::Deny,
    };
    if let Some(tx) = app.state::<BotManager>().inner.lock().unwrap().approvals.remove(&id) {
        let _ = tx.send(decision);
    }
    let _ = app.emit(TOOL_APPROVAL_RESOLVED_EVENT, json!({ "id": id }));
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

#[tauri::command]
pub fn start_bot(app: AppHandle) {
    set_running(&app, true);
}

#[tauri::command]
pub fn stop_bot(app: AppHandle) {
    set_running(&app, false);
}

#[tauri::command]
pub fn get_bot_status(manager: State<'_, BotManager>) -> BotStatus {
    let running = manager.inner.lock().unwrap().running;
    BotStatus { running }
}

/// Restart the bot so saved Settings take effect — but only if it is currently
/// running (saving while stopped shouldn't start it). Awaits a clean gateway
/// shutdown before reconnecting so there's no overlap/double-reply.
#[tauri::command]
pub async fn restart_bot(app: AppHandle) {
    let (shard_manager, task) = begin_stop(&app);
    let was_running = shard_manager.is_some() || task.is_some();
    finish_stop(shard_manager, task).await;
    if was_running {
        start(&app);
    }
}
