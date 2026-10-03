//! Exposes the metadata sources, their settings and the worker as Tauri
//! commands, and the dialogs where the user picks an album's details and
//! cover, or an artist, from what each source offers.

use rusqlite::Connection;
use serde::Serialize;
use tauri::{AppHandle, Manager, Runtime, State};

use super::albums::{self, ReleaseCandidate};
use super::artists::{self, ArtistCandidate};
use super::coverartarchive::{self, Fetched};
use super::discography::{self, Discography};
use super::jobs::{self, Job, MetadataChanged, Progress};
use super::musicbrainz::Release;
use super::settings::{self, Kind, ServiceSettings, SourceId, SourceInfo};
use super::worker;
use super::{keys, Error};
use crate::library::albums::{self as library_albums, AlbumDetails};
use crate::library::art::{self, CoverCandidate};
use crate::library::commands::{on_library, LibraryState};

#[derive(Debug, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
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
    // The sources or their order changed: any album may show another.
    if let Err(error) = crate::library::thumbs::forget_all(&state.conn()) {
        log::warn!("{error}");
    }
    worker::enrich_library(&app);
    Ok(with_sources(saved))
}

/// Saves `key` for `source` in the keychain, or removes it with `None`,
/// and returns the settings, which now say whether it has one. Saving a key
/// turns the source on.
#[tauri::command]
pub async fn metadata_set_key<R: Runtime>(
    app: AppHandle<R>,
    source: SourceId,
    key: Option<String>,
) -> Result<MetadataSettings, String> {
    blocking(app, move |app| {
        if !source.info().needs_key {
            return Err(Error::Invalid(format!(
                "{} doesn't take a key",
                source.info().name
            )));
        }
        let key = key.as_deref().map(str::trim).filter(|key| !key.is_empty());
        keys::set(source, key)?;
        let saved = settings::set_has_key(&library(app)?.conn(), source, key.is_some())?;
        worker::enrich_library(app);
        Ok(with_sources(saved))
    })
    .await
}

/// Back to the default sources and order.
#[tauri::command]
pub fn metadata_reset_settings<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, LibraryState>,
) -> Result<MetadataSettings, String> {
    let settings = settings::reset_service_settings(&state.conn()).map_err(|e| e.to_string())?;
    state.art.clear();
    // The sources or their order changed: any album may show another.
    if let Err(error) = crate::library::thumbs::forget_all(&state.conn()) {
        log::warn!("{error}");
    }
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

// ---- The "Find details", "Choose cover" and "Find artist" dialogs ----------

/// What one source offers in a dialog.
#[derive(Debug, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct SourceCandidates<T> {
    pub source: SourceId,
    pub source_name: &'static str,
    pub candidates: Vec<T>,
    /// Why there are none, when that isn't just "nothing found": offline,
    /// turned off, not matched yet.
    pub note: Option<String>,
    /// What the source's terms want shown next to its data, and the page
    /// on its site with the data listed (the search there).
    pub credit: Option<&'static str>,
    pub credit_url: Option<String>,
}

impl<T> SourceCandidates<T> {
    fn new(source: SourceId, found: Result<Vec<T>, Error>) -> SourceCandidates<T> {
        let (candidates, note) = match found {
            Ok(candidates) => (candidates, None),
            Err(error) => (Vec::new(), Some(error.to_string())),
        };
        SourceCandidates {
            source,
            source_name: source.info().name,
            candidates,
            note,
            credit: None,
            credit_url: None,
        }
    }
}

/// Runs `f` with the app on a blocking thread, since it waits on the
/// worker or reads files.
async fn blocking<R: Runtime, T: Send + 'static>(
    app: AppHandle<R>,
    f: impl FnOnce(&AppHandle<R>) -> Result<T, Error> + Send + 'static,
) -> Result<T, String> {
    tauri::async_runtime::spawn_blocking(move || f(&app))
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| e.to_string())
}

fn library<R: Runtime>(app: &AppHandle<R>) -> Result<State<'_, LibraryState>, Error> {
    app.try_state::<LibraryState>()
        .ok_or_else(|| Error::Invalid("The library is not available".into()))
}

fn current_settings<R: Runtime>(app: &AppHandle<R>) -> Result<ServiceSettings, Error> {
    settings::service_settings(&library(app)?.conn())
}

/// Fails unless `source` may be contacted now.
fn check_usable(conn: &Connection, source: SourceId) -> Result<(), Error> {
    let settings = settings::service_settings(conn)?;
    match settings.is_usable(source) {
        true => Ok(()),
        false => Err(jobs::turned_off(&settings, source)),
    }
}

