//! The Plex Media Server's API (PLAN.md Phase 7f), as far as the app uses
//! it: who the server is, its music libraries, their tracks a page at a
//! time, their audio playlists, where to fetch a track or a picture, and
//! reporting what plays. plex.tv signs in and lists the servers
//! (`plextv`); this talks to the server itself.
//!
//! Every request goes through `http::Client` with Plex's headers
//! (`client_headers`): this install's id, the app's name and version, and
//! the server's token in `X-Plex-Token`, never in a URL. Paging is by
//! query parameters (`X-Plex-Container-Start`, `-Size`), which Plex takes
//! as well as headers. Responses are parsed into the neutral types of
//! `servers`, so the sync doesn't know which kind of server it reads.
//!
//! A server answers at several addresses (its own network, the internet,
//! Plex's relay), which change: `connect` finds one that answers as the
//! server, the one that answered last first and the relay last, and only
//! while the user allows it.

use serde::Deserialize;
use serde_json::json;

use super::address::{self, Resolver};
use super::plextv::Connection;
use super::sync::ServerLibrary;
use super::{Device, Section, ServerPlaylist, ServerTrack};
use crate::coded::coded;
use crate::metadata::http::{percent_encode, Client, JSON_LIMIT};
use crate::metadata::Error;

/// Tracks asked for per page of a sync.
pub const PAGE_SIZE: usize = 500;

/// Track ids asked for per page when looking for removed tracks.
pub const ID_PAGE_SIZE: usize = 2000;

/// The widest picture asked for: the image cache's thumbnails are made
/// from it (H17), and "full" size shows it.
const IMAGE_WIDTH: u32 = 1200;

/// What a picture's key in the image cache starts with: `plex://<machine
/// id><thumb path>`, which names no address, since a server's changes.
const ART_SCHEME: &str = "plex://";

/// Asks the server to transcode a track to MP3 over plain http (the
/// default profile has no MP3 target). To check against a test server
/// (7f.1).
pub const MP3_PROFILE: &str = "add-transcode-target(type=musicProfile&context=streaming\
     &protocol=http&container=mp3&audioCodec=mp3)";

/// The platform Plex is told the app runs on.
fn platform() -> &'static str {
    if cfg!(target_os = "macos") {
        "macOS"
    } else if cfg!(target_os = "ios") {
        "iOS"
    } else if cfg!(target_os = "windows") {
        "Windows"
    } else {
        "Linux"
    }
}

/// The headers that say who is asking, to plex.tv and to servers alike:
/// this install's id, the app's name and version, the device, and the
/// token once there is one.
pub fn client_headers(device: &Device, token: Option<&str>) -> Vec<(&'static str, String)> {
    let mut headers = vec![
        ("X-Plex-Client-Identifier", device.id.clone()),
        ("X-Plex-Product", "ano-mp".to_owned()),
        ("X-Plex-Version", crate::anomp::version()),
        ("X-Plex-Device-Name", device.name.clone()),
        ("X-Plex-Platform", platform().to_owned()),
    ];
    if let Some(token) = token {
        headers.push(("X-Plex-Token", token.to_owned()));
    }
    headers
}

/// A conversation with a server at one of its addresses.
#[derive(Clone, Copy)]
pub struct Plex<'a> {
    pub http: &'a Client,
    /// One of the server's connections' `uri`.
    pub address: &'a str,
    pub device: &'a Device,
    /// The server's access token (`plextv::Resource::access_token`).
    pub token: Option<&'a str>,
}

/// Who answers at an address.
#[derive(Debug, Clone, PartialEq)]
pub struct Identity {
    pub machine_id: String,
    pub version: Option<String>,
}

