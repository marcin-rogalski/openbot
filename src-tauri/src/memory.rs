//! Per-bot long-term memory. The model saves memories/rules through a tool;
//! they're persisted in the store keyed by bot id and injected into the system
//! prompt each turn (see `discord::build_messages`).
//!
//! To keep the prompt bounded, a save that pushes a bot over its budget triggers
//! an **LLM-consolidation** pass: the bot's own model merges the list into fewer,
//! denser entries (rules preserved verbatim). If that call fails or can't be
//! parsed, we fall back to FIFO eviction so memory is always bounded.

use serde::{Deserialize, Serialize};
use serde_json::json;
use tauri::AppHandle;

use crate::bot;
use crate::config::{self, BotConfig, STORE_FILE};
use crate::model::{self, ChatMessage};

use tauri_plugin_store::StoreExt;

const MEMORIES_KEY: &str = "memories";

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Memory {
    pub id: String,
    /// `"rule"` (protected, always kept) or `"note"` (evictable).
    pub kind: String,
    pub text: String,
    pub created: u64,
}

impl Default for Memory {
    fn default() -> Self {
        Self {
            id: String::new(),
            kind: "note".into(),
            text: String::new(),
            created: 0,
        }
    }
}

impl Memory {
    fn is_rule(&self) -> bool {
        self.kind == "rule"
    }
}

// --- Storage ----------------------------------------------------------------

pub fn load(app: &AppHandle, bot_id: &str) -> Vec<Memory> {
    let Ok(store) = app.store(STORE_FILE) else {
        return Vec::new();
    };
    store
        .get(MEMORIES_KEY)
        .and_then(|v| v.get(bot_id).cloned())
        .and_then(|v| serde_json::from_value(v).ok())
        .unwrap_or_default()
}

fn store_all(app: &AppHandle, bot_id: &str, memories: &[Memory]) {
    let Ok(store) = app.store(STORE_FILE) else {
        return;
    };
    let mut all = store.get(MEMORIES_KEY).unwrap_or_else(|| json!({}));
    if let Some(map) = all.as_object_mut() {
        map.insert(bot_id.to_string(), json!(memories));
    }
    store.set(MEMORIES_KEY, all);
    let _ = store.save();
}

pub fn delete(app: &AppHandle, bot_id: &str, id: &str) {
    let mut memories = load(app, bot_id);
    memories.retain(|m| m.id != id);
    store_all(app, bot_id, &memories);
}

pub fn clear(app: &AppHandle, bot_id: &str) {
    store_all(app, bot_id, &[]);
}

/// Append a memory, then enforce the bot's budget. Returns a short confirmation
/// string for the tool result.
pub async fn save(app: &AppHandle, bot_id: &str, kind: &str, text: &str) -> String {
    let text = text.trim();
    if text.is_empty() {
        return "error: empty memory".to_string();
    }
    let kind = if kind == "rule" { "rule" } else { "note" };

    let mut memories = load(app, bot_id);
    memories.push(Memory {
        id: config::new_id("mem"),
        kind: kind.into(),
        text: text.to_string(),
        created: now_ms(),
    });
    store_all(app, bot_id, &memories);

    let Some(bot) = config::load_bot(app, bot_id) else {
        return format!("saved {kind}");
    };
    if over_budget(&memories, &bot) {
        let before = memories.len();
        let after = enforce_budget(app, bot_id, &bot, memories).await;
        bot::emit_log(
            app,
            bot_id,
            format!("memory: consolidated {before} → {after} entries"),
        );
    }
    format!("saved {kind}")
}

// --- Budget enforcement -----------------------------------------------------

fn total_chars(memories: &[Memory]) -> usize {
    memories.iter().map(|m| m.text.chars().count()).sum()
}

fn over_budget(memories: &[Memory], bot: &BotConfig) -> bool {
    let notes = memories.iter().filter(|m| !m.is_rule()).count();
    notes > bot.memory_max_notes as usize || total_chars(memories) > bot.memory_char_budget as usize
}

/// Consolidate via the model; fall back to FIFO on any failure. Persists the
/// result and returns the new entry count.
async fn enforce_budget(
    app: &AppHandle,
    bot_id: &str,
    bot: &BotConfig,
    memories: Vec<Memory>,
) -> usize {
    let consolidated = consolidate(bot, &memories).await;
    let next = match consolidated {
        Some(next) if !next.is_empty() => next,
        _ => fifo_trim(memories, bot),
    };
    store_all(app, bot_id, &next);
    next.len()
}

