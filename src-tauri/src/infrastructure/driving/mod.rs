//! Driving (primary) adapters — translate inbound requests into usecase calls:
//! the Discord gateway (`discord`), voice receiver (`voice`), the localhost
//! control API (`control_api`), OS integration (`os`: tray/window/macos), and
//! the per-capability adapters the tool loop invokes.

pub mod control_api;
pub mod discord;
pub mod drive;
pub mod ingestion;
pub mod knowledge;
pub mod memory;
pub mod os;
pub mod transcription;
pub mod voice;
pub mod web;
