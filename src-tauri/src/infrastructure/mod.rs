//! Infrastructure: everything about format, protocol, and IO. Split into driving
//! adapters (call into the app), driven adapters (the app calls out), boundary
//! DTOs, and shared cross-cutting tech.

pub mod driven;
pub mod driving;
pub mod dto;
pub mod shared;
