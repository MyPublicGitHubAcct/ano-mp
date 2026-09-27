//! Tauri commands for the optional features (PLAN.md §4.6, O1–O19) that
//! read or change the library: thin wrappers over `library`, `history`
//! and the analysis, each refusing when its feature is off. The queue's
//! (radio, practice) and the remote's are with them.

use serde::Serialize;
use tauri::{AppHandle, Runtime};

use crate::history::listenbrainz::{self, ListenBrainzStatus};
use crate::history::views::{self, Highlights, RecentEntry, TopKind, TopPlayed};
use crate::library::analysis::{self, AnalysisProgress, TrackAnalysis};
use crate::library::commands::on_library;
use crate::library::discover::{self, AlbumCard};
use crate::library::health::{self, HealthReport};
use crate::library::lyrics::{self, Lyrics};
use crate::library::prefs::{self, AlbumPrefs, TrackPrefs};
use crate::library::Error;
use crate::metadata::keys::{self, Account};
use crate::settings::{self, FeatureSettings};

fn require<R: Runtime>(
    app: &AppHandle<R>,
    on: impl Fn(&FeatureSettings) -> bool,
    name: &str,
) -> Result<(), String> {
    if on(&settings::current(app).features) {
        Ok(())
    } else {
        Err(format!("{name} is turned off in Settings › Features"))
    }
}

// ---- Analysis (O1, O2) --------------------------------------------------------

#[tauri::command]
pub fn analysis_status<R: Runtime>(app: AppHandle<R>) -> AnalysisProgress {
    analysis::progress(&app)
}

/// The track's waveform for the seek bar, if analysed.
#[tauri::command]
pub async fn analysis_waveform<R: Runtime>(
    app: AppHandle<R>,
    track_id: i64,
) -> Result<Option<Vec<i8>>, String> {
    if !settings::current(&app).features.waveform_seek_bar {
        return Ok(None);
    }
    on_library(&app, move |library| {
        analysis::waveform(&library.conn(), track_id)
    })
    .await
}

#[tauri::command]
pub async fn analysis_track<R: Runtime>(
    app: AppHandle<R>,
    track_id: i64,
) -> Result<Option<TrackAnalysis>, String> {
    on_library(&app, move |library| {
        analysis::track_analysis(&library.conn(), track_id)
    })
    .await
}

// ---- History (O8, O16, O19) --------------------------------------------------

#[tauri::command]
pub async fn history_recent<R: Runtime>(
    app: AppHandle<R>,
    limit: u32,
) -> Result<Vec<RecentEntry>, String> {
    require(
        &app,
        |f| f.listening_history && f.recently_played,
        "Recently played",
    )?;
    on_library(&app, move |library| {
        views::recently_played(&library.conn(), limit.min(500) as usize)
    })
    .await
}

#[tauri::command]
pub async fn history_top<R: Runtime>(
    app: AppHandle<R>,
    kind: TopKind,
    year: i32,
    month: Option<u32>,
) -> Result<TopPlayed, String> {
    require(&app, |f| f.listening_history && f.top_played, "Top played")?;
    on_library(&app, move |library| {
        views::top_played(&library.conn(), kind, year, month)
    })
    .await
}

#[tauri::command]
pub async fn history_highlights<R: Runtime>(app: AppHandle<R>) -> Result<Highlights, String> {
    require(&app, |f| f.listening_history, "The listening history")?;
    on_library(&app, |library| views::highlights(&library.conn())).await
}

/// Forgets every play (not what's waiting for ListenBrainz).
#[tauri::command]
pub async fn history_clear<R: Runtime>(app: AppHandle<R>) -> Result<(), String> {
    on_library(&app, |library| views::clear(&library.conn())).await
}

#[tauri::command]
pub async fn history_listenbrainz_status<R: Runtime>(
    app: AppHandle<R>,
) -> Result<ListenBrainzStatus, String> {
    on_library(&app, |library| {
        listenbrainz::status(&library.conn()).map_err(Error::from)
    })
    .await
}

/// Stores the user's ListenBrainz token in the keychain after checking it
/// with ListenBrainz (`None` removes it); returns the user it belongs to.
#[tauri::command]
pub async fn history_set_listenbrainz_token<R: Runtime>(
    app: AppHandle<R>,
    token: Option<String>,
) -> Result<Option<String>, String> {
    let user = tauri::async_runtime::spawn_blocking(move || -> Result<Option<String>, String> {
        let token = token
            .map(|token| token.trim().to_owned())
            .filter(|t| !t.is_empty());
        let user = match &token {
            Some(token) => {
                if !keys::is_valid(token) {
                    return Err("That isn't a valid ListenBrainz token".into());
                }
                let client = crate::metadata::http::Client::new(
                    Box::new(crate::metadata::http::UreqTransport::new()),
                    Box::new(crate::metadata::http::SystemClock),
                );
                Some(listenbrainz::validate_token(&client, token).map_err(|e| e.to_string())?)
            }
            None => None,
        };
        keys::set(Account::ListenBrainz, token.as_deref()).map_err(|e| e.to_string())?;
        Ok(user)
    })
    .await
    .map_err(|e| e.to_string())??;
    crate::history::wake(&app);
    Ok(user)
}

