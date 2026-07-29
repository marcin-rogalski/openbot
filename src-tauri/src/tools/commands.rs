//! Tauri commands exposing the tool boundary to the desktop UI. Lives in the
//! `tools` layer (not `infrastructure/driving`) because the manifest is a
//! tools-layer concern and `infrastructure` must not depend on the outer layer.

use super::manifest::ToolManifest;

/// The schema of every configurable tool class. The frontend renders the "+ Add
/// tool" menu, instance editors, and per-bot approval lists from this.
#[tauri::command]
pub fn tool_manifests() -> Vec<ToolManifest> {
    super::manifests()
}
