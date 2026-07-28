//! Application core: provider- and platform-agnostic. Only ports, services, and
//! usecases live here; nothing imports serenity/reqwest/rusqlite/tauri.

pub mod ports;
pub mod services;
pub mod usecases;
