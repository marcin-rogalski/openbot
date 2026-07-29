//! Business domain: the representation of data + operations on it (validation,
//! sanitization, invariants). Pure — std + serde only, no IO or external crates.

pub mod drive;
pub mod knowledge;
pub mod memory;
pub mod page;
pub mod search;
pub mod transcript;
