//! Driving adapter for per-bot memory. Two inbound surfaces call in here:
//! - the model's `memory` tool (save/delete), via the Discord tool loop;
//! - the desktop UI, via tauri commands (list/delete/clear).
//!
//! It also renders memories into the prompt text the Discord adapter injects
//! (system section) and the guidance the attachment gate uses (rules only).

use tauri::AppHandle;

use crate::compose::{driven, driving};
use crate::domain::memory::{Memory, MemoryKind};
use crate::infrastructure::bot;
use crate::infrastructure::config;
use crate::infrastructure::dto::memory::MemoryDto;

// --- Tool ops (Discord tool loop) -------------------------------------------

/// Append a memory and enforce the bot's budget. Returns a short tool-result
/// string.
pub async fn save(app: &AppHandle, bot_id: &str, kind: &str, text: &str) -> String {
    let kind = MemoryKind::parse(kind);
    let Some(bot) = config::load_bot(app, bot_id) else {
        // No bot config: best-effort append without budget enforcement.
        let store = driven::memory_store(app, bot_id);
        let Some(text) = crate::domain::memory::sanitize_text(text) else {
            return "error: empty memory".to_string();
        };
        let mut memories = store.load();
        memories.push(store.mint(kind, text));
        store.store_all(&memories);
        return format!("saved {}", kind.as_str());
    };

    match driving::save_memory(app, &bot).run(kind, text).await {
        Ok(outcome) => {
            if let Some((before, after)) = outcome.consolidated {
                bot::emit_log(
                    app,
                    bot_id,
                    format!("memory: consolidated {before} → {after} entries"),
                );
            }
            format!("saved {}", outcome.kind.as_str())
        }
        Err(e) => format!("error: {e}"),
    }
}

/// Delete a memory by id (tool loop).
pub fn delete(app: &AppHandle, bot_id: &str, id: &str) {
    let store = driven::memory_store(app, bot_id);
    let mut memories = store.load();
    memories.retain(|m| m.id != id);
    store.store_all(&memories);
}

// --- Read helper (prompt building, attachment gate) -------------------------

pub fn load(app: &AppHandle, bot_id: &str) -> Vec<Memory> {
    driven::memory_store(app, bot_id).load()
}

// --- Prompt / gate presentation ---------------------------------------------

/// The memory block appended to the system prompt. Empty if no memories.
pub fn system_section(memories: &[Memory]) -> String {
    if memories.is_empty() {
        return String::new();
    }
    let rules: Vec<&Memory> = memories.iter().filter(|m| m.is_rule()).collect();
    let notes: Vec<&Memory> = memories.iter().filter(|m| !m.is_rule()).collect();

    let mut out = String::from("\n\n## Memory (things you remember about this server/users)\n");
    if !rules.is_empty() {
        out.push_str("Rules — always follow:\n");
        for r in rules {
            out.push_str(&format!("- {}\n", r.text.trim()));
        }
    }
    if !notes.is_empty() {
        out.push_str("Notes:\n");
        for n in notes {
            out.push_str(&format!("- {}\n", n.text.trim()));
        }
    }
    out
}

/// Standing rules rendered as guidance for the attachment gate. Rules are the
/// directives ("archive all PDFs from #research"); notes are facts, so omitted.
pub fn guidance(memories: &[Memory]) -> String {
    memories
        .iter()
        .filter(|m| m.is_rule())
        .map(|m| format!("- {}", m.text.trim()))
        .collect::<Vec<_>>()
        .join("\n")
}

// --- Tauri commands (desktop UI) --------------------------------------------

#[tauri::command]
pub fn get_memories(app: AppHandle, bot_id: String) -> Vec<MemoryDto> {
    load(&app, &bot_id)
        .iter()
        .map(MemoryDto::from_domain)
        .collect()
}

#[tauri::command]
pub fn delete_memory(app: AppHandle, bot_id: String, id: String) {
    delete(&app, &bot_id, &id);
}

#[tauri::command]
pub fn clear_memories(app: AppHandle, bot_id: String) {
    driven::memory_store(&app, &bot_id).store_all(&[]);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rule(text: &str) -> Memory {
        Memory {
            id: "r".into(),
            kind: MemoryKind::Rule,
            text: text.into(),
            created: 0,
        }
    }
    fn note(text: &str) -> Memory {
        Memory {
            id: "n".into(),
            kind: MemoryKind::Note,
            text: text.into(),
            created: 1,
        }
    }

    #[test]
    fn guidance_includes_only_rules() {
        let g = guidance(&[rule("archive PDFs"), note("fact")]);
        assert!(g.contains("archive PDFs"));
        assert!(!g.contains("fact"));
    }

    #[test]
    fn system_section_empty_when_no_memories() {
        assert!(system_section(&[]).is_empty());
    }
}
