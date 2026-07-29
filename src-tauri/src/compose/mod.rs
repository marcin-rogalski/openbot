//! Composition root — the only place that names concrete adapters and wires them
//! into usecases. Organized **by type**, in dependency order:
//! `commons` (shared singletons) → `driven` (outbound adapters) →
//! `driving` (usecases the inbound side calls). `main.rs` calls `commons::init()`
//! at startup; the per-instance builders in `driven`/`driving` run on demand.

pub mod commons;
pub mod driven;
pub mod driving;