// ---- Discovery (O15, O17, O18) -------------------------------------------------

#[tauri::command]
pub async fn library_recently_added<R: Runtime>(
    app: AppHandle<R>,
    limit: u32,
) -> Result<Vec<AlbumCard>, String> {
    require(&app, |f| f.recently_added, "Recently added")?;
    on_library(&app, move |library| {
        discover::recently_added(&library.conn(), limit.min(1000))
    })
    .await
}

/// Albums released on today's date (the UI's local date) in earlier years.
#[tauri::command]
pub async fn library_on_this_day<R: Runtime>(
    app: AppHandle<R>,
    year: i32,
    month: u32,
    day: u32,
) -> Result<Vec<AlbumCard>, String> {
    require(&app, |f| f.on_this_day, "On this day")?;
    on_library(&app, move |library| {
        discover::on_this_day(&library.conn(), year, month, day)
    })
    .await
}

#[tauri::command]
pub async fn library_more_in_genre<R: Runtime>(
    app: AppHandle<R>,
    album_id: i64,
    genre: String,
    seed: u64,
) -> Result<Vec<AlbumCard>, String> {
    require(&app, |f| f.more_in_genre, "More in this genre")?;
    on_library(&app, move |library| {
        discover::more_in_genre(&library.conn(), album_id, &genre, seed, 5)
    })
    .await
}

// ---- Health (O4) and lyrics (O13) ---------------------------------------------

#[tauri::command]
pub async fn library_health<R: Runtime>(app: AppHandle<R>) -> Result<HealthReport, String> {
    require(&app, |f| f.health_report, "The health report")?;
    on_library(&app, |library| health::report(&library.conn())).await
}

#[tauri::command]
pub async fn library_lyrics<R: Runtime>(
    app: AppHandle<R>,
    track_id: i64,
) -> Result<Option<Lyrics>, String> {
    require(&app, |f| f.lyrics, "Lyrics")?;
    on_library(&app, move |library| {
        lyrics::lyrics(&library.conn(), track_id)
    })
    .await
}

// ---- Playback preferences (O7) --------------------------------------------------

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Prefs {
    pub track: Option<TrackPrefs>,
    pub album: Option<AlbumPrefs>,
}

/// A track's rules (and its album's), or an album's.
#[tauri::command]
pub async fn prefs_get<R: Runtime>(
    app: AppHandle<R>,
    track_id: Option<i64>,
    album_id: Option<i64>,
) -> Result<Prefs, String> {
    on_library(&app, move |library| {
        let conn = library.conn();
        let album_id = match (track_id, album_id) {
            (_, Some(album)) => Some(album),
            (Some(track), None) => conn
                .query_row(
                    "SELECT album_id FROM tracks WHERE id = ?1",
                    [track],
                    |row| row.get(0),
                )
                .unwrap_or(None),
            _ => None,
        };
        Ok(Prefs {
            track: track_id
                .map(|id| prefs::track_prefs(&conn, id))
                .transpose()?,
            album: album_id
                .map(|id| prefs::album_prefs(&conn, id))
                .transpose()?,
        })
    })
    .await
}

#[tauri::command]
pub async fn prefs_set_track<R: Runtime>(
    app: AppHandle<R>,
    track_id: i64,
    prefs: TrackPrefs,
) -> Result<(), String> {
    require(&app, |f| f.playback_preferences, "Playback preferences")?;
    on_library(&app, move |library| {
        prefs::set_track_prefs(&library.conn(), track_id, &prefs)
    })
    .await?;
    prefs_changed(&app).await;
    Ok(())
}

#[tauri::command]
pub async fn prefs_set_album<R: Runtime>(
    app: AppHandle<R>,
    album_id: i64,
    prefs: AlbumPrefs,
) -> Result<(), String> {
    require(&app, |f| f.playback_preferences, "Playback preferences")?;
    on_library(&app, move |library| {
        prefs::set_album_prefs(&library.conn(), album_id, &prefs)
    })
    .await?;
    prefs_changed(&app).await;
    Ok(())
}

/// The queue's tracks may skip or shuffle differently now, and play louder.
async fn prefs_changed<R: Runtime>(app: &AppHandle<R>) {
    crate::queue::refresh_tracks(app).await;
    crate::queue::refresh_gains(app);
    let _ = tauri::Emitter::emit(app, "library-prefs-changed", ());
}
