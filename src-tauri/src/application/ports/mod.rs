//! Ports: the contracts the application needs. Defined here (by the app's needs),
//! implemented by driven adapters, called by usecases / driving adapters.

pub mod drive;
pub mod knowledge;
pub mod memory;
pub mod webfetch;
pub mod websearch;
