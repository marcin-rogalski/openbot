//! The tool manifest — the code-owned schema (the Definition layer of
//! ADR-0001/0005) that the frontend renders every tool editor and approval list
//! from. It is the single source of truth: `src/lib/config.ts` no longer
//! hardcodes tool classes, ops, or config fields; it fetches these instead.
//!
//! Each tool module builds its own `manifest()`; `tools::manifests()` aggregates
//! them and `tools::commands::tool_manifests` exposes them to the UI.

use serde::Serialize;

/// One tool class's full schema: how to configure an instance + what it can do.
/// Serialized as-is to the frontend (tools are outside the hexagon, so one struct
/// doubling as the wire shape is fine here).
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolManifest {
    /// The persisted `ToolInstance.type`.
    pub kind: &'static str,
    /// Human label for menus and section headers.
    pub label: &'static str,
    /// Emoji icon; the frontend may swap a real logo keyed by `kind`.
    pub icon: &'static str,
    /// Whether the instance needs an OAuth "Connect" step (Drive).
    pub oauth: bool,
    /// A one-line caption shown above the config fields (nullable).
    pub config_caption: Option<&'static str>,
    /// The config fields an instance carries.
    pub config_fields: Vec<ManifestField>,
    /// The callable ops this tool exposes (empty for event-driven tools).
    pub ops: Vec<ManifestOp>,
}

/// One config field on an instance, mapped by `key` to the camelCase
/// `ToolInstance` property the editor reads and writes.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ManifestField {
    pub key: &'static str,
    pub label: &'static str,
    /// Render as a password input.
    pub secret: bool,
}

/// One callable op: its suffix, a label, and whether it writes (which sets the
/// default approval policy — writes → ask, reads → allow).
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ManifestOp {
    pub op: &'static str,
    pub label: &'static str,
    pub write: bool,
}
