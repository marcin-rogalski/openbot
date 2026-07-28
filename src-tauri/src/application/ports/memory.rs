//! Ports for the memory capability. Driven adapters implement them.

use async_trait::async_trait;

use crate::domain::memory::{Memory, MemoryKind};

/// Persistence for a single bot's memories. `mint` assigns a fresh id + creation
/// timestamp (nondeterministic, hence the adapter's job — the repository owns
/// identity), so the application never touches a clock or id generator.
pub trait MemoryStore: Send + Sync {
    fn load(&self) -> Vec<Memory>;
    fn store_all(&self, memories: &[Memory]);
    fn mint(&self, kind: MemoryKind, text: String) -> Memory;
}

/// Compress a memory list into fewer, denser entries (rules preserved). Backed
/// by the bot's model. `None` on failure — the caller falls back to FIFO.
#[async_trait]
pub trait MemoryConsolidator: Send + Sync {
    async fn consolidate(&self, memories: &[Memory], max_notes: u32) -> Option<Vec<Memory>>;
}
