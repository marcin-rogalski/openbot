//! Bot lifecycle + activity stream.
//!
//! [`BotManager`] is the single source of truth for whether the bot is running.
//! Both the window (via commands) and the tray menu funnel through
//! [`set_running`], which spawns/aborts the work, keeps the tray menu items in
//! sync, and emits [`STATUS_EVENT`] so every surface reacts to the same change.
//!
//! For this milestone the "work" is a mock loop emitting canned
//! [`ActivityEvent`]s. The real bot (serenity + rmcp + MLX) will replace
//! [`mock_loop`] while keeping the same events, so the UI needs no changes.

use std::sync::Mutex;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::Serialize;
use tauri::async_runtime::JoinHandle;
use tauri::menu::MenuItem;
use tauri::{AppHandle, Emitter, Manager, State, Wry};

/// Emitted whenever the running state flips: `{ running: bool }`.
pub const STATUS_EVENT: &str = "bot://status";
/// Emitted for each piece of bot activity shown in the chat preview.
pub const ACTIVITY_EVENT: &str = "bot://activity";
/// Emitted with throughput numbers for the status bar (prefill / inference).
pub const METRICS_EVENT: &str = "bot://metrics";

#[derive(Clone, Serialize)]
pub struct BotStatus {
    pub running: bool,
}

/// One entry in the read-only activity feed. `kind` drives how the UI renders
/// it; the shape is intentionally stable so the real bot can reuse it verbatim.
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ActivityEvent {
    pub id: String,
    pub ts: u64,
    pub kind: ActivityKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
    pub content: String,
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

/// Model throughput in tokens/second. `None` means "not measured yet".
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Metrics {
    pub prefill_tps: Option<f64>,
    pub inference_tps: Option<f64>,
}

#[derive(Default)]
struct Inner {
    running: bool,
    task: Option<JoinHandle<()>>,
    start_item: Option<MenuItem<Wry>>,
    stop_item: Option<MenuItem<Wry>>,
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
        let inner = self.inner.lock().unwrap();
        let _ = start.set_enabled(!inner.running);
        let _ = stop.set_enabled(inner.running);
        drop(inner);
        let mut inner = self.inner.lock().unwrap();
        inner.start_item = Some(start);
        inner.stop_item = Some(stop);
    }
}

/// Flip the running state. Idempotent: a no-op if already in `running`. This is
/// the only place run-state changes, so the window and tray can never disagree.
pub fn set_running(app: &AppHandle, running: bool) {
    let manager = app.state::<BotManager>();
    let mut inner = manager.inner.lock().unwrap();
    if inner.running == running {
        return;
    }
    inner.running = running;

    if let Some(task) = inner.task.take() {
        task.abort();
    }
    if running {
        let handle = app.clone();
        inner.task = Some(tauri::async_runtime::spawn(mock_loop(handle)));
    }

    if let Some(item) = &inner.start_item {
        let _ = item.set_enabled(!running);
    }
    if let Some(item) = &inner.stop_item {
        let _ = item.set_enabled(running);
    }
    drop(inner);

    let _ = app.emit(STATUS_EVENT, BotStatus { running });
}

/// Stand-in for the real bot: replays a canned Discord-style exchange on a loop
/// until the task is aborted.
async fn mock_loop(app: AppHandle) {
    let script = [
        (ActivityKind::Message, Some("alice"), "hey bot, what's the weather in Wrocław?"),
        (ActivityKind::ModelCall, None, "→ qwen2.5-7b (temp 0.7)"),
        (ActivityKind::ToolCall, None, "get_weather { city: \"Wrocław\" }"),
        (ActivityKind::Log, None, "get_weather → 200 OK (142ms)"),
        (ActivityKind::Reply, Some("openbot"), "It's 22°C and sunny in Wrocław right now."),
    ];

    let mut seq: u64 = 0;
    loop {
        for (kind, author, content) in &script {
            // Fake plausible, slightly-varying throughput when the model runs.
            if matches!(kind, ActivityKind::ModelCall) {
                let _ = app.emit(
                    METRICS_EVENT,
                    Metrics {
                        prefill_tps: Some(220.0 + (seq % 9) as f64 * 11.0),
                        inference_tps: Some(32.0 + (seq % 6) as f64 * 3.0),
                    },
                );
            }
            let event = ActivityEvent {
                id: format!("mock-{seq}"),
                ts: now_ms(),
                kind: kind.clone(),
                author: author.map(str::to_string),
                content: content.to_string(),
            };
            let _ = app.emit(ACTIVITY_EVENT, event);
            seq += 1;
            tokio::time::sleep(Duration::from_millis(900)).await;
        }
        tokio::time::sleep(Duration::from_millis(1500)).await;
    }
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
