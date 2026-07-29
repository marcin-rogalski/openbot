//! Driven (secondary) adapters — the app calls out through these; each implements
//! an application port.

pub mod embeddings;
pub mod gdrive_storage;
pub mod http_fetcher;
pub mod ingest_extractor;
pub mod keenable;
pub mod knowledge_index;
pub mod memory_consolidator;
pub mod memory_store;
pub mod model_archive_policy;
pub mod model_summarizer;
pub mod model_transcriber;
pub mod symphonia_codec;
