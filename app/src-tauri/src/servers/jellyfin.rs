//! The Jellyfin server's REST API (PLAN.md Phase 7e), as far as the app
//! uses it: who the server is, signing in (with a password or Quick
//! Connect), its music a page at a time, its playlists, where to fetch a
//! track or a picture, reporting what plays, and signing out.
//!
//! Every request goes through `http::Client` (rate limits, offline backoff,
//! the test fake) with Jellyfin's `Authorization: MediaBrowser …` header,
//! which carries the token; tokens never go in URLs. Responses are parsed
//! into the neutral types of `servers` (`ServerTrack`, `ServerPlaylist`), so
//! the sync doesn't know which kind of server it reads.
//!
//! The calls are those of Jellyfin 10.9 and later (`MIN_VERSION`): `/Items`
//! with a `userId`, and Quick Connect's `POST /QuickConnect/Initiate`.

use serde::Deserialize;
use serde_json::json;

use super::sync::ServerLibrary;
use super::{Device, ServerInfo, ServerPlaylist, ServerTrack, SignedIn};
use crate::coded::coded;
use crate::metadata::http::{percent_encode, Client, JSON_LIMIT};
use crate::metadata::Error;

/// The oldest Jellyfin the app talks to: major, minor.
pub const MIN_VERSION: (u32, u32) = (10, 9);

/// Tracks asked for per page of a sync.
pub const PAGE_SIZE: usize = 500;

/// Item ids asked for per page when looking for removed tracks.
pub const ID_PAGE_SIZE: usize = 2000;

/// The optional fields a sync asks for with each track.
const TRACK_FIELDS: &str = "MediaSources,Genres,ProviderIds,DateCreated";

/// Jellyfin counts time in ticks of 100 ns.
const TICKS_PER_SECOND: f64 = 10_000_000.0;

/// The widest picture asked for: the image cache's thumbnails are made
/// from it (H17), and "full" size shows it.
const IMAGE_WIDTH: u32 = 1200;

/// A signed-in (or signing-in) conversation with one server.
#[derive(Clone, Copy)]
pub struct Jellyfin<'a> {
    pub http: &'a Client,
    /// As `address::parse` gives it.
    pub address: &'a str,
    pub device: &'a Device,
    pub token: Option<&'a str>,
}