fn album_changed(album_id: i64) -> MetadataChanged {
    MetadataChanged {
        albums: vec![album_id],
        artists: Vec::new(),
    }
}

fn artist_changed(artist_id: i64) -> MetadataChanged {
    MetadataChanged {
        albums: Vec::new(),
        artists: vec![artist_id],
    }
}

/// Album `album_id`'s details: its tags, its links to the album-details
/// sources, and where its cover comes from.
#[tauri::command]
pub async fn metadata_album<R: Runtime>(
    app: AppHandle<R>,
    album_id: i64,
) -> Result<AlbumDetails, String> {
    on_library(&app, move |library| {
        library_albums::album_details(library, album_id)?.ok_or_else(|| {
            crate::library::Error::Invalid(crate::coded::gone(crate::coded::Gone::Album))
        })
    })
    .await
}

/// The releases each usable album-details source offers for album
/// `album_id`: found by searching for `title` and `artist` (the album's own
/// when `title` is missing), or by the MBID or URL in `title`.
#[tauri::command]
pub async fn metadata_release_candidates<R: Runtime>(
    app: AppHandle<R>,
    album_id: i64,
    title: Option<String>,
    artist: Option<String>,
) -> Result<Vec<SourceCandidates<ReleaseCandidate>>, String> {
    blocking(app, move |app| {
        let settings = current_settings(app)?;
        let title = title.filter(|title| !title.trim().is_empty());
        let artist = artist.filter(|artist| !artist.trim().is_empty());
        let (album_title, album_artist) = {
            let library = library(app)?;
            let conn = library.conn();
            let (facts, _) = albums::album_facts(&conn, album_id)?
                .ok_or_else(|| Error::Invalid(crate::coded::gone(crate::coded::Gone::Album)))?;
            (facts.title, facts.artist)
        };
        let query = match &title {
            Some(title) => format!(
                "{} {}",
                title.trim(),
                artist.as_deref().unwrap_or("").trim()
            ),
            None => format!("{album_title} {}", album_artist.as_deref().unwrap_or("")),
        };
        let mut sources = Vec::new();
        for source in settings.sources_for(Kind::Release) {
            let (title, artist) = (title.clone(), artist.clone());
            let found = worker::call(app, move |context| {
                let search = title
                    .as_deref()
                    .map(|title| (title.trim(), artist.as_deref().map(str::trim)));
                let releases = albums::release_source(source, context.client, context.conn)?;
                albums::release_candidates_at(&*releases, context.conn, album_id, search)
            });
            let mut candidates = SourceCandidates::new(source, found);
            candidates.credit = source.info().credit;
            candidates.credit_url = candidates
                .credit
                .and_then(|_| albums::search_page(source, query.trim()));
            sources.push(candidates);
        }
        Ok(sources)
    })
    .await
}

/// Links album `album_id` to `release_id` at `source`, as the user's choice,
/// then fetches its cover for a MusicBrainz release (arriving in
/// `metadata-changed`).
#[tauri::command]
pub async fn metadata_choose_release<R: Runtime>(
    app: AppHandle<R>,
    album_id: i64,
    source: SourceId,
    release_id: String,
) -> Result<(), String> {
    blocking(app, move |app| {
        library_albums::check_details_source(source)?;
        worker::call(app, move |context| {
            check_usable(context.conn, source)?;
            let covers = coverartarchive::cover_urls(context.conn, album_id)?;
            let releases = albums::release_source(source, context.client, context.conn)?;
            albums::choose_release_at(&*releases, context.conn, album_id, &release_id)?;
            match coverartarchive::cover_urls(context.conn, album_id)? == covers {
                true => context.album_changed(album_id),
                false => context.art_changed(album_id),
            }
            Ok(())
        })?;
        if source == SourceId::MusicBrainz {
            worker::request(app, Job::Cover(album_id));
        }
        Ok(())
    })
    .await
}

/// The release album `album_id` is linked to at `source` (matched, or the
/// candidate awaiting review), or `None` if it has no link there. For a
/// source that doesn't keep its releases (Discogs) it's fetched through the
/// worker, so it fails while the source may not be contacted or can't be
/// reached: those sources' data isn't kept for offline use.
#[tauri::command]
pub async fn metadata_release_details<R: Runtime>(
    app: AppHandle<R>,
    album_id: i64,
    source: SourceId,
) -> Result<Option<Release>, String> {
    blocking(app, move |app| {
        library_albums::check_details_source(source)?;
        worker::call(app, move |context| {
            let Some(link) = albums::album_link(context.conn, album_id, source)? else {
                return Ok(None);
            };
            if link.release.is_some() || link.external_id.is_none() {
                return Ok(link.release);
            }
            check_usable(context.conn, source)?;
            let releases = albums::release_source(source, context.client, context.conn)?;
            albums::linked_release(&*releases, &link)
        })
    })
    .await
}

