//! Tauri commands for what the user makes of the library (PLAN.md §4.7):
//! playlists and smart playlists (F1, F2), hearts and ratings (F3), a
//! track's Get Info (F16), and exporting and importing all of it (F20).
//! Thin wrappers over `library`; each emits `collection-changed` naming what
//! changed, so every view showing it reloads.

use std::path::PathBuf;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Runtime};

use crate::library::commands::on_library;
use crate::library::info::{self, TrackDetails};
use crate::library::marks::{self, Favourites, MarkKind};
use crate::library::playlists::{self, ImportReport, Playlist, PlaylistPage};
use crate::library::smart::SmartRules;
use crate::library::transfer::{self, UserData};
use crate::library::Error;

/// Frontend event with a `CollectionChanged` payload.
pub const COLLECTION_CHANGED_EVENT: &str = "collection-changed";

/// What changed.
#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum CollectionChanged {
    Playlists,
    Favourites,
    Ratings,
    /// Everything (an import).
    All,
}

fn changed<R: Runtime>(app: &AppHandle<R>, what: CollectionChanged) {
    let _ = app.emit(COLLECTION_CHANGED_EVENT, what);
}

// ---- Playlists (F1, F2) -------------------------------------------------------

#[tauri::command]
pub async fn playlists_list<R: Runtime>(app: AppHandle<R>) -> Result<Vec<Playlist>, String> {
    on_library(&app, |library| playlists::playlists(&library.conn())).await
}

/// A page of a playlist's entries.
#[tauri::command]
pub async fn playlists_page<R: Runtime>(
    app: AppHandle<R>,
    playlist_id: i64,
    offset: u32,
    limit: u32,
) -> Result<PlaylistPage, String> {
    on_library(&app, move |library| {
        playlists::page(&library.conn(), playlist_id, offset, limit.min(1000))
    })
    .await
}

/// Creates a playlist of `track_ids`, or a smart playlist of `rules`.
#[tauri::command]
pub async fn playlists_create<R: Runtime>(
    app: AppHandle<R>,
    name: String,
    rules: Option<SmartRules>,
    track_ids: Vec<i64>,
) -> Result<Playlist, String> {
    let playlist = on_library(&app, move |library| {
        playlists::create(&mut library.conn(), &name, rules.as_ref(), &track_ids)
    })
    .await?;
    changed(&app, CollectionChanged::Playlists);
    Ok(playlist)
}

/// How many tracks `rules` match now (up to its limit), for the rules
/// dialog.
#[tauri::command]
pub async fn playlists_preview<R: Runtime>(
    app: AppHandle<R>,
    rules: SmartRules,
) -> Result<u32, String> {
    on_library(&app, move |library| {
        crate::library::smart::summary(&library.conn(), &rules).map(|(count, _)| count)
    })
    .await
}

/// "Save queue as playlist": the queue's library tracks, in order.
#[tauri::command]
pub async fn playlists_create_from_queue<R: Runtime>(
    app: AppHandle<R>,
    name: String,
) -> Result<Playlist, String> {
    let track_ids = crate::queue::library_track_ids(&app)?;
    playlists_create(app, name, None, track_ids).await
}

#[tauri::command]
pub async fn playlists_rename<R: Runtime>(
    app: AppHandle<R>,
    playlist_id: i64,
    name: String,
) -> Result<Playlist, String> {
    let playlist = on_library(&app, move |library| {
        playlists::rename(&library.conn(), playlist_id, &name)
    })
    .await?;
    changed(&app, CollectionChanged::Playlists);
    Ok(playlist)
}

#[tauri::command]
pub async fn playlists_set_rules<R: Runtime>(
    app: AppHandle<R>,
    playlist_id: i64,
    rules: SmartRules,
) -> Result<Playlist, String> {
    let playlist = on_library(&app, move |library| {
        playlists::set_rules(&library.conn(), playlist_id, &rules)
    })
    .await?;
    changed(&app, CollectionChanged::Playlists);
    Ok(playlist)
}

#[tauri::command]
pub async fn playlists_delete<R: Runtime>(
    app: AppHandle<R>,
    playlist_id: i64,
) -> Result<(), String> {
    on_library(&app, move |library| {
        playlists::delete(&library.conn(), playlist_id)
    })
    .await?;
    changed(&app, CollectionChanged::Playlists);
    Ok(())
}