impl Jellyfin<'_> {
    /// The `Authorization` header: the app, this device and the token.
    pub fn authorization(&self) -> String {
        let quote = |text: &str| text.replace(['"', '\\', '\r', '\n'], "");
        let mut header = format!(
            "MediaBrowser Client=\"ano-mp\", Device=\"{}\", DeviceId=\"{}\", Version=\"{}\"",
            quote(&self.device.name),
            quote(&self.device.id),
            quote(&crate::anomp::version()),
        );
        if let Some(token) = self.token {
            header.push_str(&format!(", Token=\"{}\"", quote(token)));
        }
        header
    }

    fn url(&self, path_and_query: &str) -> String {
        format!("{}{path_and_query}", self.address)
    }

    fn get<T: for<'de> Deserialize<'de>>(&self, path_and_query: &str) -> Result<T, Error> {
        let url = self.url(path_and_query);
        let auth = self.authorization();
        let response =
            self.http
                .get_with_auth(&url, "application/json", Some(&auth), JSON_LIMIT)?;
        serde_json::from_slice(&response.body)
            .map_err(|error| Error::Invalid(format!("{path_and_query}: {error}")))
    }

    fn post<T: for<'de> Deserialize<'de>>(
        &self,
        path: &str,
        body: &serde_json::Value,
    ) -> Result<T, Error> {
        let response = self.post_empty(path, body)?;
        serde_json::from_slice(&response)
            .map_err(|error| Error::Invalid(format!("{path}: {error}")))
    }

    /// POSTs `body`, ignoring what comes back (often 204 No Content).
    fn post_empty(&self, path: &str, body: &serde_json::Value) -> Result<Vec<u8>, Error> {
        let auth = self.authorization();
        let response = self
            .http
            .post_json(&self.url(path), &body.to_string(), Some(&auth))?;
        Ok(response.body)
    }

    /// Who the server is; needs no sign-in. Fails with `serverNotJellyfin`
    /// when something else answers, and `serverTooOld` for a version before
    /// `MIN_VERSION`.
    pub fn public_info(&self) -> Result<ServerInfo, Error> {
        let info: PublicInfo = self
            .get("/System/Info/Public")
            .map_err(|error| match error {
                Error::Invalid(_) | Error::Status { status: 404, .. } => Error::Invalid(coded(
                    "serverNotJellyfin",
                    &[],
                    "Nothing at that address answers as a Jellyfin server",
                )),
                other => other,
            })?;
        if info.id.is_empty()
            || info
                .product_name
                .as_deref()
                .is_some_and(|p| !p.contains("Jellyfin"))
        {
            return Err(Error::Invalid(coded(
                "serverNotJellyfin",
                &[],
                "Nothing at that address answers as a Jellyfin server",
            )));
        }
        let version = info.version.unwrap_or_default();
        if version_below(&version, MIN_VERSION) {
            return Err(Error::Invalid(coded(
                "serverTooOld",
                &[
                    ("version", json!(version)),
                    (
                        "minimum",
                        json!(format!("{}.{}", MIN_VERSION.0, MIN_VERSION.1)),
                    ),
                ],
                format!(
                    "This server runs Jellyfin {version}; the app needs {}.{} or later",
                    MIN_VERSION.0, MIN_VERSION.1
                ),
            )));
        }
        Ok(ServerInfo {
            id: info.id,
            name: info.server_name.unwrap_or_else(|| "Jellyfin".into()),
            version: Some(version),
        })
    }

    /// Signs in with a user name and password, which go to the server and
    /// nowhere else. Fails with `serverWrongPassword` when the server says no.
    pub fn sign_in(&self, user: &str, password: &str) -> Result<SignedIn, Error> {
        let result: AuthenticationResult = self
            .post(
                "/Users/AuthenticateByName",
                &json!({ "Username": user, "Pw": password }),
            )
            .map_err(|error| match error {
                Error::Status {
                    status: 400 | 401 | 403,
                    ..
                } => Error::Invalid(coded(
                    "serverWrongPassword",
                    &[],
                    "The server didn't accept that user name and password",
                )),
                other => other,
            })?;
        result.signed_in()
    }

    /// Whether the server allows Quick Connect (an administrator turns it
    /// on).
    pub fn quick_connect_enabled(&self) -> Result<bool, Error> {
        self.get("/QuickConnect/Enabled")
    }

    /// Starts Quick Connect: the code to type into another signed-in
    /// Jellyfin app, and the secret to ask about it with.
    pub fn quick_connect_start(&self) -> Result<QuickConnect, Error> {
        let started: QuickConnectState =
            self.post("/QuickConnect/Initiate", &json!({}))
                .map_err(|error| match error {
                    Error::Status { status: 401, .. } => Error::Invalid(coded(
                        "serverQuickConnectOff",
                        &[],
                        "Quick Connect is turned off on this server",
                    )),
                    other => other,
                })?;
        Ok(QuickConnect {
            code: started.code,
            secret: started.secret,
        })
    }

    /// Whether the user has approved the code of `secret` yet.
    pub fn quick_connect_approved(&self, secret: &str) -> Result<bool, Error> {
        let state: QuickConnectState = self.get(&format!(
            "/QuickConnect/Connect?secret={}",
            percent_encode(secret)
        ))?;
        Ok(state.authenticated)
    }

    /// Signs in with an approved Quick Connect secret.
    pub fn quick_connect_sign_in(&self, secret: &str) -> Result<SignedIn, Error> {
        let result: AuthenticationResult = self.post(
            "/Users/AuthenticateWithQuickConnect",
            &json!({ "Secret": secret }),
        )?;
        result.signed_in()
    }

    /// Ends this device's session on the server, so the token stops working.
    pub fn sign_out(&self) -> Result<(), Error> {
        self.post_empty("/Sessions/Logout", &json!({})).map(|_| ())
    }

    /// The query for `user_id`'s music, a page of `limit` from `start`.
    fn audio_query(user_id: &str, start: usize, limit: usize) -> String {
        format!(
            "/Items?userId={}&IncludeItemTypes=Audio&Recursive=true&SortBy=SortName\
             &SortOrder=Ascending&StartIndex={start}&Limit={limit}",
            percent_encode(user_id)
        )
    }

    /// A page of `user_id`'s tracks from `start`, changed since
    /// `changed_since` (Unix seconds) if given. Returns the tracks and how
    /// many there are in all.
    pub fn tracks(
        &self,
        user_id: &str,
        start: usize,
        changed_since: Option<i64>,
    ) -> Result<(Vec<ServerTrack>, usize), Error> {
        let mut query = format!(
            "{}&Fields={TRACK_FIELDS}&EnableImageTypes=Primary&EnableUserData=false\
             &EnableTotalRecordCount=true",
            Self::audio_query(user_id, start, PAGE_SIZE)
        );
        if let Some(since) = changed_since {
            query.push_str(&format!(
                "&MinDateLastSaved={}",
                percent_encode(&iso_date(since))
            ));
        }
        let page: Page<Item> = self.get(&query)?;
        let tracks = page
            .items
            .into_iter()
            .filter_map(|item| self.track(item))
            .collect();
        Ok((tracks, page.total))
    }

    /// A page of the ids of all `user_id`'s tracks from `start`, and how
    /// many there are in all: what a sync of changes alone compares with
    /// the library to find what was removed.
    pub fn track_ids(&self, user_id: &str, start: usize) -> Result<(Vec<String>, usize), Error> {
        let query = format!(
            "{}&Fields=&EnableImages=false&EnableUserData=false&EnableTotalRecordCount=true",
            Self::audio_query(user_id, start, ID_PAGE_SIZE)
        );
        let page: Page<IdOnly> = self.get(&query)?;
        Ok((
            page.items.into_iter().map(|item| item.id).collect(),
            page.total,
        ))
    }

    /// `user_id`'s audio playlists, each with its tracks' ids in order.
    pub fn playlists(&self, user_id: &str) -> Result<Vec<ServerPlaylist>, Error> {
        let user = percent_encode(user_id);
        let page: Page<PlaylistItem> = self.get(&format!(
            "/Items?userId={user}&IncludeItemTypes=Playlist&Recursive=true&Fields=&EnableImages=false"
        ))?;
        let mut playlists = Vec::new();
        for playlist in page.items {
            if playlist
                .media_type
                .as_deref()
                .is_some_and(|kind| kind != "Audio")
            {
                continue;
            }
            let mut item_ids = Vec::new();
            loop {
                let entries: Page<IdOnly> = self.get(&format!(
                    "/Playlists/{}/Items?userId={user}&StartIndex={}&Limit={ID_PAGE_SIZE}\
                     &Fields=&EnableImages=false&EnableUserData=false",
                    percent_encode(&playlist.id),
                    item_ids.len()
                ))?;
                let got = entries.items.len();
                item_ids.extend(entries.items.into_iter().map(|item| item.id));
                if got == 0 || item_ids.len() >= entries.total {
                    break;
                }
            }
            playlists.push(ServerPlaylist {
                id: playlist.id,
                name: playlist.name.unwrap_or_default(),
                item_ids,
            });
        }
        Ok(playlists)
    }

    /// Where the original file of track `item_id` is fetched.
    pub fn original_url(&self, item_id: &str) -> String {
        self.url(&format!(
            "/Audio/{}/stream?static=true",
            percent_encode(item_id)
        ))
    }

    /// Where track `item_id` is fetched as MP3 at `kbps`, transcoded by the
    /// server.
    pub fn mp3_url(&self, item_id: &str, kbps: u32) -> String {
        self.url(&format!(
            "/Audio/{}/stream?static=false&container=mp3&audioCodec=mp3&audioBitRate={}\
             &deviceId={}",
            percent_encode(item_id),
            kbps * 1000,
            percent_encode(&self.device.id)
        ))
    }

    /// Tells the server track `item_id` started playing on this device.
    pub fn playing(&self, item_id: &str) -> Result<(), Error> {
        self.post_empty(
            "/Sessions/Playing",
            &json!({
                "ItemId": item_id,
                "PositionTicks": 0,
                "CanSeek": true,
                "IsPaused": false,
                "PlayMethod": "DirectPlay",
            }),
        )
        .map(|_| ())
    }

    /// Tells the server track `item_id` stopped `seconds` in; the server
    /// counts a play by its own rule (most of the track heard).
    pub fn stopped(&self, item_id: &str, seconds: f64) -> Result<(), Error> {
        self.post_empty(
            "/Sessions/Playing/Stopped",
            &json!({
                "ItemId": item_id,
                "PositionTicks": (seconds.max(0.0) * TICKS_PER_SECOND) as i64,
            }),
        )
        .map(|_| ())
    }

    /// A track as the app keeps it; `None` for an item without an id.
    fn track(&self, item: Item) -> Option<ServerTrack> {
        if item.id.is_empty() {
            return None;
        }
        let source = item.media_sources.into_iter().next().unwrap_or_default();
        let stream = source
            .media_streams
            .iter()
            .find(|stream| stream.kind.as_deref() == Some("Audio"))
            .cloned()
            .unwrap_or_default();
        let names = |items: Vec<NameId>| -> Vec<String> {
            items
                .into_iter()
                .filter_map(|item| item.name)
                .filter(|name| !name.trim().is_empty())
                .collect()
        };
        let artists = {
            let named = names(item.artist_items);
            if named.is_empty() {
                item.artists
            } else {
                named
            }
        };
        let album_artists = {
            let named = names(item.album_artists);
            if named.is_empty() {
                item.album_artist.into_iter().collect()
            } else {
                named
            }
        };
        let ids = item.provider_ids;
        let art_url = match (&item.album_id, &item.album_primary_image_tag) {
            (Some(album), Some(tag)) => Some(self.image_url(album, tag)),
            _ => item
                .image_tags
                .primary
                .as_ref()
                .map(|tag| self.image_url(&item.id, tag)),
        };
        let container = source
            .container
            .or(item.container)
            .map(|container| container.to_ascii_lowercase());
        let bitrate = stream.bit_rate.or(source.bitrate);
        Some(ServerTrack {
            id: item.id,
            title: item.name,
            artists,
            album_artists,
            album: item.album,
            track_number: item.index_number,
            disc_number: item.parent_index_number,
            year: item.production_year,
            date: item
                .premiere_date
                .map(|date| date.chars().take(10).collect()),
            duration: item
                .run_time_ticks
                .map_or(0.0, |ticks| ticks as f64 / TICKS_PER_SECOND),
            genres: item.genres,
            container,
            size: source.size.unwrap_or(0),
            bitrate_kbps: bitrate.map(|bits| bits / 1000).filter(|&kbps| kbps > 0),
            sample_rate: stream.sample_rate.unwrap_or(0),
            channels: stream.channels.unwrap_or(0),
            // Picard's "track id" is the recording; newer Jellyfin keeps it
            // as MusicBrainzRecording, older as MusicBrainzTrack.
            recording_mbid: ids.recording.or(ids.track),
            release_mbid: ids.album,
            release_group_mbid: ids.release_group,
            artist_mbid: ids.artist,
            album_artist_mbid: ids.album_artist,
            art_url,
            media_path: None,
            track_gain: item.normalization_gain,
        })
    }

    fn image_url(&self, item_id: &str, tag: &str) -> String {
        self.url(&format!(
            "/Items/{}/Images/Primary?tag={}&maxWidth={IMAGE_WIDTH}&quality=90",
            percent_encode(item_id),
            percent_encode(tag)
        ))
    }
}

