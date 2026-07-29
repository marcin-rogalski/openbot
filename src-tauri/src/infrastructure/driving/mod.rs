//! Driving (primary) adapters — translate inbound requests into usecase calls
//! (Discord gateway, tauri commands, the control API, voice receive, OS input).
//! Migration in progress; most of the app is still driven by the old top-level
//! modules.

pub mod drive;
pub mod knowledge;
pub mod memory;
pub mod web;
