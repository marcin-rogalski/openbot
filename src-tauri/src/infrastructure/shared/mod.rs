//! Shared infrastructure: cross-cutting technical utilities (HTTP client today;
//! logger, filesystem, and in-project wrappers as the project grows). Composed
//! before anything that depends on it — see `compose::commons`.

pub mod http;
pub mod time;