/// One user's music on a Jellyfin server, as `sync` reads it.
pub struct Library<'a> {
    pub server: Jellyfin<'a>,
    pub user_id: &'a str,
}

impl ServerLibrary for Library<'_> {
    fn tracks(
        &self,
        start: usize,
        changed_since: Option<i64>,
    ) -> Result<(Vec<ServerTrack>, usize), Error> {
        self.server.tracks(self.user_id, start, changed_since)
    }

    fn track_ids(&self, start: usize) -> Result<(Vec<String>, usize), Error> {
        self.server.track_ids(self.user_id, start)
    }

    fn playlists(&self) -> Result<Vec<ServerPlaylist>, Error> {
        self.server.playlists(self.user_id)
    }
}

/// A Quick Connect sign-in under way.
#[derive(Debug, Clone, PartialEq)]
pub struct QuickConnect {
    /// What the user types into another Jellyfin app.
    pub code: String,
    pub secret: String,
}

/// Whether `version` ("10.10.3") is before `minimum`; an unreadable
/// version is.
fn version_below(version: &str, minimum: (u32, u32)) -> bool {
    let mut parts = version.split('.').map(|part| part.parse::<u32>());
    match (parts.next(), parts.next()) {
        (Some(Ok(major)), Some(Ok(minor))) => (major, minor) < minimum,
        _ => true,
    }
}