impl Plex<'_> {
    pub fn headers(&self) -> Vec<(&'static str, String)> {
        client_headers(self.device, self.token)
    }

    fn url(&self, path_and_query: &str) -> String {
        format!("{}{path_and_query}", self.address)
    }

    fn get<T: for<'de> Deserialize<'de>>(&self, path_and_query: &str) -> Result<T, Error> {
        let headers = self.headers();
        let headers: Vec<(&str, &str)> = headers.iter().map(|(n, v)| (*n, v.as_str())).collect();
        let response = self.http.get_with_headers(
            &self.url(path_and_query),
            "application/json",
            &headers,
            JSON_LIMIT,
        )?;
        let body: Body<T> = serde_json::from_slice(&response.body)
            .map_err(|error| Error::Invalid(format!("{path_and_query}: {error}")))?;
        Ok(body.media_container)
    }

    /// Who answers here; needs no token. Fails with `serverNotPlex` when
    /// something else does.
    pub fn identity(&self) -> Result<Identity, Error> {
        let identity: IdentityDto = self.get("/identity").map_err(|error| match error {
            Error::Invalid(_) | Error::Status { status: 404, .. } => Error::Invalid(not_plex()),
            other => other,
        })?;
        if identity.machine_identifier.is_empty() {
            return Err(Error::Invalid(not_plex()));
        }
        Ok(Identity {
            machine_id: identity.machine_identifier,
            version: identity.version,
        })
    }

    /// The server's music libraries.
    pub fn music_sections(&self) -> Result<Vec<Section>, Error> {
        let sections: Directories = self.get("/library/sections")?;
        Ok(sections
            .directory
            .into_iter()
            .filter(|section| section.kind == "artist" && !section.key.is_empty())
            .map(|section| Section {
                key: section.key,
                name: section.title.unwrap_or_else(|| "Music".into()),
            })
            .collect())
    }

    /// The query for a page of `limit` of library `section`'s tracks from
    /// `start`.
    fn tracks_query(section: &str, start: usize, limit: usize) -> String {
        format!(
            "/library/sections/{}/all?type=10&X-Plex-Container-Start={start}\
             &X-Plex-Container-Size={limit}",
            percent_encode(section)
        )
    }

    /// A page of library `section`'s tracks from `start`, changed since
    /// `changed_since` (Unix seconds) if given, as server `machine_id` has
    /// them. Returns the tracks and how many there are in all.
    pub fn tracks(
        &self,
        machine_id: &str,
        section: &str,
        start: usize,
        changed_since: Option<i64>,
    ) -> Result<(Vec<ServerTrack>, usize), Error> {
        let mut query = format!(
            "{}&includeGuids=1",
            Self::tracks_query(section, start, PAGE_SIZE)
        );
        if let Some(since) = changed_since {
            // `updatedAt>=…`, the `>` encoded.
            query.push_str(&format!("&updatedAt%3E={since}"));
        }
        let page: Page = self.get(&query)?;
        let total = page.total_size.unwrap_or(page.metadata.len() + start);
        let tracks = page
            .metadata
            .into_iter()
            .filter_map(|item| track(machine_id, item))
            .collect();
        Ok((tracks, total))
    }

    /// A page of the ids of all library `section`'s tracks from `start`,
    /// and how many there are in all.
    pub fn track_ids(&self, section: &str, start: usize) -> Result<(Vec<String>, usize), Error> {
        let page: Page = self.get(&Self::tracks_query(section, start, ID_PAGE_SIZE))?;
        let total = page.total_size.unwrap_or(page.metadata.len() + start);
        Ok((
            page.metadata
                .into_iter()
                .map(|item| item.rating_key)
                .filter(|id| !id.is_empty())
                .collect(),
            total,
        ))
    }

    /// Library `section`'s audio playlists, each with its tracks' ids in
    /// order.
    pub fn playlists(&self, section: &str) -> Result<Vec<ServerPlaylist>, Error> {
        let page: Page = self.get(&format!(
            "/playlists?playlistType=audio&sectionID={}",
            percent_encode(section)
        ))?;
        let mut playlists = Vec::new();
        for playlist in page.metadata {
            if playlist.rating_key.is_empty() {
                continue;
            }
            let mut item_ids = Vec::new();
            loop {
                let entries: Page = self.get(&format!(
                    "/playlists/{}/items?X-Plex-Container-Start={}\
                     &X-Plex-Container-Size={ID_PAGE_SIZE}",
                    percent_encode(&playlist.rating_key),
                    item_ids.len()
                ))?;
                let got = entries.metadata.len();
                item_ids.extend(entries.metadata.into_iter().map(|item| item.rating_key));
                if got == 0 || item_ids.len() >= entries.total_size.unwrap_or(0) {
                    break;
                }
            }
            playlists.push(ServerPlaylist {
                id: playlist.rating_key,
                name: playlist.title.unwrap_or_default(),
                item_ids,
            });
        }
        Ok(playlists)
    }

    /// Where the original file at `media_path` (a part's key) is fetched;
    /// `None` for a path that isn't a part's.
    pub fn original_url(&self, media_path: &str) -> Option<String> {
        (media_path.starts_with("/library/parts/") && !media_path.contains(['?', '#', '\\']))
            .then(|| self.url(media_path))
    }

    /// Where track `rating_key` is fetched as MP3 at `kbps`, transcoded by
    /// the server (with `MP3_PROFILE`).
    pub fn mp3_url(&self, rating_key: &str, kbps: u32) -> String {
        self.url(&format!(
            "/music/:/transcode/universal/start.mp3?path={}&mediaIndex=0&partIndex=0\
             &protocol=http&directPlay=0&directStream=0&musicBitrate={kbps}\
             &maxAudioBitrate={kbps}&session={}",
            percent_encode(&format!("/library/metadata/{rating_key}")),
            percent_encode(&self.device.id)
        ))
    }

    /// Where the picture kept under `key` (`art_key`) is fetched, sized
    /// by the server; `None` for a key that isn't a Plex picture's.
    pub fn art_url(&self, key: &str) -> Option<String> {
        let path = art_path(key)?;
        Some(self.url(&format!(
            "/photo/:/transcode?width={IMAGE_WIDTH}&height={IMAGE_WIDTH}&minSize=1&upscale=0\
             &url={}",
            percent_encode(path)
        )))
    }

    /// Tells the server where track `rating_key` (`duration` seconds long)
    /// is: `state` is "playing" or "stopped". The server counts a play by
    /// its own rule (most of the track heard).
    pub fn timeline(
        &self,
        rating_key: &str,
        state: &str,
        seconds: f64,
        duration: f64,
    ) -> Result<(), Error> {
        let millis = |seconds: f64| (seconds.max(0.0) * 1000.0).round() as i64;
        let headers = self.headers();
        let headers: Vec<(&str, &str)> = headers.iter().map(|(n, v)| (*n, v.as_str())).collect();
        self.http.get_with_headers(
            &self.url(&format!(
                "/:/timeline?ratingKey={}&key={}&state={state}&time={}&duration={}",
                percent_encode(rating_key),
                percent_encode(&format!("/library/metadata/{rating_key}")),
                millis(seconds),
                millis(duration)
            )),
            "application/json",
            &headers,
            JSON_LIMIT,
        )?;
        Ok(())
    }
}

