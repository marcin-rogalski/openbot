//! Reusable application services (no IO of their own) injected into usecases —
//! e.g. text chunking, and (as the migration proceeds) prompt assembly, the tool
//! registry, and the tool `Ctx`.

pub mod chunking;