/// Records that none of `source`'s entries is album `album_id`, as the
/// user's choice.
#[tauri::command]
pub async fn metadata_reject_release<R: Runtime>(
    app: AppHandle<R>,
    album_id: i64,
    source: SourceId,
) -> Result<(), String> {
    on_library(&app, move |library| {
        library_albums::check_details_source(source)?;
        Ok(albums::reject_releases(&library.conn(), album_id, source)?)
    })
    .await?;
    worker::report_changes(&app, &[album_id], album_changed(album_id));
    Ok(())
}

/// Forgets the user's choice (or automatic match) of album `album_id` at
/// `source` and matches it again now; fails at once offline, leaving the
/// album to be matched later.
#[tauri::command]
pub async fn metadata_use_automatic_release<R: Runtime>(
    app: AppHandle<R>,
    album_id: i64,
    source: SourceId,
) -> Result<(), String> {
    on_library(&app, move |library| {
        library_albums::check_details_source(source)?;
        Ok(albums::clear_link(&library.conn(), album_id, source)?)
    })
    .await?;
    worker::report_changes(&app, &[album_id], album_changed(album_id));
    blocking(app, move |app| worker::update_album(app, album_id)).await
}

/// The picture the user chose for an album.
#[derive(Debug, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct ChosenCover {
    pub source: SourceId,
    pub reference: Option<String>,
}

#[derive(Debug, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct CoverChoices {
    pub chosen: Option<ChosenCover>,
    /// Each album-art source shown, in the configured order.
    pub sources: Vec<SourceCandidates<CoverCandidate>>,
}

/// The pictures each album-art source has for album `album_id`. The
/// archive's are listed through the worker; fetch their previews with
/// `metadata_fetch_image`.
#[tauri::command]
pub async fn metadata_cover_candidates<R: Runtime>(
    app: AppHandle<R>,
    album_id: i64,
) -> Result<CoverChoices, String> {
    blocking(app, move |app| {
        let library = library(app)?;
        let settings = settings::service_settings(&library.conn())?;
        let chosen = art::chosen_art(&library.conn(), album_id)?
            .map(|(source, reference)| ChosenCover { source, reference });
        let mut sources = Vec::new();
        for source in settings.sources_shown(Kind::AlbumArt) {
            let found = match source {
                SourceId::CoverArtArchive if !settings.is_usable(source) => {
                    Err(jobs::turned_off(&settings, source))
                }
                SourceId::CoverArtArchive => {
                    let link =
                        albums::album_link(&library.conn(), album_id, SourceId::MusicBrainz)?;
                    if link
                        .as_ref()
                        .and_then(coverartarchive::matched_release)
                        .is_none()
                    {
                        Err(Error::Invalid(
                            "The album isn't matched on MusicBrainz yet: use Find details".into(),
                        ))
                    } else {
                        worker::call(app, move |context| {
                            coverartarchive::cover_candidates(
                                context.client,
                                context.conn,
                                album_id,
                            )
                        })
                    }
                }
                source => art::local_candidates(&library, album_id, source).map_err(Error::from),
            };
            sources.push(SourceCandidates::new(source, found));
        }
        Ok(CoverChoices { chosen, sources })
    })
    .await
}

/// Downloads the archive picture at `url` into the image cache, for a
/// preview; false if the archive doesn't have it.
#[tauri::command]
pub async fn metadata_fetch_image<R: Runtime>(
    app: AppHandle<R>,
    url: String,
) -> Result<bool, String> {
    if !coverartarchive::is_archive_url(&url) {
        return Err(format!("Not a Cover Art Archive URL: {url}"));
    }
    blocking(app, move |app| {
        worker::call(app, move |context| {
            check_usable(context.conn, SourceId::CoverArtArchive)?;
            let fetched = coverartarchive::fetch_image(context.client, context.images, &url)?;
            Ok(fetched != Fetched::NotFound)
        })
    })
    .await
}