/// Adds tracks before entry `at`, or at the end; returns how many.
#[tauri::command]
pub async fn playlists_add<R: Runtime>(
    app: AppHandle<R>,
    playlist_id: i64,
    track_ids: Vec<i64>,
    at: Option<usize>,
) -> Result<usize, String> {
    let added = on_library(&app, move |library| {
        playlists::add(&mut library.conn(), playlist_id, &track_ids, at)
    })
    .await?;
    changed(&app, CollectionChanged::Playlists);
    Ok(added)
}

#[tauri::command]
pub async fn playlists_remove<R: Runtime>(
    app: AppHandle<R>,
    playlist_id: i64,
    item_ids: Vec<i64>,
) -> Result<(), String> {
    on_library(&app, move |library| {
        playlists::remove(&mut library.conn(), playlist_id, &item_ids)
    })
    .await?;
    changed(&app, CollectionChanged::Playlists);
    Ok(())
}

/// Moves entries to index `to` of the list after the move.
#[tauri::command]
pub async fn playlists_move<R: Runtime>(
    app: AppHandle<R>,
    playlist_id: i64,
    item_ids: Vec<i64>,
    to: usize,
) -> Result<(), String> {
    on_library(&app, move |library| {
        playlists::move_items(&mut library.conn(), playlist_id, &item_ids, to)
    })
    .await?;
    changed(&app, CollectionChanged::Playlists);
    Ok(())
}

/// The playlist's tracks, to play or add to the queue.
#[tauri::command]
pub async fn playlists_track_ids<R: Runtime>(
    app: AppHandle<R>,
    playlist_id: i64,
) -> Result<Vec<i64>, String> {
    on_library(&app, move |library| {
        playlists::track_ids(&library.conn(), playlist_id)
    })
    .await
}

/// Imports an M3U or M3U8 file the user picked.
#[tauri::command]
pub async fn playlists_import<R: Runtime>(
    app: AppHandle<R>,
    path: PathBuf,
) -> Result<ImportReport, String> {
    let report = on_library(&app, move |library| {
        playlists::import_m3u(&mut library.conn(), &path)
    })
    .await?;
    changed(&app, CollectionChanged::Playlists);
    Ok(report)
}

/// Writes a playlist as M3U8 to the file the user named; returns how many
/// tracks.
#[tauri::command]
pub async fn playlists_export<R: Runtime>(
    app: AppHandle<R>,
    playlist_id: i64,
    path: PathBuf,
) -> Result<usize, String> {
    on_library(&app, move |library| {
        playlists::export_m3u(&library.conn(), playlist_id, &path)
    })
    .await
}

// ---- Hearts and ratings (F3) --------------------------------------------------

/// Hearts (or unhearts) tracks, albums or artists.
#[tauri::command]
pub async fn marks_set_favourite<R: Runtime>(
    app: AppHandle<R>,
    kind: MarkKind,
    ids: Vec<i64>,
    favourite: bool,
) -> Result<(), String> {
    on_library(&app, move |library| {
        marks::set_favourite(&library.conn(), kind, &ids, favourite).map(|_| ())
    })
    .await?;
    changed(&app, CollectionChanged::Favourites);
    Ok(())
}

/// Which of `ids` are hearted.
#[tauri::command]
pub async fn marks_favourites_among<R: Runtime>(
    app: AppHandle<R>,
    kind: MarkKind,
    ids: Vec<i64>,
) -> Result<Vec<i64>, String> {
    on_library(&app, move |library| {
        marks::favourites_among(&library.conn(), kind, &ids)
    })
    .await
}

/// Rates tracks 1 to 5 stars, or clears their rating with null.
#[tauri::command]
pub async fn marks_set_rating<R: Runtime>(
    app: AppHandle<R>,
    track_ids: Vec<i64>,
    rating: Option<u8>,
) -> Result<(), String> {
    on_library(&app, move |library| {
        marks::set_rating(&library.conn(), &track_ids, rating)
    })
    .await?;
    changed(&app, CollectionChanged::Ratings);
    Ok(())
}

/// Everything hearted: the Favourites view.
#[tauri::command]
pub async fn marks_favourites<R: Runtime>(app: AppHandle<R>) -> Result<Favourites, String> {
    on_library(&app, |library| marks::favourites(&library.conn())).await
}