/// Ask the bot's own model to merge the memory list into fewer, denser entries.
/// Rules must be preserved. Returns `None` on request/parse failure.
async fn consolidate(bot: &BotConfig, memories: &[Memory]) -> Option<Vec<Memory>> {
    let current = render_lines(memories);
    let messages = vec![
        ChatMessage::system(
            "You compress an assistant's long-term memory. Merge and de-duplicate the entries \
             into as few, dense entries as possible without losing meaning. Preserve every RULE \
             line verbatim. Keep all essential facts from NOTE lines; drop redundant or trivial \
             ones. Output ONLY lines, each starting with exactly 'RULE: ' or 'NOTE: '. No \
             preamble, no commentary, no blank lines.",
        ),
        ChatMessage::user(format!(
            "Current memory ({} entries), target at most {} NOTE lines plus all RULE lines:\n\n{}",
            memories.len(),
            bot.memory_max_notes,
            current,
        )),
    ];

    // Consolidation is a background pass — no live streaming needed.
    let (text, _) = model::chat(bot, messages, |_| {}).await.ok()?;
    let parsed = parse_lines(&text);
    if parsed.is_empty() {
        return None;
    }
    Some(parsed)
}

/// Keep all rules; drop oldest notes until under both caps.
fn fifo_trim(memories: Vec<Memory>, bot: &BotConfig) -> Vec<Memory> {
    let (rules, mut notes): (Vec<Memory>, Vec<Memory>) =
        memories.into_iter().partition(Memory::is_rule);
    // Oldest first, so we can pop the newest we want to keep from the back.
    notes.sort_by_key(|m| m.created);

    let max_notes = bot.memory_max_notes as usize;
    while notes.len() > max_notes {
        notes.remove(0);
    }
    let rule_chars: usize = total_chars(&rules);
    while !notes.is_empty() && rule_chars + total_chars(&notes) > bot.memory_char_budget as usize {
        notes.remove(0);
    }

    let mut out = rules;
    out.extend(notes);
    out
}

// --- Rendering / parsing ----------------------------------------------------

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

/// Render as `RULE:`/`NOTE:` lines for the consolidation prompt.
fn render_lines(memories: &[Memory]) -> String {
    memories
        .iter()
        .map(|m| {
            let tag = if m.is_rule() { "RULE" } else { "NOTE" };
            format!("{tag}: {}", m.text.trim())
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Parse `RULE:`/`NOTE:` lines back into memories.
fn parse_lines(text: &str) -> Vec<Memory> {
    let now = now_ms();
    text.lines()
        .filter_map(|line| {
            let line = line.trim();
            let (kind, rest) = if let Some(rest) = line.strip_prefix("RULE:") {
                ("rule", rest)
            } else if let Some(rest) = line.strip_prefix("NOTE:") {
                ("note", rest)
            } else {
                return None;
            };
            let text = rest.trim();
            if text.is_empty() {
                return None;
            }
            Some(Memory {
                id: config::new_id("mem"),
                kind: kind.into(),
                text: text.to_string(),
                created: now,
            })
        })
        .collect()
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

// --- Commands ---------------------------------------------------------------

#[tauri::command]
pub fn get_memories(app: AppHandle, bot_id: String) -> Vec<Memory> {
    load(&app, &bot_id)
}

#[tauri::command]
pub fn delete_memory(app: AppHandle, bot_id: String, id: String) {
    delete(&app, &bot_id, &id);
}

#[tauri::command]
pub fn clear_memories(app: AppHandle, bot_id: String) {
    clear(&app, &bot_id);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::BotConfig;

    fn note(text: &str, created: u64) -> Memory {
        Memory {
            id: format!("n{created}"),
            kind: "note".into(),
            text: text.into(),
            created,
        }
    }
    fn rule(text: &str) -> Memory {
        Memory {
            id: "r".into(),
            kind: "rule".into(),
            text: text.into(),
            created: 0,
        }
    }

    #[test]
    fn total_chars_sums_text() {
        assert_eq!(total_chars(&[note("abc", 1), note("de", 2)]), 5);
    }

    fn capped_bot() -> BotConfig {
        BotConfig {
            memory_max_notes: 1,
            memory_char_budget: 10_000,
            ..Default::default()
        }
    }

    #[test]
    fn over_budget_by_note_count() {
        let b = capped_bot();
        assert!(over_budget(&[note("a", 1), note("b", 2)], &b));
        assert!(!over_budget(&[note("a", 1)], &b));
    }

    #[test]
    fn fifo_trim_keeps_rules_drops_oldest_notes() {
        let b = capped_bot();
        let out = fifo_trim(vec![rule("keep me"), note("old", 1), note("new", 2)], &b);
        assert!(out.iter().any(|m| m.kind == "rule" && m.text == "keep me"));
        let notes: Vec<&Memory> = out.iter().filter(|m| m.kind == "note").collect();
        assert_eq!(notes.len(), 1);
        assert_eq!(notes[0].text, "new");
    }

    #[test]
    fn render_parse_roundtrip() {
        let rendered = render_lines(&[rule("always X"), note("fact Y", 5)]);
        let parsed = parse_lines(&rendered);
        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed[0].kind, "rule");
        assert_eq!(parsed[0].text, "always X");
        assert_eq!(parsed[1].kind, "note");
    }

    #[test]
    fn guidance_includes_only_rules() {
        let g = guidance(&[rule("archive PDFs"), note("fact", 1)]);
        assert!(g.contains("archive PDFs"));
        assert!(!g.contains("fact"));
    }
}