/// Makes a picture album `album_id`'s cover, ahead of the source order; an
/// archive picture is downloaded next.
#[tauri::command]
pub async fn metadata_choose_cover<R: Runtime>(
    app: AppHandle<R>,
    album_id: i64,
    source: SourceId,
    reference: Option<String>,
) -> Result<(), String> {
    on_library(&app, move |library| {
        art::choose(library, album_id, Some((source, reference.as_deref())))
    })
    .await?;
    worker::report_changes(&app, &[album_id], album_changed(album_id));
    if source == SourceId::CoverArtArchive {
        worker::request(&app, Job::Cover(album_id));
    }
    Ok(())
}

/// Goes back to the first picture in the source order for album
/// `album_id`, fetching the archive's cover if that's needed now.
#[tauri::command]
pub async fn metadata_use_automatic_cover<R: Runtime>(
    app: AppHandle<R>,
    album_id: i64,
) -> Result<(), String> {
    on_library(&app, move |library| art::choose(library, album_id, None)).await?;
    worker::report_changes(&app, &[album_id], album_changed(album_id));
    worker::request(&app, Job::Cover(album_id));
    Ok(())
}

/// The MusicBrainz artists the user can pick for artist `artist_id`: found
/// by searching for `name` (the artist's own when missing), or by the MBID
/// or URL in it.
#[tauri::command]
pub async fn metadata_artist_candidates<R: Runtime>(
    app: AppHandle<R>,
    artist_id: i64,
    name: Option<String>,
) -> Result<Vec<ArtistCandidate>, String> {
    blocking(app, move |app| {
        worker::call(app, move |context| {
            check_usable(context.conn, SourceId::MusicBrainz)?;
            let name = name
                .as_deref()
                .map(str::trim)
                .filter(|name| !name.is_empty());
            artists::artist_candidates(context.client, context.conn, artist_id, name)
        })
    })
    .await
}

/// Links artist `artist_id` to MusicBrainz artist `mbid`, as the user's
/// choice, then fetches its biography (arriving in `metadata-changed`).
#[tauri::command]
pub async fn metadata_choose_artist<R: Runtime>(
    app: AppHandle<R>,
    artist_id: i64,
    mbid: String,
) -> Result<(), String> {
    blocking(app, move |app| {
        worker::call(app, move |context| {
            check_usable(context.conn, SourceId::MusicBrainz)?;
            artists::choose_artist(context.client, context.conn, artist_id, &mbid)?;
            context.artist_changed(artist_id);
            Ok(())
        })?;
        worker::request(app, Job::Artist(artist_id));
        Ok(())
    })
    .await
}

/// What MusicBrainz lists for artist `artist_id` that the library doesn't
/// have. Fetched through the worker, from the cache unless `refresh`; while
/// MusicBrainz is shown but may not be contacted (online services off),
/// only what was fetched before.
#[tauri::command]
pub async fn metadata_artist_discography<R: Runtime>(
    app: AppHandle<R>,
    artist_id: i64,
    refresh: bool,
) -> Result<Discography, String> {
    blocking(app, move |app| {
        let settings = current_settings(app)?;
        let source = SourceId::MusicBrainz;
        if settings.is_usable(source) {
            worker::call(app, move |context| {
                let mut fetch = discography::online(context.client, context.conn, refresh);
                discography::discography(context.conn, artist_id, &mut fetch)
            })
        } else if settings.is_shown(source) {
            let why = jobs::turned_off(&settings, source).to_string();
            let library = library(app)?;
            let conn = library.conn();
            let mut fetch = discography::cached(&conn, &why);
            discography::discography(&conn, artist_id, &mut fetch)
        } else {
            Err(jobs::turned_off(&settings, source))
        }
    })
    .await
}

/// Records that no MusicBrainz artist is artist `artist_id`, as the user's
/// choice.
#[tauri::command]
pub async fn metadata_reject_artist<R: Runtime>(
    app: AppHandle<R>,
    artist_id: i64,
) -> Result<(), String> {
    on_library(&app, move |library| {
        Ok(artists::reject_artists(&library.conn(), artist_id)?)
    })
    .await?;
    worker::report_changes(&app, &[], artist_changed(artist_id));
    Ok(())
}

/// Forgets the user's choice (or automatic match) for artist `artist_id`
/// and looks the artist up again now.
#[tauri::command]
pub async fn metadata_use_automatic_artist<R: Runtime>(
    app: AppHandle<R>,
    artist_id: i64,
) -> Result<(), String> {
    on_library(&app, move |library| {
        Ok(artists::clear_link(
            &library.conn(),
            artist_id,
            SourceId::MusicBrainz,
        )?)
    })
    .await?;
    worker::report_changes(&app, &[], artist_changed(artist_id));
    blocking(app, move |app| worker::update_artist(app, artist_id)).await
}