// ---- Get Info (F16) -------------------------------------------------------------

#[tauri::command]
pub async fn library_track_details<R: Runtime>(
    app: AppHandle<R>,
    track_id: i64,
) -> Result<TrackDetails, String> {
    on_library(&app, move |library| {
        info::track_details(library, track_id)?
            .ok_or_else(|| Error::Invalid(crate::coded::gone(crate::coded::Gone::Track)))
    })
    .await
}

// ---- Export and import (F20) ------------------------------------------------------

/// Writes what the user made to the file they named.
#[tauri::command]
pub async fn data_export<R: Runtime>(app: AppHandle<R>, path: PathBuf) -> Result<(), String> {
    let queue = crate::queue::snapshot(&app).ok();
    let data = on_library(&app, move |library| {
        transfer::export(&library.conn(), queue)
    })
    .await?;
    let json = serde_json::to_vec_pretty(&data).map_err(|e| e.to_string())?;
    tauri::async_runtime::spawn_blocking(move || std::fs::write(&path, json))
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| format!("Cannot write the file: {e}"))
}

/// What importing a data file did.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DataImport {
    #[serde(flatten)]
    pub report: transfer::ImportReport,
    /// The settings were replaced by the file's.
    pub settings: bool,
    /// The queue was empty and took the file's.
    pub queue: bool,
}

/// Adds the data in the file the user picked to the library; with
/// `settings`, also replaces the settings, sort rules and online source
/// settings with the file's.
#[tauri::command]
pub async fn data_import<R: Runtime>(
    app: AppHandle<R>,
    path: PathBuf,
    settings: bool,
) -> Result<DataImport, String> {
    let bytes = tauri::async_runtime::spawn_blocking(move || std::fs::read(&path))
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| format!("Cannot read the file: {e}"))?;
    let data: UserData =
        serde_json::from_slice(&bytes).map_err(|_| crate::library::transfer::not_data_file())?;
    transfer::check(&data).map_err(|e| e.to_string())?;

    let imported = data.clone();
    let (report, queue_ids) = on_library(&app, move |library| {
        let mut conn = library.conn();
        let found = transfer::resolve(&conn, &imported)?;
        let report = transfer::import(&mut conn, &imported, &found)?;
        let queue_ids = imported.queue.as_ref().map(|queue| {
            let ids: Vec<i64> = queue
                .tracks
                .iter()
                .filter_map(|&index| found.tracks.get(index).copied().flatten())
                .collect();
            (ids, queue.current, queue.position)
        });
        Ok::<_, Error>((report, queue_ids))
    })
    .await?;

    let mut applied_settings = false;
    if settings {
        applied_settings = apply_settings(&app, &data).await?;
    }
    let mut queue = false;
    if let Some((ids, current, position)) = queue_ids {
        queue = crate::queue::restore_imported(&app, ids, current, position).await?;
    }
    changed(&app, CollectionChanged::All);
    crate::queue::refresh_tracks(&app).await;
    Ok(DataImport {
        report,
        settings: applied_settings,
        queue,
    })
}

/// Stores the file's sort rules and online source settings as they are
/// (both are read leniently), and applies its app settings like the
/// settings page does.
async fn apply_settings<R: Runtime>(app: &AppHandle<R>, data: &UserData) -> Result<bool, String> {
    let rows: Vec<(String, String)> = transfer::SETTINGS_KEYS
        .iter()
        .filter(|key| **key != "app")
        .filter_map(|key| {
            data.settings
                .get(*key)
                .map(|value| ((*key).to_owned(), value.to_string()))
        })
        .collect();
    let any = !rows.is_empty() || data.settings.contains_key("app");
    on_library(app, move |library| {
        let conn = library.conn();
        for (key, value) in rows {
            conn.execute(
                "INSERT INTO settings (key, value) VALUES (?1, ?2)
                 ON CONFLICT (key) DO UPDATE SET value = excluded.value",
                [key, value],
            )?;
        }
        Ok(())
    })
    .await?;
    if let Some(app_settings) = data.settings.get("app") {
        let settings = crate::settings::from_value(app_settings);
        crate::settings::settings_save(app.clone(), settings).await?;
    }
    Ok(any)
}
