//! Knowledge-base domain: the business representation of an indexed source and a
//! retrieved passage. No SQL, no embeddings maths — those live in the adapter.

/// A source file registered in the index.
#[derive(Clone, Debug)]
pub struct SourceRef {
    pub drive_id: String,
    pub name: String,
    pub mime: String,
    /// The embedding model that produced this source's vectors (provenance).
    pub embed_model: String,
}

/// A retrieved chunk with its source citation.
#[derive(Clone, Debug)]
pub struct KnowledgePassage {
    pub name: String,
    pub drive_id: String,
    pub text: String,
}

/// A source listing row: file + how many chunks it holds.
#[derive(Clone, Debug)]
pub struct SourceSummary {
    pub name: String,
    pub drive_id: String,
    pub chunks: i64,
}
