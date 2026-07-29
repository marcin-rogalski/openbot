//! The per-bot memory tool: `save` + `delete` ops, executed via the memory
//! driving adapter.

use serde_json::Value;
use tauri::AppHandle;

#[derive(Clone, Copy)]
pub enum MemoryOp {
    Save,
    Delete,
}

impl MemoryOp {
    pub const ALL: [MemoryOp; 2] = [MemoryOp::Save, MemoryOp::Delete];

    pub fn suffix(self) -> &'static str {
        match self {
            MemoryOp::Save => "save",
            MemoryOp::Delete => "delete",
        }
    }

    pub fn call_name(self) -> &'static str {
        match self {
            MemoryOp::Save => "memory_save",
            MemoryOp::Delete => "memory_delete",
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            MemoryOp::Save => {
                "Remember a fact ('note') or a standing instruction ('rule') for \
                               future conversations."
            }
            MemoryOp::Delete => "Forget a memory by its id.",
        }
    }

    pub fn args(self) -> &'static str {
        match self {
            MemoryOp::Save => "{\"kind\": \"note\"|\"rule\", \"text\": string}",
            MemoryOp::Delete => "{\"id\": string}",
        }
    }
}

/// Execute a memory op for a bot.
pub(super) async fn execute(app: &AppHandle, bot_id: &str, op: MemoryOp, args: &Value) -> String {
    let arg = |key: &str| args.get(key).and_then(Value::as_str).unwrap_or("");
    match op {
        MemoryOp::Save => {
            crate::infrastructure::driving::memory::save(app, bot_id, arg("kind"), arg("text"))
                .await
        }
        MemoryOp::Delete => {
            crate::infrastructure::driving::memory::delete(app, bot_id, arg("id"));
            "forgotten".to_string()
        }
    }
}