/// The key a server's picture is kept under in the image cache.
pub fn art_key(machine_id: &str, thumb: &str) -> String {
    format!("{ART_SCHEME}{machine_id}{thumb}")
}

/// The path on the server of the picture kept under `key`.
fn art_path(key: &str) -> Option<&str> {
    let rest = key.strip_prefix(ART_SCHEME)?;
    let path = &rest[rest.find('/')?..];
    (path.starts_with("/library/") && !path.contains(['?', '#'])).then_some(path)
}

fn not_plex() -> String {
    coded(
        "serverNotPlex",
        &[],
        "Nothing at that address answers as a Plex server",
    )
}

/// A track as the app keeps it; `None` for an item without an id.
fn track(machine_id: &str, item: Item) -> Option<ServerTrack> {
    if item.rating_key.is_empty() {
        return None;
    }
    let media = item.media.into_iter().next().unwrap_or_default();
    let part = media.part.into_iter().next().unwrap_or_default();
    let album_artist = item
        .grandparent_title
        .filter(|name| !name.trim().is_empty());
    // A track's own artist, where it differs from the album's.
    let artist = item
        .original_title
        .filter(|name| !name.trim().is_empty())
        .or_else(|| album_artist.clone());
    let recording_mbid = item.guid.iter().find_map(|guid| {
        guid.id
            .strip_prefix("mbid://")
            .filter(|id| id.len() == 36)
            .map(str::to_owned)
    });
    let thumb = item.parent_thumb.or(item.thumb);
    Some(ServerTrack {
        id: item.rating_key,
        title: item.title,
        artists: artist.into_iter().collect(),
        album_artists: album_artist.into_iter().collect(),
        album: item.parent_title,
        track_number: item.index,
        disc_number: item.parent_index,
        year: item.parent_year.or(item.year),
        date: None,
        duration: item.duration.or(media.duration).unwrap_or(0) as f64 / 1000.0,
        genres: item.genre.into_iter().map(|genre| genre.tag).collect(),
        container: part
            .container
            .or(media.container)
            .map(|container| container.to_ascii_lowercase()),
        size: part.size.unwrap_or(0),
        bitrate_kbps: media.bitrate.filter(|&kbps| kbps > 0),
        sample_rate: 0,
        channels: media.audio_channels.unwrap_or(0),
        recording_mbid,
        art_url: thumb.map(|thumb| art_key(machine_id, &thumb)),
        media_path: part.key,
        ..ServerTrack::default()
    })
}

