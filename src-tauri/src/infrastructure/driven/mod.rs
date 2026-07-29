//! Driven (secondary) adapters — the app calls out through these; each implements
//! an application port.

pub mod embeddings;
pub mod gdrive_storage;
pub mod keenable;
pub mod knowledge_index;
pub mod memory_consolidator;
pub mod memory_store;
