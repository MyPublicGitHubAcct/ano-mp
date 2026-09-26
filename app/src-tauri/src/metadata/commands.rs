//! Exposes the metadata sources and their settings as Tauri commands.

use serde::Serialize;
use tauri::State;

use super::settings::{self, ServiceSettings, SourceId, SourceInfo};
use crate::library::commands::LibraryState;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MetadataSettings {
    /// Every source this version knows, in the order the UI lists them.
    pub sources: Vec<SourceInfo>,
    pub settings: ServiceSettings,
}

fn with_sources(settings: ServiceSettings) -> MetadataSettings {
    MetadataSettings {
        sources: SourceId::ALL.into_iter().map(SourceId::info).collect(),
        settings,
    }
}

#[tauri::command]
pub fn metadata_settings(state: State<'_, LibraryState>) -> Result<MetadataSettings, String> {
    settings::service_settings(&state.conn())
        .map(with_sources)
        .map_err(|e| e.to_string())
}

/// Stores the settings (completed: every source listed once, every kind's
/// order complete) and returns what was stored. Art may come from other
/// sources now, so the UI should reload it.
#[tauri::command]
pub fn metadata_save_settings(
    state: State<'_, LibraryState>,
    settings: ServiceSettings,
) -> Result<MetadataSettings, String> {
    let saved =
        settings::save_service_settings(&state.conn(), settings).map_err(|e| e.to_string())?;
    state.art.clear();
    Ok(with_sources(saved))
}

/// Back to the default sources and order.
#[tauri::command]
pub fn metadata_reset_settings(state: State<'_, LibraryState>) -> Result<MetadataSettings, String> {
    let settings = settings::reset_service_settings(&state.conn()).map_err(|e| e.to_string())?;
    state.art.clear();
    Ok(with_sources(settings))
}