/// One music library on a Plex server, as `sync` reads it.
pub struct Library<'a> {
    pub server: Plex<'a>,
    pub machine_id: &'a str,
    pub section: &'a str,
}

impl ServerLibrary for Library<'_> {
    fn tracks(
        &self,
        start: usize,
        changed_since: Option<i64>,
    ) -> Result<(Vec<ServerTrack>, usize), Error> {
        self.server
            .tracks(self.machine_id, self.section, start, changed_since)
    }

    fn track_ids(&self, start: usize) -> Result<(Vec<String>, usize), Error> {
        self.server.track_ids(self.section, start)
    }

    fn playlists(&self) -> Result<Vec<ServerPlaylist>, Error> {
        self.server.playlists(self.section)
    }
}

/// The order to try a server's addresses in: the one that answered last,
/// then those on its own network, then the rest, then Plex's relay if
/// `allow_relay`; each once, and only those the app may send a token to
/// (`address::check`).
pub fn candidates(
    last: &str,
    connections: &[Connection],
    allow_relay: bool,
    resolver: &impl Resolver,
) -> Vec<String> {
    let mut ordered: Vec<&str> = Vec::new();
    let is_relay = |uri: &str| {
        connections
            .iter()
            .any(|connection| connection.relay && connection.uri == uri)
    };
    if !last.is_empty() && (allow_relay || !is_relay(last)) {
        ordered.push(last);
    }
    for pass in 0..3 {
        for connection in connections {
            let wanted = match pass {
                0 => connection.local && !connection.relay,
                1 => !connection.local && !connection.relay,
                _ => connection.relay && allow_relay,
            };
            if wanted && !ordered.contains(&connection.uri.as_str()) {
                ordered.push(&connection.uri);
            }
        }
    }
    ordered
        .into_iter()
        .filter(|uri| address::check(uri, resolver).is_ok())
        .map(str::to_owned)
        .collect()
}

/// The first of the server `machine_id`'s addresses (`candidates`) that
/// answers as that server, and what it says of itself. Fails with
/// `plexRelayOff` when only the relay is left and the user doesn't allow
/// it, and as the last address failed otherwise.
#[allow(clippy::too_many_arguments)]
pub fn connect(
    http: &Client,
    device: &Device,
    machine_id: &str,
    server_name: &str,
    last: &str,
    connections: &[Connection],
    allow_relay: bool,
    resolver: &impl Resolver,
) -> Result<(String, Identity), Error> {
    let mut failure = Error::Offline(format!("{server_name} has no address to try"));
    for uri in candidates(last, connections, allow_relay, resolver) {
        let server = Plex {
            http,
            address: &uri,
            device,
            token: None,
        };
        match server.identity() {
            Ok(identity) if identity.machine_id == machine_id => return Ok((uri, identity)),
            Ok(_) => log::debug!("another Plex server answers at {uri}"),
            Err(error) => {
                log::debug!("Plex server {server_name} doesn't answer at {uri}: {error}");
                failure = error;
            }
        }
    }
    if !allow_relay && connections.iter().any(|connection| connection.relay) {
        return Err(Error::Invalid(coded(
            "plexRelayOff",
            &[("server", json!(server_name))],
            format!(
                "{server_name} can't be reached directly; to play through Plex's relay, \
                 turn on \"Use Plex's relay\" in Settings › Servers"
            ),
        )));
    }
    Err(failure)
}

