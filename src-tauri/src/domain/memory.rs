//! Per-bot long-term memory: the business representation of what a bot remembers
//! and the pure operations on it (validation, budgeting, eviction). No IO, no
//! model calls, no wire formats — those live in adapters.

/// A memory's category. Rules are protected directives ("archive all PDFs from
/// #research"); notes are evictable facts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MemoryKind {
    Note,
    Rule,
}

impl MemoryKind {
    /// Parse from the stored/tool string; anything but `"rule"` is a note.
    pub fn parse(s: &str) -> Self {
        if s == "rule" {
            Self::Rule
        } else {
            Self::Note
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Note => "note",
            Self::Rule => "rule",
        }
    }
}

/// One remembered entry.
#[derive(Clone, Debug)]
pub struct Memory {
    pub id: String,
    pub kind: MemoryKind,
    pub text: String,
    pub created: u64,
}

impl Memory {
    pub fn is_rule(&self) -> bool {
        self.kind == MemoryKind::Rule
    }
}

/// Trim and reject empty memory text. `None` = nothing worth storing.
pub fn sanitize_text(raw: &str) -> Option<String> {
    let text = raw.trim();
    if text.is_empty() {
        None
    } else {
        Some(text.to_string())
    }
}

pub fn total_chars(memories: &[Memory]) -> usize {
    memories.iter().map(|m| m.text.chars().count()).sum()
}

/// Whether the list breaches either cap: too many notes, or too many characters.
pub fn over_budget(memories: &[Memory], max_notes: usize, char_budget: usize) -> bool {
    let notes = memories.iter().filter(|m| !m.is_rule()).count();
    notes > max_notes || total_chars(memories) > char_budget
}

/// Keep all rules; drop oldest notes until under both caps.
pub fn fifo_trim(memories: Vec<Memory>, max_notes: usize, char_budget: usize) -> Vec<Memory> {
    let (rules, mut notes): (Vec<Memory>, Vec<Memory>) =
        memories.into_iter().partition(Memory::is_rule);
    // Oldest first, so removing from the front drops the oldest.
    notes.sort_by_key(|m| m.created);

    while notes.len() > max_notes {
        notes.remove(0);
    }
    let rule_chars = total_chars(&rules);
    while !notes.is_empty() && rule_chars + total_chars(&notes) > char_budget {
        notes.remove(0);
    }

    let mut out = rules;
    out.extend(notes);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn note(text: &str, created: u64) -> Memory {
        Memory {
            id: format!("n{created}"),
            kind: MemoryKind::Note,
            text: text.into(),
            created,
        }
    }
    fn rule(text: &str) -> Memory {
        Memory {
            id: "r".into(),
            kind: MemoryKind::Rule,
            text: text.into(),
            created: 0,
        }
    }

    #[test]
    fn sanitize_trims_and_rejects_empty() {
        assert_eq!(sanitize_text("  hi "), Some("hi".to_string()));
        assert_eq!(sanitize_text("   "), None);
    }

    #[test]
    fn total_chars_sums_text() {
        assert_eq!(total_chars(&[note("abc", 1), note("de", 2)]), 5);
    }

    #[test]
    fn over_budget_by_note_count() {
        assert!(over_budget(&[note("a", 1), note("b", 2)], 1, 10_000));
        assert!(!over_budget(&[note("a", 1)], 1, 10_000));
    }

    #[test]
    fn fifo_trim_keeps_rules_drops_oldest_notes() {
        let out = fifo_trim(
            vec![rule("keep me"), note("old", 1), note("new", 2)],
            1,
            10_000,
        );
        assert!(out.iter().any(|m| m.is_rule() && m.text == "keep me"));
        let notes: Vec<&Memory> = out.iter().filter(|m| !m.is_rule()).collect();
        assert_eq!(notes.len(), 1);
        assert_eq!(notes[0].text, "new");
    }
}