/// Unix seconds as the ISO 8601 date and time (UTC) Jellyfin's queries
/// take: "2026-10-10T08:30:00Z".
pub fn iso_date(unix: i64) -> String {
    let days = unix.div_euclid(86400);
    let seconds = unix.rem_euclid(86400);
    // Howard Hinnant's days-to-civil algorithm.
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        seconds / 3600,
        seconds / 60 % 60,
        seconds % 60
    )
}

// What the server sends, as far as it's read.

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct PublicInfo {
    #[serde(default)]
    id: String,
    server_name: Option<String>,
    version: Option<String>,
    product_name: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct AuthenticationResult {
    user: Option<UserDto>,
    access_token: Option<String>,
    server_id: Option<String>,
}

impl AuthenticationResult {
    fn signed_in(self) -> Result<SignedIn, Error> {
        match (self.user, self.access_token) {
            (Some(user), Some(token)) if !user.id.is_empty() && !token.is_empty() => Ok(SignedIn {
                token,
                user_id: user.id,
                user_name: user.name.unwrap_or_default(),
                server_id: self.server_id,
            }),
            _ => Err(Error::Invalid(
                "The server's answer to signing in had no user or token".into(),
            )),
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct UserDto {
    #[serde(default)]
    id: String,
    name: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct QuickConnectState {
    #[serde(default)]
    secret: String,
    #[serde(default)]
    code: String,
    #[serde(default)]
    authenticated: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct Page<T> {
    #[serde(default = "Vec::new")]
    items: Vec<T>,
    #[serde(default, rename = "TotalRecordCount")]
    total: usize,
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct IdOnly {
    id: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct PlaylistItem {
    id: String,
    name: Option<String>,
    media_type: Option<String>,
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "PascalCase", default)]
struct Item {
    id: String,
    name: Option<String>,
    album: Option<String>,
    album_id: Option<String>,
    album_primary_image_tag: Option<String>,
    album_artist: Option<String>,
    album_artists: Vec<NameId>,
    artists: Vec<String>,
    artist_items: Vec<NameId>,
    index_number: Option<u32>,
    parent_index_number: Option<u32>,
    production_year: Option<u32>,
    premiere_date: Option<String>,
    run_time_ticks: Option<i64>,
    genres: Vec<String>,
    container: Option<String>,
    media_sources: Vec<MediaSource>,
    provider_ids: ProviderIds,
    image_tags: ImageTags,
    normalization_gain: Option<f64>,
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "PascalCase", default)]
struct NameId {
    name: Option<String>,
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "PascalCase", default)]
struct MediaSource {
    container: Option<String>,
    size: Option<i64>,
    bitrate: Option<u32>,
    media_streams: Vec<MediaStream>,
}

#[derive(Deserialize, Default, Clone)]
#[serde(rename_all = "PascalCase", default)]
struct MediaStream {
    #[serde(rename = "Type")]
    kind: Option<String>,
    sample_rate: Option<u32>,
    channels: Option<u32>,
    bit_rate: Option<u32>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct ProviderIds {
    #[serde(rename = "MusicBrainzRecording")]
    recording: Option<String>,
    #[serde(rename = "MusicBrainzTrack")]
    track: Option<String>,
    #[serde(rename = "MusicBrainzAlbum")]
    album: Option<String>,
    #[serde(rename = "MusicBrainzReleaseGroup")]
    release_group: Option<String>,
    #[serde(rename = "MusicBrainzArtist")]
    artist: Option<String>,
    #[serde(rename = "MusicBrainzAlbumArtist")]
    album_artist: Option<String>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct ImageTags {
    #[serde(rename = "Primary")]
    primary: Option<String>,
}

#[cfg(test)]
pub mod testing {
    //! A server for tests, answering from the hand-made responses in
    //! `metadata/fixtures/jellyfin/` (made as `manifest.json` says).

    use super::super::Device;
    use crate::metadata::http::testing::FakeTransport;

    pub const ADDRESS: &str = "http://192.168.1.20:8096";
    pub const USER: &str = "5f1b2c3d4e5f60718293a4b5c6d7e8f9";

    pub const PUBLIC_INFO: &str =
        include_str!("../metadata/fixtures/jellyfin/system-info-public.json");
    pub const AUTHENTICATE: &str =
        include_str!("../metadata/fixtures/jellyfin/authenticate-by-name.json");
    pub const TRACKS_1: &str = include_str!("../metadata/fixtures/jellyfin/items-audio-1.json");
    pub const TRACKS_2: &str = include_str!("../metadata/fixtures/jellyfin/items-audio-2.json");
    pub const PLAYLISTS: &str = include_str!("../metadata/fixtures/jellyfin/items-playlists.json");
    pub const PLAYLIST_ITEMS: &str =
        include_str!("../metadata/fixtures/jellyfin/playlist-items.json");
    pub const TRACK_IDS: &str = include_str!("../metadata/fixtures/jellyfin/items-audio-ids.json");

    /// The committed audio fixture a test server serves as a track's file.
    pub const FLAC: &[u8] = include_bytes!("../../../../core/tests/fixtures/flac-44k.flac");

    pub fn device() -> Device {
        Device {
            id: "0123456789abcdef0123456789abcdef".into(),
            name: "Mac".into(),
        }
    }

    pub fn url(path_and_query: &str) -> String {
        format!("{ADDRESS}{path_and_query}")
    }

    /// The URL of a page of the test user's tracks from `start`.
    pub fn tracks_url(start: usize, changed_since: Option<&str>) -> String {
        let mut url = url(&format!(
            "/Items?userId={USER}&IncludeItemTypes=Audio&Recursive=true&SortBy=SortName\
             &SortOrder=Ascending&StartIndex={start}&Limit={}&Fields=MediaSources,Genres,\
             ProviderIds,DateCreated&EnableImageTypes=Primary&EnableUserData=false\
             &EnableTotalRecordCount=true",
            super::PAGE_SIZE
        ));
        if let Some(since) = changed_since {
            url.push_str(&format!("&MinDateLastSaved={since}"));
        }
        url
    }

    /// The URL of a page of the test user's track ids from `start`.
    pub fn track_ids_url(start: usize) -> String {
        url(&format!(
            "/Items?userId={USER}&IncludeItemTypes=Audio&Recursive=true&SortBy=SortName\
             &SortOrder=Ascending&StartIndex={start}&Limit={}&Fields=&EnableImages=false\
             &EnableUserData=false&EnableTotalRecordCount=true",
            super::ID_PAGE_SIZE
        ))
    }

    pub fn playlists_url() -> String {
        url(&format!(
            "/Items?userId={USER}&IncludeItemTypes=Playlist&Recursive=true&Fields=&EnableImages=false"
        ))
    }

    pub fn playlist_items_url(playlist: &str, start: usize) -> String {
        url(&format!(
            "/Playlists/{playlist}/Items?userId={USER}&StartIndex={start}&Limit={}\
             &Fields=&EnableImages=false&EnableUserData=false",
            super::ID_PAGE_SIZE
        ))
    }

    /// Serves the fixtures' library: two pages of tracks, one playlist.
    pub fn serve_library(transport: &FakeTransport) {
        transport.push_status(&tracks_url(0, None), 200, TRACKS_1);
        transport.push_status(&tracks_url(3, None), 200, TRACKS_2);
        transport.push_status(&playlists_url(), 200, PLAYLISTS);
        transport.push_status(
            &playlist_items_url("a0b1c2d3e4f5a6b7c8d9e0f1a2b3c4d5", 0),
            200,
            PLAYLIST_ITEMS,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::testing::*;
    use super::*;
    use crate::metadata::http::testing::fake_client;

    fn server<'a>(http: &'a Client, device: &'a Device, token: Option<&'a str>) -> Jellyfin<'a> {
        Jellyfin {
            http,
            address: ADDRESS,
            device,
            token,
        }
    }

    #[test]
    fn sends_the_mediabrowser_header() {
        let (http, _, _) = fake_client();
        let device = device();
        let header = server(&http, &device, Some("tok\"en")).authorization();
        assert_eq!(
            header,
            format!(
                "MediaBrowser Client=\"ano-mp\", Device=\"Mac\", \
                 DeviceId=\"0123456789abcdef0123456789abcdef\", Version=\"{}\", Token=\"token\"",
                crate::anomp::version()
            )
        );
        assert!(!server(&http, &device, None)
            .authorization()
            .contains("Token"));
    }

    #[test]
    fn reads_who_the_server_is() {
        let (http, transport, _) = fake_client();
        let device = device();
        transport.push_status(&url("/System/Info/Public"), 200, PUBLIC_INFO);
        let info = server(&http, &device, None).public_info().unwrap();
        assert_eq!(info.name, "Living Room");
        assert_eq!(info.version.as_deref(), Some("10.10.7"));
        assert_eq!(info.id, "4c5e9a1b2d3f4a5b6c7d8e9f0a1b2c3d");

        transport.push_status(
            &url("/System/Info/Public"),
            200,
            r#"{"Id":"x","ServerName":"Old","Version":"10.8.13","ProductName":"Jellyfin Server"}"#,
        );
        let error = server(&http, &device, None).public_info().unwrap_err();
        assert!(crate::coded::is(&error, "serverTooOld"), "{error}");

        transport.push_status(&url("/System/Info/Public"), 200, "<html>router</html>");
        let error = server(&http, &device, None).public_info().unwrap_err();
        assert!(crate::coded::is(&error, "serverNotJellyfin"), "{error}");
        transport.push_status(&url("/System/Info/Public"), 404, "");
        let error = server(&http, &device, None).public_info().unwrap_err();
        assert!(crate::coded::is(&error, "serverNotJellyfin"), "{error}");
    }

    #[test]
    fn signs_in_with_a_password() {
        let (http, transport, _) = fake_client();
        let device = device();
        transport.push_status(&url("/Users/AuthenticateByName"), 200, AUTHENTICATE);
        let signed = server(&http, &device, None).sign_in("mark", "pw").unwrap();
        assert_eq!(signed.user_id, USER);
        assert_eq!(signed.user_name, "mark");
        assert_eq!(signed.token, "6a1f0c9e8d7b4a3f2e1d0c9b8a7f6e5d");
        let body: serde_json::Value =
            serde_json::from_str(&transport.bodies.lock().unwrap()[0]).unwrap();
        assert_eq!(body, json!({"Username": "mark", "Pw": "pw"}));

        transport.push_status(&url("/Users/AuthenticateByName"), 401, "");
        let error = server(&http, &device, None)
            .sign_in("mark", "no")
            .unwrap_err();
        assert!(crate::coded::is(&error, "serverWrongPassword"), "{error}");
    }

    #[test]
    fn signs_in_with_quick_connect() {
        let (http, transport, _) = fake_client();
        let device = device();
        let jellyfin = server(&http, &device, None);
        transport.push_status(&url("/QuickConnect/Enabled"), 200, "true");
        assert!(jellyfin.quick_connect_enabled().unwrap());
        transport.push_status(
            &url("/QuickConnect/Initiate"),
            200,
            r#"{"Authenticated":false,"Secret":"s3cr3t","Code":"482913","DeviceId":"d"}"#,
        );
        let started = jellyfin.quick_connect_start().unwrap();
        assert_eq!(started.code, "482913");
        let connect = url("/QuickConnect/Connect?secret=s3cr3t");
        transport.push_status(&connect, 200, r#"{"Authenticated":false}"#);
        transport.push_status(&connect, 200, r#"{"Authenticated":true}"#);
        assert!(!jellyfin.quick_connect_approved("s3cr3t").unwrap());
        assert!(jellyfin.quick_connect_approved("s3cr3t").unwrap());
        transport.push_status(
            &url("/Users/AuthenticateWithQuickConnect"),
            200,
            AUTHENTICATE,
        );
        assert_eq!(
            jellyfin.quick_connect_sign_in("s3cr3t").unwrap().user_id,
            USER
        );

        transport.push_status(&url("/QuickConnect/Initiate"), 401, "");
        let error = jellyfin.quick_connect_start().unwrap_err();
        assert!(crate::coded::is(&error, "serverQuickConnectOff"), "{error}");
    }

    #[test]
    fn reads_tracks_a_page_at_a_time() {
        let (http, transport, _) = fake_client();
        let device = device();
        let jellyfin = server(&http, &device, Some("t"));
        transport.push_status(&tracks_url(0, None), 200, TRACKS_1);
        let (tracks, total) = jellyfin.tracks(USER, 0, None).unwrap();
        assert_eq!(total, 4);
        assert_eq!(tracks.len(), 3);
        let first = &tracks[0];
        assert_eq!(first.id, "b1f3a0e2c4d5e6f708192a3b4c5d6e7f");
        assert_eq!(first.title.as_deref(), Some("Chirp One"));
        assert_eq!(first.artists, ["Test Signal", "Guest"]);
        assert_eq!(first.album_artists, ["Test Signal"]);
        assert_eq!(first.album.as_deref(), Some("Chirps"));
        assert_eq!((first.track_number, first.disc_number), (Some(1), Some(1)));
        assert_eq!(first.year, Some(2024));
        assert_eq!(first.date.as_deref(), Some("2024-03-01"));
        assert!((first.duration - 1.5).abs() < 1e-9);
        assert_eq!(first.genres, ["Electronic", "Test"]);
        assert_eq!(first.container.as_deref(), Some("flac"));
        assert_eq!(first.size, 36202);
        assert_eq!(first.bitrate_kbps, Some(193));
        assert_eq!((first.sample_rate, first.channels), (44100, 2));
        assert_eq!(
            first.recording_mbid.as_deref(),
            Some("7c1b4b62-8a3c-4b8e-9d5e-0c3a6f2b1e4d")
        );
        assert_eq!(
            first.release_mbid.as_deref(),
            Some("3f2e1d0c-9b8a-4f6e-8d7c-1b2a3c4d5e6f")
        );
        assert_eq!(first.track_gain, Some(-6.5));
        assert_eq!(
            first.art_url.as_deref(),
            Some(
                "http://192.168.1.20:8096/Items/c0ffee00c0ffee00c0ffee00c0ffee00/Images/Primary\
                 ?tag=9a8b7c6d&maxWidth=1200&quality=90"
            )
        );
        // The token goes in the header, never the URL.
        assert!(transport.urls().iter().all(|url| !url.contains("Token")));
        assert!(transport.auths.lock().unwrap()[0]
            .as_deref()
            .is_some_and(|auth| auth.ends_with("Token=\"t\"")));

        // Changes alone.
        let since = "2025-10-01T00%3A00%3A00Z";
        transport.push_status(&tracks_url(0, Some(since)), 200, TRACKS_2);
        let (changed, _) = jellyfin.tracks(USER, 0, Some(1_759_276_800)).unwrap();
        assert_eq!(changed.len(), 1);
        assert_eq!(changed[0].container.as_deref(), Some("dsf"));
    }

    #[test]
    fn lists_every_track_id() {
        let (http, transport, _) = fake_client();
        let device = device();
        transport.push_status(&track_ids_url(0), 200, TRACK_IDS);
        let (ids, total) = server(&http, &device, Some("t"))
            .track_ids(USER, 0)
            .unwrap();
        assert_eq!(total, 3);
        assert_eq!(ids[1], "e4b6d3f5a7b8c9d0e1f2a3b4c5d6e7f8");
    }

    #[test]
    fn reads_playlists_with_their_tracks() {
        let (http, transport, _) = fake_client();
        let device = device();
        serve_library(&transport);
        let playlists = server(&http, &device, Some("t")).playlists(USER).unwrap();
        assert_eq!(playlists.len(), 1, "the video playlist is left out");
        assert_eq!(playlists[0].name, "Road Trip");
        assert_eq!(
            playlists[0].item_ids,
            [
                "d3a5c2e4f6a7b8c9d0e1f2a3b4c5d6e7",
                "b1f3a0e2c4d5e6f708192a3b4c5d6e7f"
            ]
        );
    }

    #[test]
    fn fetch_and_report_urls() {
        let (http, transport, _) = fake_client();
        let device = device();
        let jellyfin = server(&http, &device, Some("t"));
        assert_eq!(
            jellyfin.original_url("abc"),
            "http://192.168.1.20:8096/Audio/abc/stream?static=true"
        );
        assert_eq!(
            jellyfin.mp3_url("abc", 192),
            "http://192.168.1.20:8096/Audio/abc/stream?static=false&container=mp3\
             &audioCodec=mp3&audioBitRate=192000&deviceId=0123456789abcdef0123456789abcdef"
        );
        transport.push_status(&url("/Sessions/Playing"), 204, "");
        transport.push_status(&url("/Sessions/Playing/Stopped"), 204, "");
        jellyfin.playing("abc").unwrap();
        jellyfin.stopped("abc", 12.5).unwrap();
        let bodies = transport.bodies.lock().unwrap().clone();
        let stopped: serde_json::Value = serde_json::from_str(&bodies[1]).unwrap();
        assert_eq!(
            stopped,
            json!({"ItemId": "abc", "PositionTicks": 125_000_000})
        );
    }

    /// Needs a test server (7e.1): `JELLYFIN_URL` (such as
    /// http://192.168.1.20:8096), `JELLYFIN_USER` and `JELLYFIN_PASSWORD`.
    /// Signs in, reads a page of tracks and their playlists, fetches the
    /// first track's file, and signs out.
    #[test]
    #[ignore = "contacts a Jellyfin server"]
    fn live_jellyfin() {
        use crate::metadata::http::{SystemClock, UreqTransport};
        let address = std::env::var("JELLYFIN_URL").expect("JELLYFIN_URL is set");
        let user = std::env::var("JELLYFIN_USER").expect("JELLYFIN_USER is set");
        let password = std::env::var("JELLYFIN_PASSWORD").unwrap_or_default();
        let address = super::super::address::parse(&address).unwrap();
        let http = Client::new(Box::new(UreqTransport::new()), Box::new(SystemClock));
        let device = device();
        let anonymous = server(&http, &device, None);
        let anonymous = Jellyfin {
            address: &address,
            ..anonymous
        };
        let info = anonymous.public_info().unwrap();
        assert!(!version_below(
            info.version.as_deref().unwrap_or(""),
            MIN_VERSION
        ));
        let signed = anonymous.sign_in(&user, &password).unwrap();
        let jellyfin = Jellyfin {
            token: Some(&signed.token),
            ..anonymous
        };
        let (tracks, total) = jellyfin.tracks(&signed.user_id, 0, None).unwrap();
        assert!(total == 0 || !tracks.is_empty());
        jellyfin.playlists(&signed.user_id).unwrap();
        if let Some(track) = tracks.first() {
            let mut sink = Vec::new();
            http.download_with_headers(
                &jellyfin.original_url(&track.id),
                &[("Authorization", &jellyfin.authorization())],
                &mut sink,
                1 << 30,
                &|| false,
            )
            .unwrap();
            assert!(!sink.is_empty());
        }
        jellyfin.sign_out().unwrap();
    }

    #[test]
    fn dates_and_versions() {
        assert_eq!(iso_date(0), "1970-01-01T00:00:00Z");
        assert_eq!(iso_date(1_759_276_800), "2025-10-01T00:00:00Z");
        assert_eq!(iso_date(951_825_599), "2000-02-29T11:59:59Z");
        assert!(version_below("10.8.13", (10, 9)));
        assert!(!version_below("10.9.0", (10, 9)));
        assert!(!version_below("10.10.7", (10, 9)));
        assert!(!version_below("11.0.0", (10, 9)));
        assert!(version_below("", (10, 9)));
    }
}
