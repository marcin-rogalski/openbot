//! Composition helpers. As the project grows these split into `compose_*` units —
//! `compose_shared` (cross-cutting infra) runs **first**, then driven adapters,
//! services, and driving adapters as those layers gain startup-composed parts.
//! Credentialed per-instance adapters (e.g. web search) are composed on demand —
//! see `compose_search_web`.

pub mod fetch_page;
pub mod search_web;
pub mod shared;
