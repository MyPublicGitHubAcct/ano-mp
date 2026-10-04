//! Tauri commands for the optional features (PLAN.md §4.6, O1–O19, and
//! X4's and X5's recommendations) that read or change the library: thin wrappers
//! over `library`, `history` and the analysis, each refusing when its
//! feature is off. The queue's (radio, practice) and the remote's are with
//! them.

use rusqlite::Connection;
use serde::Serialize;
use tauri::{AppHandle, Manager, Runtime};

use crate::history::listenbrainz::{self, ListenBrainzStatus};
use crate::history::views::{self, Highlights, RecentEntry, TopKind, TopPlayed};
use crate::library::analysis::{self, AnalysisProgress, TrackAnalysis};
use crate::library::commands::{on_library, LibraryState};
use crate::library::discover::{self, AlbumCard};
use crate::library::health::{self, HealthReport};
use crate::library::lyrics::{self, Lyrics};
use crate::library::prefs::{self, AlbumPrefs, TrackPrefs};
use crate::library::similar::{self, SimilarAlbum, SimilarArtist, SimilarTrack};
use crate::library::Error;
use crate::metadata::keys::{self, Account};
use crate::metadata::listenbrainz as metadata_listenbrainz;
use crate::metadata::outside::{self, Fetch, OutsideArtist, OutsideLinks, OutsideStatus};
use crate::metadata::settings as metadata_settings;
use crate::metadata::{musicbrainz, worker, Error as MetadataError};
use crate::settings::{self, FeatureSettings};

