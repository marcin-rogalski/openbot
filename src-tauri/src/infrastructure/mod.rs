//! Infrastructure: everything about format, protocol, and IO. Split into driving
//! adapters (call into the app), driven adapters (the app calls out), boundary
//! DTOs, and shared cross-cutting tech. `bot` (runtime + UI event sink) and
//! `config` (persisted config) are cross-cutting infra used across adapters.

pub mod bot;
pub mod config;
pub mod driven;
pub mod driving;
pub mod dto;
pub mod shared;
