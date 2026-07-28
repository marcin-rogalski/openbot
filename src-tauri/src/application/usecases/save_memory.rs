//! Usecase: save a memory, keeping the bot within its budget. Appends the entry,
//! and if that breaches a cap, consolidates via the model (port), falling back to
//! FIFO eviction (domain) so memory is always bounded.

use std::sync::Arc;

use crate::application::ports::memory::{MemoryConsolidator, MemoryStore};
use crate::domain::memory::{self, MemoryKind};

/// What a save did, for the caller to report/log.
pub struct SaveOutcome {
    pub kind: MemoryKind,
    /// `Some((before, after))` when a consolidation/eviction pass ran.
    pub consolidated: Option<(usize, usize)>,
}

pub struct SaveMemory {
    store: Arc<dyn MemoryStore>,
    consolidator: Arc<dyn MemoryConsolidator>,
    max_notes: u32,
    char_budget: u32,
}

impl SaveMemory {
    pub fn new(
        store: Arc<dyn MemoryStore>,
        consolidator: Arc<dyn MemoryConsolidator>,
        max_notes: u32,
        char_budget: u32,
    ) -> Self {
        Self {
            store,
            consolidator,
            max_notes,
            char_budget,
        }
    }

    pub async fn run(&self, kind: MemoryKind, raw_text: &str) -> Result<SaveOutcome, String> {
        let text = memory::sanitize_text(raw_text).ok_or("empty memory")?;

        let mut memories = self.store.load();
        memories.push(self.store.mint(kind, text));
        self.store.store_all(&memories);

        let max_notes = self.max_notes as usize;
        let char_budget = self.char_budget as usize;
        if !memory::over_budget(&memories, max_notes, char_budget) {
            return Ok(SaveOutcome {
                kind,
                consolidated: None,
            });
        }

        let before = memories.len();
        let next = match self
            .consolidator
            .consolidate(&memories, self.max_notes)
            .await
        {
            Some(next) if !next.is_empty() => next,
            _ => memory::fifo_trim(memories, max_notes, char_budget),
        };
        let after = next.len();
        self.store.store_all(&next);
        Ok(SaveOutcome {
            kind,
            consolidated: Some((before, after)),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::memory::Memory;
    use async_trait::async_trait;
    use std::sync::Mutex;

    #[derive(Default)]
    struct FakeStore {
        saved: Mutex<Vec<Memory>>,
        seq: Mutex<u64>,
    }

    impl MemoryStore for FakeStore {
        fn load(&self) -> Vec<Memory> {
            self.saved.lock().unwrap().clone()
        }
        fn store_all(&self, memories: &[Memory]) {
            *self.saved.lock().unwrap() = memories.to_vec();
        }
        fn mint(&self, kind: MemoryKind, text: String) -> Memory {
            let mut seq = self.seq.lock().unwrap();
            *seq += 1;
            Memory {
                id: format!("m{seq}"),
                kind,
                text,
                created: *seq,
            }
        }
    }

    struct NoConsolidator;

    #[async_trait]
    impl MemoryConsolidator for NoConsolidator {
        async fn consolidate(&self, _m: &[Memory], _n: u32) -> Option<Vec<Memory>> {
            None
        }
    }

    #[tokio::test]
    async fn rejects_empty_text() {
        let uc = SaveMemory::new(
            Arc::new(FakeStore::default()),
            Arc::new(NoConsolidator),
            10,
            10_000,
        );
        assert!(uc.run(MemoryKind::Note, "   ").await.is_err());
    }

    #[tokio::test]
    async fn evicts_via_fifo_when_over_note_cap_and_consolidation_fails() {
        let store = Arc::new(FakeStore::default());
        let uc = SaveMemory::new(store.clone(), Arc::new(NoConsolidator), 1, 10_000);
        uc.run(MemoryKind::Note, "first").await.unwrap();
        let out = uc.run(MemoryKind::Note, "second").await.unwrap();
        assert_eq!(out.consolidated, Some((2, 1)));
        let saved = store.load();
        assert_eq!(saved.len(), 1);
        assert_eq!(saved[0].text, "second");
    }
}
