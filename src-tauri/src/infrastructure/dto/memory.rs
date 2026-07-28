//! Memory boundary formats, kept out of the domain:
//! - `MemoryDto`: the JSON persisted in the store and returned to the frontend.
//! - `RULE:`/`NOTE:` lines: the wire format for the model consolidation round-trip.

use serde::{Deserialize, Serialize};

use crate::domain::memory::{Memory, MemoryKind};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct MemoryDto {
    pub id: String,
    /// `"rule"` or `"note"`.
    pub kind: String,
    pub text: String,
    pub created: u64,
}

impl Default for MemoryDto {
    fn default() -> Self {
        Self {
            id: String::new(),
            kind: "note".into(),
            text: String::new(),
            created: 0,
        }
    }
}

impl MemoryDto {
    pub fn from_domain(m: &Memory) -> Self {
        Self {
            id: m.id.clone(),
            kind: m.kind.as_str().to_string(),
            text: m.text.clone(),
            created: m.created,
        }
    }

    pub fn into_domain(self) -> Memory {
        Memory {
            id: self.id,
            kind: MemoryKind::parse(&self.kind),
            text: self.text,
            created: self.created,
        }
    }
}

/// Render memories as `RULE:`/`NOTE:` lines for the consolidation prompt.
pub fn render_lines(memories: &[Memory]) -> String {
    memories
        .iter()
        .map(|m| format!("{}: {}", tag(m.kind), m.text.trim()))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Parse `RULE:`/`NOTE:` lines from a model reply back into `(kind, text)` pairs;
/// the adapter mints ids/timestamps. Non-matching or empty lines are dropped.
pub fn parse_lines(text: &str) -> Vec<(MemoryKind, String)> {
    text.lines()
        .filter_map(|line| {
            let line = line.trim();
            let (kind, rest) = if let Some(rest) = line.strip_prefix("RULE:") {
                (MemoryKind::Rule, rest)
            } else if let Some(rest) = line.strip_prefix("NOTE:") {
                (MemoryKind::Note, rest)
            } else {
                return None;
            };
            let text = rest.trim();
            if text.is_empty() {
                None
            } else {
                Some((kind, text.to_string()))
            }
        })
        .collect()
}

fn tag(kind: MemoryKind) -> &'static str {
    match kind {
        MemoryKind::Rule => "RULE",
        MemoryKind::Note => "NOTE",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_parse_roundtrip() {
        let mems = vec![
            Memory {
                id: "a".into(),
                kind: MemoryKind::Rule,
                text: "always X".into(),
                created: 0,
            },
            Memory {
                id: "b".into(),
                kind: MemoryKind::Note,
                text: "fact Y".into(),
                created: 5,
            },
        ];
        let parsed = parse_lines(&render_lines(&mems));
        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed[0], (MemoryKind::Rule, "always X".to_string()));
        assert_eq!(parsed[1].0, MemoryKind::Note);
    }
}
