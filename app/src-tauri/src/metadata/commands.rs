//! Exposes the metadata sources, their settings and the worker as Tauri
//! commands.

use serde::Serialize;
use tauri::{AppHandle, Runtime, State};

use super::jobs::Progress;
use super::settings::{self, ServiceSettings, SourceId, SourceInfo};
use super::worker;
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
/// sources now, so the UI should reload it. Sources may have been turned
/// on, so background enrichment is queued if it's enabled.
#[tauri::command]
pub fn metadata_save_settings<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, LibraryState>,
    settings: ServiceSettings,
) -> Result<MetadataSettings, String> {
    let saved =
        settings::save_service_settings(&state.conn(), settings).map_err(|e| e.to_string())?;
    state.art.clear();
    worker::enrich_library(&app);
    Ok(with_sources(saved))
}

/// Back to the default sources and order.
#[tauri::command]
pub fn metadata_reset_settings<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, LibraryState>,
) -> Result<MetadataSettings, String> {
    let settings = settings::reset_service_settings(&state.conn()).map_err(|e| e.to_string())?;
    state.art.clear();
    worker::enrich_library(&app);
    Ok(with_sources(settings))
}

/// What the metadata worker is doing, as last sent in `metadata-progress`.
#[tauri::command]
pub fn metadata_status<R: Runtime>(app: AppHandle<R>) -> Progress {
    worker::progress(&app)
}

/// Tries services that couldn't be reached again now, resuming paused work.
#[tauri::command]
pub fn metadata_retry_now<R: Runtime>(app: AppHandle<R>) {
    worker::retry_now(&app);
}

/// Matches album `album_id` and fetches its cover now, as far as each is
/// needed, ahead of background work, even if "match automatically" is off
/// or it was tried lately. Fails at once when the service is unreachable.
#[tauri::command]
pub async fn metadata_update_album<R: Runtime>(
    app: AppHandle<R>,
    album_id: i64,
) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || worker::update_album(&app, album_id))
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| e.to_string())
}

/// Matches artist `artist_id` and fetches its biography now, as
/// `metadata_update_album` does for albums.
#[tauri::command]
pub async fn metadata_update_artist<R: Runtime>(
    app: AppHandle<R>,
    artist_id: i64,
) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || worker::update_artist(&app, artist_id))
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| e.to_string())
}
