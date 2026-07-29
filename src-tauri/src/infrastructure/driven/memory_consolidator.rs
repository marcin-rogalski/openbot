//! Driven adapter: `MemoryConsolidator` backed by the bot's own model. Renders
//! the list to `RULE:`/`NOTE:` lines, asks the model to merge them, and parses
//! the reply back into memories (minting fresh ids/timestamps).

use async_trait::async_trait;

use crate::application::ports::memory::MemoryConsolidator;
use crate::domain::conversation::ChatMessage;
use crate::domain::memory::Memory;
use crate::infrastructure::config::{self, BotConfig};
use crate::infrastructure::driven::model;
use crate::infrastructure::dto::memory::{parse_lines, render_lines};
use crate::infrastructure::shared::time::now_ms;

pub struct ModelConsolidator {
    bot: BotConfig,
}

impl ModelConsolidator {
    pub fn new(bot: BotConfig) -> Self {
        Self { bot }
    }
}

#[async_trait]
impl MemoryConsolidator for ModelConsolidator {
    async fn consolidate(&self, memories: &[Memory], max_notes: u32) -> Option<Vec<Memory>> {
        let current = render_lines(memories);
        let messages = vec![
            ChatMessage::system(
                "You compress an assistant's long-term memory. Merge and de-duplicate the entries \
                 into as few, dense entries as possible without losing meaning. Preserve every \
                 RULE line verbatim. Keep all essential facts from NOTE lines; drop redundant or \
                 trivial ones. Output ONLY lines, each starting with exactly 'RULE: ' or 'NOTE: '. \
                 No preamble, no commentary, no blank lines.",
            ),
            ChatMessage::user(format!(
                "Current memory ({} entries), target at most {} NOTE lines plus all RULE lines:\n\n{}",
                memories.len(),
                max_notes,
                current,
            )),
        ];

        // Consolidation is a background pass — no live streaming needed.
        let (text, _) = model::chat(&self.bot, messages, &|_: &str| {}).await.ok()?;
        let parsed = parse_lines(&text);
        if parsed.is_empty() {
            return None;
        }
        let now = now_ms();
        Some(
            parsed
                .into_iter()
                .map(|(kind, text)| Memory {
                    id: config::new_id("mem"),
                    kind,
                    text,
                    created: now,
                })
                .collect(),
        )
    }
}