/// A server's refusal to send a track while away from its network, where
/// Plex wants a Plex Pass (PLAN.md Phase 7f): `plexPassNeeded`, in place of
/// a plain error.
pub fn pass_needed(server: &str) -> String {
    coded(
        "plexPassNeeded",
        &[("server", json!(server))],
        format!(
            "{server} won't stream this away from its network; Plex may need a Plex Pass \
             for that. It still plays on the server's own network."
        ),
    )
}

// What the server sends, as far as it's read: everything is inside a
// `MediaContainer`.

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct Body<T> {
    media_container: T,
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
struct IdentityDto {
    machine_identifier: String,
    version: Option<String>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct Directories {
    #[serde(rename = "Directory")]
    directory: Vec<Directory>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct Directory {
    key: String,
    #[serde(rename = "type")]
    kind: String,
    title: Option<String>,
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
struct Page {
    total_size: Option<usize>,
    #[serde(rename = "Metadata")]
    metadata: Vec<Item>,
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
struct Item {
    rating_key: String,
    title: Option<String>,
    original_title: Option<String>,
    parent_title: Option<String>,
    grandparent_title: Option<String>,
    index: Option<u32>,
    parent_index: Option<u32>,
    year: Option<u32>,
    parent_year: Option<u32>,
    /// Milliseconds.
    duration: Option<i64>,
    thumb: Option<String>,
    parent_thumb: Option<String>,
    #[serde(rename = "Media")]
    media: Vec<Media>,
    #[serde(rename = "Genre")]
    genre: Vec<Tag>,
    #[serde(rename = "Guid")]
    guid: Vec<Guid>,
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
struct Media {
    duration: Option<i64>,
    /// kbit/s.
    bitrate: Option<u32>,
    audio_channels: Option<u32>,
    container: Option<String>,
    #[serde(rename = "Part")]
    part: Vec<Part>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct Part {
    key: Option<String>,
    size: Option<i64>,
    container: Option<String>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct Tag {
    tag: String,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct Guid {
    id: String,
}

#[cfg(test)]
pub mod testing {
    //! A Plex server for tests, answering from the hand-made responses in
    //! `metadata/fixtures/plex/` (made as `manifest.json` says).

    use super::super::plextv::Connection;
    use crate::metadata::http::testing::FakeTransport;

    /// The test server's local https connection.
    pub const ADDRESS: &str = "https://192-168-1-20.a1b2c3.plex.direct:32400";
    pub const REMOTE: &str = "https://203-0-113-9.a1b2c3.plex.direct:32400";
    pub const RELAY: &str = "https://198-51-100-4.a1b2c3.plex.direct:8443";
    pub const MACHINE: &str = "9d3f1c7a5b2e4f6081a3c5e7f9b1d3e5a7c9e1f3";
    pub const ACCOUNT: &str = "31415926";
    pub const SERVER_TOKEN: &str = "srvTok3n8Kq2Lm5Np7Rs";

    pub const IDENTITY: &str = include_str!("../metadata/fixtures/plex/identity.json");
    pub const SECTIONS: &str = include_str!("../metadata/fixtures/plex/sections.json");
    pub const TRACKS_1: &str = include_str!("../metadata/fixtures/plex/tracks-1.json");
    pub const TRACKS_2: &str = include_str!("../metadata/fixtures/plex/tracks-2.json");
    pub const PLAYLISTS: &str = include_str!("../metadata/fixtures/plex/playlists.json");
    pub const PLAYLIST_ITEMS: &str = include_str!("../metadata/fixtures/plex/playlist-items.json");

    pub fn connections() -> Vec<Connection> {
        vec![
            Connection {
                uri: ADDRESS.into(),
                local: true,
                relay: false,
            },
            Connection {
                uri: REMOTE.into(),
                local: false,
                relay: false,
            },
            Connection {
                uri: RELAY.into(),
                local: false,
                relay: true,
            },
        ]
    }

    pub fn url(path_and_query: &str) -> String {
        format!("{ADDRESS}{path_and_query}")
    }

    /// The URL of a page of library `section`'s tracks from `start`.
    pub fn tracks_url(section: &str, start: usize, changed_since: Option<i64>) -> String {
        let mut url = url(&format!(
            "/library/sections/{section}/all?type=10&X-Plex-Container-Start={start}\
             &X-Plex-Container-Size={}&includeGuids=1",
            super::PAGE_SIZE
        ));
        if let Some(since) = changed_since {
            url.push_str(&format!("&updatedAt%3E={since}"));
        }
        url
    }

    pub fn playlists_url(section: &str) -> String {
        url(&format!(
            "/playlists?playlistType=audio&sectionID={section}"
        ))
    }

    pub fn playlist_items_url(playlist: &str, start: usize) -> String {
        url(&format!(
            "/playlists/{playlist}/items?X-Plex-Container-Start={start}\
             &X-Plex-Container-Size={}",
            super::ID_PAGE_SIZE
        ))
    }

    /// Serves library 1 of the fixtures: two pages of tracks, one playlist.
    pub fn serve_library(transport: &FakeTransport) {
        transport.push_status(&tracks_url("1", 0, None), 200, TRACKS_1);
        transport.push_status(&tracks_url("1", 2, None), 200, TRACKS_2);
        transport.push_status(&playlists_url("1"), 200, PLAYLISTS);
        transport.push_status(&playlist_items_url("5001", 0), 200, PLAYLIST_ITEMS);
    }
}

#[cfg(test)]
mod tests {
    use super::testing::*;
    use super::*;
    use crate::metadata::http::testing::fake_client;
    use crate::servers::address::testing::FakeResolver;
    use crate::servers::jellyfin::testing::device;

    fn server<'a>(http: &'a Client, device: &'a Device) -> Plex<'a> {
        Plex {
            http,
            address: ADDRESS,
            device,
            token: Some(SERVER_TOKEN),
        }
    }

    #[test]
    fn reads_who_answers_and_its_music_libraries() {
        let (http, transport, _) = fake_client();
        let device = device();
        transport.push_status(&url("/identity"), 200, IDENTITY);
        let identity = server(&http, &device).identity().unwrap();
        assert_eq!(identity.machine_id, MACHINE);
        assert_eq!(identity.version.as_deref(), Some("1.41.9.9961-46083195d"));
        transport.push_status(&url("/identity"), 200, "<html>router</html>");
        let error = server(&http, &device).identity().unwrap_err();
        assert!(crate::coded::is(&error, "serverNotPlex"), "{error}");

        transport.push_status(&url("/library/sections"), 200, SECTIONS);
        let sections = server(&http, &device).music_sections().unwrap();
        assert_eq!(
            sections,
            [
                Section {
                    key: "1".into(),
                    name: "Music".into()
                },
                Section {
                    key: "4".into(),
                    name: "Live Recordings".into()
                }
            ],
            "films aren't music"
        );
    }

    #[test]
    fn reads_tracks_a_page_at_a_time() {
        let (http, transport, _) = fake_client();
        let device = device();
        let plex = server(&http, &device);
        transport.push_status(&tracks_url("1", 0, None), 200, TRACKS_1);
        let (tracks, total) = plex.tracks(MACHINE, "1", 0, None).unwrap();
        assert_eq!(total, 3);
        assert_eq!(tracks.len(), 2);
        let first = &tracks[0];
        assert_eq!(first.id, "1001");
        assert_eq!(first.title.as_deref(), Some("Chirp One"));
        assert_eq!(first.artists, ["Test Signal feat. Guest"]);
        assert_eq!(first.album_artists, ["Test Signal"]);
        assert_eq!(first.album.as_deref(), Some("Chirps"));
        assert_eq!((first.track_number, first.disc_number), (Some(1), Some(1)));
        assert_eq!(first.year, Some(2024));
        assert!((first.duration - 1.5).abs() < 1e-9);
        assert_eq!(first.genres, ["Electronic"]);
        assert_eq!(first.container.as_deref(), Some("flac"));
        assert_eq!(first.size, 36202);
        assert_eq!(first.bitrate_kbps, Some(193));
        assert_eq!(first.channels, 2);
        assert_eq!(
            first.recording_mbid.as_deref(),
            Some("7c1b4b62-8a3c-4b8e-9d5e-0c3a6f2b1e4d")
        );
        assert_eq!(
            first.media_path.as_deref(),
            Some("/library/parts/827/1712000000/file.flac")
        );
        assert_eq!(
            first.art_url.as_deref(),
            Some(
                "plex://9d3f1c7a5b2e4f6081a3c5e7f9b1d3e5a7c9e1f3\
                 /library/metadata/1000/thumb/1712000000"
            )
        );
        // An album artist's own track: the artist is the album's.
        assert_eq!(tracks[1].artists, ["Test Signal"]);
        // The token goes in a header, never the URL.
        assert!(transport.urls().iter().all(|url| !url.contains("Token")));
        let sent = transport.headers.lock().unwrap()[0].clone();
        assert!(sent
            .iter()
            .any(|(name, value)| name == "X-Plex-Token" && value == SERVER_TOKEN));

        // Changes alone.
        transport.push_status(&tracks_url("1", 0, Some(1_759_276_800)), 200, TRACKS_2);
        let (changed, _) = plex.tracks(MACHINE, "1", 0, Some(1_759_276_800)).unwrap();
        assert_eq!(changed.len(), 1);
        assert_eq!(changed[0].container.as_deref(), Some("dsf"));
    }

    #[test]
    fn reads_playlists_with_their_tracks() {
        let (http, transport, _) = fake_client();
        let device = device();
        serve_library(&transport);
        let playlists = server(&http, &device).playlists("1").unwrap();
        assert_eq!(playlists.len(), 1);
        assert_eq!(playlists[0].name, "Road Trip");
        assert_eq!(playlists[0].item_ids, ["1003", "1001"]);
    }

    #[test]
    fn fetch_picture_and_report_urls() {
        let (http, transport, _) = fake_client();
        let device = device();
        let plex = server(&http, &device);
        assert_eq!(
            plex.original_url("/library/parts/827/1712000000/file.flac")
                .as_deref(),
            Some(
                "https://192-168-1-20.a1b2c3.plex.direct:32400\
                 /library/parts/827/1712000000/file.flac"
            )
        );
        assert_eq!(plex.original_url("/elsewhere/x"), None);
        assert_eq!(plex.original_url("/library/parts/1/x?y"), None);
        assert_eq!(
            plex.mp3_url("1001", 192),
            format!(
                "{ADDRESS}/music/:/transcode/universal/start.mp3?path=%2Flibrary%2Fmetadata%2F1001\
                 &mediaIndex=0&partIndex=0&protocol=http&directPlay=0&directStream=0\
                 &musicBitrate=192&maxAudioBitrate=192&session=0123456789abcdef0123456789abcdef"
            )
        );
        let key = art_key(MACHINE, "/library/metadata/1000/thumb/1712000000");
        assert_eq!(
            plex.art_url(&key),
            Some(format!(
                "{ADDRESS}/photo/:/transcode?width=1200&height=1200&minSize=1&upscale=0\
                 &url=%2Flibrary%2Fmetadata%2F1000%2Fthumb%2F1712000000"
            ))
        );
        assert_eq!(plex.art_url("http://elsewhere/x.jpg"), None);
        assert_eq!(plex.art_url("plex://m/etc/passwd"), None);

        let timeline = url(
            "/:/timeline?ratingKey=1001&key=%2Flibrary%2Fmetadata%2F1001&state=stopped\
             &time=12500&duration=180000",
        );
        transport.push_status(&timeline, 200, "{}");
        plex.timeline("1001", "stopped", 12.5, 180.0).unwrap();
        assert_eq!(transport.urls(), [timeline]);
    }

    #[test]
    fn tries_the_last_address_then_local_then_remote_and_the_relay_last() {
        let resolver = FakeResolver::default();
        let all = connections();
        assert_eq!(
            candidates("", &all, false, &resolver),
            [ADDRESS, REMOTE],
            "the relay only when allowed"
        );
        assert_eq!(
            candidates(REMOTE, &all, true, &resolver),
            [REMOTE, ADDRESS, RELAY]
        );
        assert_eq!(
            candidates(RELAY, &all, false, &resolver),
            [ADDRESS, REMOTE],
            "a relay that answered last isn't tried once turned off"
        );
        // Plain http only to a local address.
        let plain = vec![
            Connection {
                uri: "http://192.168.1.20:32400".into(),
                local: true,
                relay: false,
            },
            Connection {
                uri: "http://203.0.113.9:32400".into(),
                local: false,
                relay: false,
            },
        ];
        assert_eq!(
            candidates("", &plain, false, &resolver),
            ["http://192.168.1.20:32400"]
        );
    }

    #[test]
    fn connects_where_the_server_answers_as_itself() {
        let (http, transport, _) = fake_client();
        let device = device();
        let resolver = FakeResolver::default();
        let all = connections();
        // The local address is down; another server answers... nowhere;
        // the remote one answers as this server.
        transport.push(
            &url("/identity"),
            Err(crate::metadata::http::TransportError::Unreachable(
                "down".into(),
            )),
        );
        transport.push_status(&format!("{REMOTE}/identity"), 200, IDENTITY);
        let (address, identity) =
            connect(&http, &device, MACHINE, "Den", "", &all, false, &resolver).unwrap();
        assert_eq!(address, REMOTE);
        assert_eq!(identity.machine_id, MACHINE);

        // Only the relay is left, and it isn't allowed.
        let (http, transport, _) = fake_client();
        transport.push_status(
            &url("/identity"),
            200,
            r#"{"MediaContainer":{"machineIdentifier":"other"}}"#,
        );
        let error = connect(
            &http,
            &device,
            MACHINE,
            "Den",
            "",
            &all[..1],
            false,
            &resolver,
        )
        .unwrap_err();
        assert!(matches!(error, Error::Offline(_)), "{error}");
        let error =
            connect(&http, &device, MACHINE, "Den", "", &all, false, &resolver).unwrap_err();
        assert!(
            crate::coded::is(&error.to_string(), "plexRelayOff"),
            "{error}"
        );
    }

    /// Needs a test server (7f.1): `PLEX_URL` (a connection's uri), and
    /// `PLEX_SERVER_TOKEN`. Lists its music libraries, reads a page of the
    /// first one's tracks and its playlists, and fetches the first track.
    #[test]
    #[ignore = "contacts a Plex server"]
    fn live_plex() {
        use crate::metadata::http::{SystemClock, UreqTransport};
        let address = std::env::var("PLEX_URL").expect("PLEX_URL is set");
        let token = std::env::var("PLEX_SERVER_TOKEN").expect("PLEX_SERVER_TOKEN is set");
        let http = Client::new(Box::new(UreqTransport::new()), Box::new(SystemClock));
        let device = device();
        let plex = Plex {
            http: &http,
            address: &address,
            device: &device,
            token: Some(&token),
        };
        let identity = plex.identity().unwrap();
        let sections = plex.music_sections().unwrap();
        let Some(section) = sections.first() else {
            return;
        };
        let (tracks, _) = plex
            .tracks(&identity.machine_id, &section.key, 0, None)
            .unwrap();
        plex.playlists(&section.key).unwrap();
        if let Some(path) = tracks.first().and_then(|track| track.media_path.clone()) {
            let headers = plex.headers();
            let headers: Vec<(&str, &str)> =
                headers.iter().map(|(n, v)| (*n, v.as_str())).collect();
            let mut sink = Vec::new();
            http.download_with_headers(
                &plex.original_url(&path).unwrap(),
                &headers,
                &mut sink,
                1 << 30,
                &|| false,
            )
            .unwrap();
            assert!(!sink.is_empty());
        }
    }
}