fn require<R: Runtime>(
    app: &AppHandle<R>,
    on: impl Fn(&FeatureSettings) -> bool,
    (feature, name): (&str, &str),
) -> Result<(), String> {
    if on(&settings::current(app).features) {
        Ok(())
    } else {
        Err(crate::coded::feature_off(feature, name))
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
        ("recentlyPlayed", "Recently played"),
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
    require(
        &app,
        |f| f.listening_history && f.top_played,
        ("topPlayed", "Top played"),
    )?;
    on_library(&app, move |library| {
        views::top_played(&library.conn(), kind, year, month)
    })
    .await
}

#[tauri::command]
pub async fn history_highlights<R: Runtime>(app: AppHandle<R>) -> Result<Highlights, String> {
    require(
        &app,
        |f| f.listening_history,
        ("listeningHistory", "The listening history"),
    )?;
    let unreadable = crate::library::availability::unreadable(&app);
    on_library(&app, move |library| {
        views::highlights(&library.conn(), &unreadable)
    })
    .await
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
    require(
        &app,
        |f| f.recently_added,
        ("recentlyAdded", "Recently added"),
    )?;
    let unreadable = crate::library::availability::unreadable(&app);
    on_library(&app, move |library| {
        discover::recently_added(&library.conn(), limit.min(1000), &unreadable)
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
    require(
        &app,
        |f| f.on_this_day,
        ("onThisDay", "Released on this day"),
    )?;
    let unreadable = crate::library::availability::unreadable(&app);
    on_library(&app, move |library| {
        discover::on_this_day(&library.conn(), year, month, day, &unreadable)
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
    require(
        &app,
        |f| f.more_in_genre,
        ("moreInGenre", "More in this genre"),
    )?;
    let unreadable = crate::library::availability::unreadable(&app);
    on_library(&app, move |library| {
        discover::more_in_genre(&library.conn(), album_id, &genre, seed, 5, &unreadable)
    })
    .await
}

// ---- Recommendations (X4) -----------------------------------------------------

const RECOMMENDATIONS: (&str, &str) = ("recommendations", "Recommendations");

/// Tracks like `track_id` on other albums.
#[tauri::command]
pub async fn library_similar_tracks<R: Runtime>(
    app: AppHandle<R>,
    track_id: i64,
) -> Result<Vec<SimilarTrack>, String> {
    require(&app, |f| f.recommendations, RECOMMENDATIONS)?;
    let unreadable = crate::library::availability::unreadable(&app);
    on_library(&app, move |library| {
        similar::similar_tracks(&library.conn(), track_id, &unreadable, similar::SHOWN)
    })
    .await
}

#[tauri::command]
pub async fn library_similar_albums<R: Runtime>(
    app: AppHandle<R>,
    album_id: i64,
) -> Result<Vec<SimilarAlbum>, String> {
    require(&app, |f| f.recommendations, RECOMMENDATIONS)?;
    let unreadable = crate::library::availability::unreadable(&app);
    on_library(&app, move |library| {
        similar::similar_albums(&library.conn(), album_id, &unreadable, similar::SHOWN)
    })
    .await
}

#[tauri::command]
pub async fn library_similar_artists<R: Runtime>(
    app: AppHandle<R>,
    artist_id: i64,
) -> Result<Vec<SimilarArtist>, String> {
    require(&app, |f| f.recommendations, RECOMMENDATIONS)?;
    let unreadable = crate::library::availability::unreadable(&app);
    on_library(&app, move |library| {
        similar::similar_artists(&library.conn(), artist_id, &unreadable, similar::SHOWN)
    })
    .await
}

/// Home's suggestions, varied by the day.
#[tauri::command]
pub async fn library_for_you<R: Runtime>(app: AppHandle<R>) -> Result<Vec<SimilarAlbum>, String> {
    require(&app, |f| f.recommendations, RECOMMENDATIONS)?;
    let unreadable = crate::library::availability::unreadable(&app);
    on_library(&app, move |library| {
        let now = crate::library::unix_now();
        similar::for_you(
            &library.conn(),
            &unreadable,
            similar::SHOWN,
            now,
            Some((now / 86400) as u64),
        )
    })
    .await
}

// ---- Recommendations from outside the library (X5) ---------------------------

const OUTSIDE: (&str, &str) = (
    "outsideRecommendations",
    "Recommendations from outside the library",
);

/// Whether online services may be contacted (Settings › Online sources).
fn online<R: Runtime>(app: &AppHandle<R>) -> Result<bool, MetadataError> {
    let library = app
        .try_state::<LibraryState>()
        .ok_or_else(|| MetadataError::Invalid("The library is not available".into()))?;
    let online = metadata_settings::service_settings(&library.conn())?.online;
    Ok(online)
}

/// Runs `f` with ListenBrainz's lists: on the metadata worker, fetching
/// as needed, while online services are on; else on a library connection
/// with what the response cache has.
async fn with_outside<R: Runtime, T: Send + 'static>(
    app: &AppHandle<R>,
    f: impl FnOnce(&Connection, &mut Fetch) -> Result<T, MetadataError> + Send + 'static,
) -> Result<T, String> {
    require(app, |f| f.outside_recommendations, OUTSIDE)?;
    let app = app.clone();
    tauri::async_runtime::spawn_blocking(move || -> Result<T, MetadataError> {
        if online(&app)? {
            worker::call(&app, move |context| {
                let (client, conn) = (context.client, context.conn);
                f(conn, &mut |mbid| {
                    metadata_listenbrainz::similar_artists(client, conn, mbid)
                })
            })
        } else {
            let library = app
                .try_state::<LibraryState>()
                .ok_or_else(|| MetadataError::Invalid("The library is not available".into()))?;
            let conn = library.conn();
            f(&conn, &mut |mbid| {
                metadata_listenbrainz::cached_similar_artists(&conn, mbid)
            })
        }
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| e.to_string())
}

/// Home's artists from outside the library.
#[tauri::command]
pub async fn outside_for_you<R: Runtime>(app: AppHandle<R>) -> Result<Vec<OutsideArtist>, String> {
    with_outside(&app, |conn, fetch| {
        let seeds = outside::seeds(conn, crate::library::unix_now(), outside::SEEDS)?;
        let found = outside::recommend(conn, &seeds, fetch, outside::SHOWN)?;
        log::info!("{} from {} seed artists", found.len(), seeds.len());
        Ok(found)
    })
    .await
}

/// Artists outside the library like library artist `artist_id`; none
/// while it has no MusicBrainz id.
#[tauri::command]
pub async fn outside_like_artist<R: Runtime>(
    app: AppHandle<R>,
    artist_id: i64,
) -> Result<Vec<OutsideArtist>, String> {
    with_outside(&app, move |conn, fetch| {
        match outside::artist_seed(conn, artist_id)? {
            Some(seed) => outside::recommend(conn, &[seed], fetch, outside::SHOWN),
            None => Ok(Vec::new()),
        }
    })
    .await
}

/// A suggestion's links out: its homepage and Bandcamp page come from
/// MusicBrainz while it may be contacted, or from an earlier lookup.
#[tauri::command]
pub async fn outside_links<R: Runtime>(
    app: AppHandle<R>,
    mbid: String,
) -> Result<OutsideLinks, String> {
    require(&app, |f| f.outside_recommendations, OUTSIDE)?;
    if !musicbrainz::is_mbid(&mbid) {
        return Err(format!("Not a MusicBrainz id: {mbid}"));
    }
    tauri::async_runtime::spawn_blocking(move || -> Result<OutsideLinks, MetadataError> {
        let usable = {
            let library = app
                .try_state::<LibraryState>()
                .ok_or_else(|| MetadataError::Invalid("The library is not available".into()))?;
            let settings = metadata_settings::service_settings(&library.conn())?;
            settings.is_usable(metadata_settings::SourceId::MusicBrainz)
        };
        let artist = if usable {
            let id = mbid.clone();
            // Without the homepage, the other links still serve.
            worker::call(&app, move |context| {
                musicbrainz::lookup_artist(context.client, context.conn, &id)
            })
            .map_err(|error| log::debug!("{error}"))
            .ok()
        } else {
            None
        };
        Ok(outside::links(&mbid, artist.as_ref()))
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| e.to_string())
}

/// Stops suggesting `mbid`.
#[tauri::command]
pub async fn outside_dismiss<R: Runtime>(
    app: AppHandle<R>,
    mbid: String,
    name: String,
) -> Result<(), String> {
    require(&app, |f| f.outside_recommendations, OUTSIDE)?;
    on_library(&app, move |library| {
        outside::dismiss(&library.conn(), &mbid, &name, crate::library::unix_now())
            .map_err(Error::from)
    })
    .await
}

/// What Settings shows: the artists whose ids are sent, and how many
/// suggestions were dismissed.
#[tauri::command]
pub async fn outside_status<R: Runtime>(app: AppHandle<R>) -> Result<OutsideStatus, String> {
    on_library(&app, |library| {
        outside::status(&library.conn(), crate::library::unix_now()).map_err(Error::from)
    })
    .await
}

#[tauri::command]
pub async fn outside_forget_dismissed<R: Runtime>(app: AppHandle<R>) -> Result<(), String> {
    on_library(&app, |library| {
        outside::forget_dismissed(&library.conn())
            .map(drop)
            .map_err(Error::from)
    })
    .await
}

// ---- Health (O4) and lyrics (O13) ---------------------------------------------

#[tauri::command]
pub async fn library_health<R: Runtime>(app: AppHandle<R>) -> Result<HealthReport, String> {
    require(
        &app,
        |f| f.health_report,
        ("healthReport", "The health report"),
    )?;
    on_library(&app, |library| health::report(&library.conn())).await
}

#[tauri::command]
pub async fn library_lyrics<R: Runtime>(
    app: AppHandle<R>,
    track_id: i64,
) -> Result<Option<Lyrics>, String> {
    require(&app, |f| f.lyrics, ("lyrics", "Lyrics"))?;
    on_library(&app, move |library| {
        lyrics::lyrics(&library.conn(), track_id)
    })
    .await
}

// ---- Playback preferences (O7) --------------------------------------------------

#[derive(Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[cfg_attr(test, ts(rename = "FeaturePrefs"))]
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
    require(
        &app,
        |f| f.playback_preferences,
        ("playbackPreferences", "Playback preferences"),
    )?;
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
    require(
        &app,
        |f| f.playback_preferences,
        ("playbackPreferences", "Playback preferences"),
    )?;
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
