//! The Discogs API (https://www.discogs.com/developers): release search and
//! lookup, parsed into the same `Release` as MusicBrainz's, with the user's
//! personal access token.
//!
//! Its terms (read 2026-09-26, last updated 2025-05-27; PLAN.md 4.8) shape
//! how it's used:
//! - Data may not be shown more than six hours behind discogs.com, nor kept
//!   longer than needed. Only the match (the Discogs release id) is stored;
//!   responses stay in memory for at most `MAX_AGE` (`Client::get_json_fresh`)
//!   and there is no offline copy.
//! - Images are "Restricted Data", not for commercial use: none are used,
//!   not even thumbnails.
//! - "Data provided by Discogs" (`CREDIT`) goes next to its data, linked to
//!   the release's page, and the settings show `NOTICE`.
//! - 60 requests a minute with a token (`http::request_interval`).

use std::time::Duration;

use rusqlite::Connection;
use serde::Deserialize;

use super::albums::{self, ReleaseSource};
use super::http::{percent_encode, Client};
use super::musicbrainz::{self, Credit, Label, Release, ReleaseTrack};
use super::settings::SourceId;
use super::Error;

pub const HOST: &str = "api.discogs.com";

const BASE: &str = "https://api.discogs.com";

/// Shown next to Discogs data, linked to the page it's from.
pub const CREDIT: &str = "Data provided by Discogs";

/// Shown with the source, in the terms' words.
pub const NOTICE: &str = "This application uses Discogs’ API but is not affiliated with, \
     sponsored or endorsed by Discogs. ‘Discogs’ is a trademark of Zink Media, LLC.";

/// How long a response is used before asking again: under the six hours
/// the terms allow, measured from when it was fetched.
pub const MAX_AGE: Duration = Duration::from_secs(5 * 3600);

/// Search results asked for.
const SEARCH_LIMIT: u32 = 10;

/// Credits kept per release; box sets can list hundreds.
const CREDITS_LIMIT: usize = 40;

pub fn release_url(id: &str) -> String {
    format!("{BASE}/releases/{id}")
}

/// The search for releases called `title` by `artist` (any artist if
/// `None`).
pub fn search_url(title: &str, artist: Option<&str>) -> String {
    let mut url = format!(
        "{BASE}/database/search?type=release&release_title={}",
        percent_encode(title)
    );
    if let Some(artist) = artist {
        url += &format!("&artist={}", percent_encode(artist));
    }
    url + &format!("&per_page={SEARCH_LIMIT}")
}

/// The release's page on discogs.com, which the credit links to.
pub fn page_url(id: &str) -> String {
    format!("https://www.discogs.com/release/{id}")
}

/// The `Authorization` header for `token`.
fn auth(token: &str) -> String {
    format!("Discogs token={token}")
}

/// The release id in a Discogs release link ("https://www.discogs.com/
/// release/1187003-Radiohead-In-Rainbows", with or without a language
/// such as "/fr/") or in Discogs' own "[r1187003]". Not a bare number,
/// which could be an album's title.
pub fn release_id_in(text: &str) -> Option<String> {
    let text = text.trim();
    let digits = if let Some(rest) = text.strip_prefix("[r") {
        rest.strip_suffix(']')?
    } else {
        let (_, rest) = text.split_once("discogs.com/")?;
        let (_, rest) = rest.split_once("release/")?;
        let end = rest
            .find(|c: char| !c.is_ascii_digit())
            .unwrap_or(rest.len());
        &rest[..end]
    };
    is_id(digits).then(|| digits.to_owned())
}

fn is_id(id: &str) -> bool {
    (1..=12).contains(&id.len()) && id.bytes().all(|b| b.is_ascii_digit())
}

/// The release with Discogs id `id`, from memory if fetched in the last
/// `MAX_AGE`.
pub fn lookup_release(client: &Client, token: &str, id: &str) -> Result<Release, Error> {
    if !is_id(id) {
        return Err(Error::Invalid(format!("Not a Discogs release id: {id}")));
    }
    parse_release(&get(client, token, &release_url(id))?)
}

/// Releases called `title` by `artist`, without their track lists.
pub fn search_releases(
    client: &Client,
    token: &str,
    title: &str,
    artist: Option<&str>,
) -> Result<Vec<Release>, Error> {
    parse_search(&get(client, token, &search_url(title, artist))?)
}

/// Discogs as an album-details source, with the user's token.
pub struct DiscogsReleases<'a> {
    pub client: &'a Client,
    pub conn: &'a Connection,
    pub token: String,
    /// Whether MusicBrainz may be asked for the release its match links to.
    pub musicbrainz: bool,
}

impl ReleaseSource for DiscogsReleases<'_> {
    fn id(&self) -> SourceId {
        SourceId::Discogs
    }

    fn search(
        &self,
        title: &str,
        artist: Option<&str>,
        _track_count: u32,
    ) -> Result<Vec<Release>, Error> {
        search_releases(self.client, &self.token, title, artist)
    }

    fn lookup(&self, id: &str) -> Result<Release, Error> {
        lookup_release(self.client, &self.token, id)
    }

    fn id_in(&self, text: &str) -> Option<String> {
        release_id_in(text)
    }

    /// The Discogs release the album's MusicBrainz match links to (an
    /// accepted match, not one awaiting review), as sure as that match.
    fn known_release(&self, album_id: i64) -> Result<Option<(String, f64)>, Error> {
        if !self.musicbrainz {
            return Ok(None);
        }
        let Some(link) = albums::album_link(self.conn, album_id, SourceId::MusicBrainz)? else {
            return Ok(None);
        };
        let Some(mbid) = link.matched_id() else {
            return Ok(None);
        };
        let ids = musicbrainz::discogs_release_ids(self.client, self.conn, mbid)?;
        Ok(ids.into_iter().next().map(|id| (id, link.score)))
    }
}

fn get(client: &Client, token: &str, url: &str) -> Result<String, Error> {
    match client.get_json_fresh(url, Some(&auth(token)), MAX_AGE) {
        Err(Error::Status {
            status: 401 | 403, ..
        }) => Err(Error::Invalid(
            "Discogs didn't accept the personal access token: check it in Online sources".into(),
        )),
        other => other,
    }
}

// The subset of Discogs' JSON that `Release` is made from. Everything is
// optional or defaulted: what's missing is left out, not an error. Images,
// community and marketplace fields are never read.

#[derive(Deserialize)]
struct RawRelease {
    id: u64,
    #[serde(default)]
    title: String,
    #[serde(default)]
    artists: Vec<RawArtist>,
    year: Option<u32>,
    released: Option<String>,
    country: Option<String>,
    #[serde(default)]
    labels: Vec<RawLabel>,
    #[serde(default)]
    formats: Vec<RawFormat>,
    master_id: Option<u64>,
    #[serde(default)]
    identifiers: Vec<RawIdentifier>,
    #[serde(default)]
    genres: Vec<String>,
    #[serde(default)]
    styles: Vec<String>,
    #[serde(default)]
    tracklist: Vec<RawTrack>,
    #[serde(default)]
    extraartists: Vec<RawArtist>,
}

#[derive(Deserialize)]
struct RawArtist {
    #[serde(default)]
    name: String,
    /// The name as printed on the release, if it differs.
    #[serde(default)]
    anv: String,
    #[serde(default)]
    join: String,
    #[serde(default)]
    role: String,
    /// The tracks a credit is limited to; empty for the whole release.
    #[serde(default)]
    tracks: String,
    id: Option<u64>,
}

#[derive(Deserialize)]
struct RawLabel {
    #[serde(default)]
    name: String,
    #[serde(default)]
    catno: String,
}

#[derive(Deserialize)]
struct RawFormat {
    #[serde(default)]
    name: String,
    qty: Option<String>,
    #[serde(default)]
    descriptions: Vec<String>,
    text: Option<String>,
}

#[derive(Deserialize)]
struct RawIdentifier {
    #[serde(rename = "type", default)]
    kind: String,
    #[serde(default)]
    value: String,
}

#[derive(Deserialize)]
struct RawTrack {
    #[serde(default)]
    position: String,
    /// "track", "index" (a track in parts) or "heading".
    #[serde(rename = "type_", default)]
    kind: String,
    #[serde(default)]
    title: String,
    #[serde(default)]
    duration: String,
}

pub fn parse_release(json: &str) -> Result<Release, Error> {
    let raw: RawRelease = serde_json::from_str(json)
        .map_err(|error| Error::Invalid(format!("Unexpected Discogs release: {error}")))?;
    let mut tracks = Vec::new();
    for track in raw
        .tracklist
        .iter()
        .filter(|track| track.kind == "track" || track.kind == "index")
    {
        let (disc, position) =
            disc_and_position(&track.position).unwrap_or((1, tracks.len() as u32 + 1));
        tracks.push(ReleaseTrack {
            disc,
            position,
            title: track.title.clone(),
            length_ms: duration_ms(&track.duration),
            recording_id: None,
        });
    }
    let descriptions: Vec<&str> = raw
        .formats
        .iter()
        .flat_map(|format| format.descriptions.iter().map(String::as_str))
        .collect();
    let (release_type, secondary_types) = release_type(&descriptions);
    let credits = raw
        .extraartists
        .iter()
        .filter(|credit| credit.tracks.trim().is_empty() && !credit.role.trim().is_empty())
        .take(CREDITS_LIMIT)
        .map(|credit| Credit {
            role: credit.role.trim().to_owned(),
            name: printed_name(credit),
        })
        .collect();
    Ok(Release {
        id: raw.id.to_string(),
        title: raw.title,
        artist: credit_text(&raw.artists),
        artist_ids: raw
            .artists
            .iter()
            .filter_map(|artist| artist.id.map(|id| id.to_string()))
            .collect(),
        date: release_date(raw.released.as_deref(), raw.year),
        country: non_empty(raw.country),
        status: None,
        barcode: raw
            .identifiers
            .iter()
            .find(|identifier| identifier.kind == "Barcode")
            .map(|identifier| identifier.value.replace(' ', ""))
            .filter(|barcode| !barcode.is_empty()),
        labels: raw
            .labels
            .iter()
            .map(|label| Label {
                name: non_empty(Some(clean_name(&label.name).to_owned())),
                catalog_number: catalog_number(&label.catno),
            })
            .filter(|label| label.name.is_some() || label.catalog_number.is_some())
            .collect(),
        release_group_id: raw.master_id.filter(|&id| id != 0).map(|id| id.to_string()),
        release_type,
        secondary_types,
        first_release_date: None,
        genres: raw.genres,
        track_count: tracks.len() as u32,
        has_front_art: None,
        tracks,
        formats: raw.formats.iter().flat_map(media).collect(),
        disambiguation: format_details(&raw.formats),
        styles: raw.styles,
        credits,
    })
}

pub fn parse_search(json: &str) -> Result<Vec<Release>, Error> {
    #[derive(Deserialize)]
    struct RawSearch {
        #[serde(default)]
        results: Vec<RawHit>,
    }
    #[derive(Deserialize)]
    struct RawHit {
        id: u64,
        #[serde(rename = "type", default)]
        kind: String,
        /// "Artist - Title".
        #[serde(default)]
        title: String,
        year: Option<String>,
        country: Option<String>,
        /// The format's name, then its descriptions: ["CD", "Album"].
        #[serde(default)]
        format: Vec<String>,
        format_quantity: Option<u32>,
        #[serde(default)]
        label: Vec<String>,
        catno: Option<String>,
        #[serde(default)]
        barcode: Vec<String>,
        master_id: Option<u64>,
        #[serde(default)]
        genre: Vec<String>,
        #[serde(default)]
        style: Vec<String>,
    }
    let raw: RawSearch = serde_json::from_str(json)
        .map_err(|error| Error::Invalid(format!("Unexpected Discogs search result: {error}")))?;
    Ok(raw
        .results
        .into_iter()
        .filter(|hit| hit.kind.is_empty() || hit.kind == "release")
        .map(|hit| {
            let (artist, title) = match hit.title.split_once(" - ") {
                Some((artist, title)) => {
                    (artist.trim_end_matches('*').to_owned(), title.to_owned())
                }
                None => (String::new(), hit.title.clone()),
            };
            let descriptions: Vec<&str> = hit.format.iter().skip(1).map(String::as_str).collect();
            let (release_type, secondary_types) = release_type(&descriptions);
            let medium = hit.format.first().cloned();
            let mut labels: Vec<Label> = Vec::new();
            for (index, name) in hit.label.iter().enumerate() {
                let name = non_empty(Some(clean_name(name).to_owned()));
                if !labels.iter().any(|label| label.name == name) {
                    labels.push(Label {
                        name,
                        // The search gives the first label's number only.
                        catalog_number: match index {
                            0 => hit.catno.as_deref().and_then(catalog_number),
                            _ => None,
                        },
                    });
                }
            }
            Release {
                id: hit.id.to_string(),
                title,
                artist,
                artist_ids: Vec::new(),
                date: hit.year.filter(|year| year.len() == 4 && year != "0000"),
                country: non_empty(hit.country),
                status: None,
                barcode: hit
                    .barcode
                    .first()
                    .map(|barcode| barcode.replace(' ', ""))
                    .filter(|barcode| !barcode.is_empty()),
                labels,
                release_group_id: hit.master_id.filter(|&id| id != 0).map(|id| id.to_string()),
                release_type,
                secondary_types,
                first_release_date: None,
                genres: hit.genre,
                // Unknown until the release is looked up.
                track_count: 0,
                has_front_art: None,
                tracks: Vec::new(),
                formats: (0..hit.format_quantity.unwrap_or(1).clamp(1, 20))
                    .map(|_| medium.clone())
                    .collect(),
                disambiguation: None,
                styles: hit.style,
                credits: Vec::new(),
            }
        })
        .collect())
}

/// Discogs names artists and labels that share a name "Name (2)"; the
/// number isn't part of the name.
fn clean_name(name: &str) -> &str {
    let name = name.trim();
    if let Some((base, number)) = name.rsplit_once(" (") {
        if number
            .strip_suffix(')')
            .is_some_and(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()))
        {
            return base;
        }
    }
    name
}

/// The name as printed on the release: its variation, else the name.
fn printed_name(artist: &RawArtist) -> String {
    match artist.anv.trim() {
        "" => clean_name(&artist.name).to_owned(),
        anv => anv.to_owned(),
    }
}

/// An artist credit as printed, e.g. "Radiohead = レディオヘッド".
fn credit_text(artists: &[RawArtist]) -> String {
    let mut text = String::new();
    for artist in artists {
        text += &printed_name(artist);
        match artist.join.trim() {
            "" => {}
            "," => text += ", ",
            join => text += &format!(" {join} "),
        }
    }
    text.trim_end().trim_end_matches(',').to_owned()
}

/// "2008-01-01", "2008-01" or "2008" from Discogs' "2008-01-00" and the
/// like, else the year alone.
fn release_date(released: Option<&str>, year: Option<u32>) -> Option<String> {
    let parts: Vec<&str> = released
        .unwrap_or("")
        .split('-')
        .take_while(|part| !part.is_empty() && part.bytes().all(|b| b.is_ascii_digit()))
        .take_while(|part| part.bytes().any(|b| b != b'0'))
        .collect();
    match parts.first() {
        Some(year) if year.len() == 4 => Some(parts.join("-")),
        _ => year.filter(|&year| year > 0).map(|year| year.to_string()),
    }
}

/// Discogs writes "none" for a release without a catalogue number.
fn catalog_number(catno: &str) -> Option<String> {
    let catno = catno.trim();
    (!catno.is_empty() && !catno.eq_ignore_ascii_case("none")).then(|| catno.to_owned())
}

/// A primary type like MusicBrainz's, and "Compilation", from a format's
/// descriptions.
fn release_type(descriptions: &[&str]) -> (Option<String>, Vec<String>) {
    let has = |name: &str| descriptions.contains(&name);
    let primary = if has("Album") || has("Mini-Album") {
        Some("Album")
    } else if has("EP") {
        Some("EP")
    } else if has("Single") || has("Maxi-Single") {
        Some("Single")
    } else {
        None
    };
    let secondary = has("Compilation")
        .then(|| "Compilation".to_owned())
        .into_iter()
        .collect();
    (primary.map(str::to_owned), secondary)
}

/// Words in a format's descriptions that say what the release is, rather
/// than how this issue differs from others.
const TYPE_WORDS: [&str; 8] = [
    "Album",
    "Mini-Album",
    "EP",
    "Single",
    "Maxi-Single",
    "Compilation",
    "LP",
    "Stereo",
];

/// Sizes that name a vinyl medium, as MusicBrainz writes them ("12\" Vinyl").
const VINYL_SIZES: [&str; 3] = ["12\"", "10\"", "7\""];

/// The format of each medium, e.g. two of "CD" for "2 × CD".
fn media(format: &RawFormat) -> Vec<Option<String>> {
    let quantity = format
        .qty
        .as_deref()
        .and_then(|qty| qty.trim().parse().ok())
        .unwrap_or(1u32)
        .clamp(1, 20);
    let name = match format.name.trim() {
        "" => None,
        "Vinyl" => Some(
            match VINYL_SIZES
                .iter()
                .find(|size| format.descriptions.iter().any(|d| d == *size))
            {
                Some(size) => format!("{size} Vinyl"),
                None => "Vinyl".to_owned(),
            },
        ),
        name => Some(name.to_owned()),
    };
    (0..quantity).map(|_| name.clone()).collect()
}

/// What tells this issue apart: the formats' other descriptions and text,
/// e.g. "Limited Edition, 180g, Gatefold".
fn format_details(formats: &[RawFormat]) -> Option<String> {
    let mut details: Vec<&str> = Vec::new();
    for format in formats {
        let words = format
            .descriptions
            .iter()
            .map(|d| d.trim())
            .filter(|d| !TYPE_WORDS.contains(d) && !VINYL_SIZES.contains(d))
            .chain(format.text.as_deref().map(str::trim));
        for word in words {
            if !word.is_empty() && !details.contains(&word) {
                details.push(word);
            }
        }
    }
    (!details.is_empty()).then(|| details.join(", "))
}

/// Disc and position from positions like "1-3", "CD2-1" or "2.4". Vinyl
/// sides ("A1") and plain numbers give `None`: the track's order is used.
fn disc_and_position(position: &str) -> Option<(u32, u32)> {
    let position = position.trim();
    let position = ["CD", "DVD", "Disc", "SACD"]
        .iter()
        .find_map(|prefix| position.strip_prefix(prefix))
        .unwrap_or(position);
    let (disc, track) = position.split_once(['-', '.'])?;
    Some((disc.trim().parse().ok()?, track.trim().parse().ok()?))
}

/// Milliseconds from "4:37" or "1:02:03".
fn duration_ms(duration: &str) -> Option<u32> {
    let mut seconds: u32 = 0;
    let parts: Vec<&str> = duration.trim().split(':').collect();
    if parts.len() < 2 || parts.len() > 3 {
        return None;
    }
    for part in parts {
        seconds = seconds.checked_mul(60)?.checked_add(part.parse().ok()?)?;
    }
    (seconds > 0).then(|| seconds * 1000)
}

fn non_empty(text: Option<String>) -> Option<String> {
    text.map(|text| text.trim().to_owned())
        .filter(|text| !text.is_empty())
}

#[cfg(test)]
pub mod fixtures {
    //! Recorded Discogs releases of "In Rainbows" by Radiohead, trimmed to
    //! their CC0 fields (titles, artists, labels, formats, identifiers,
    //! track lists, credits; no images, community or marketplace data), and
    //! a search response for them. Searching needs a token, so the search
    //! wasn't recorded: it was put together in the documented format from
    //! the three releases.

    pub const SEARCH: &str = include_str!("fixtures/discogs/search-in-rainbows.json");

    /// The Japanese CD, the US LP (which MusicBrainz links to) and the
    /// Canadian CD, by id.
    pub const RELEASES: [(&str, &str); 3] = [
        (
            "1376757",
            include_str!("fixtures/discogs/release-1376757.json"),
        ),
        (
            "1187003",
            include_str!("fixtures/discogs/release-1187003.json"),
        ),
        (
            "1186009",
            include_str!("fixtures/discogs/release-1186009.json"),
        ),
    ];

    pub const MASTER: &str = "21520";
}

#[cfg(test)]
mod tests {
    use super::fixtures::*;
    use super::*;
    use crate::metadata::http::testing::fake_client;

    #[test]
    fn parses_a_release() {
        let release = parse_release(RELEASES[1].1).unwrap();
        assert_eq!(release.id, "1187003");
        assert_eq!(
            (release.title.as_str(), release.artist.as_str()),
            ("In Rainbows", "Radiohead")
        );
        assert_eq!(release.date.as_deref(), Some("2008-01-01"));
        assert_eq!(release.country.as_deref(), Some("US"));
        assert_eq!(release.release_group_id.as_deref(), Some(MASTER));
        assert_eq!(release.release_type.as_deref(), Some("Album"));
        assert_eq!(release.formats, [Some("Vinyl".to_owned())]);
        assert_eq!(release.barcode.as_deref(), Some("880882162313"));
        assert_eq!(
            release.labels[0],
            Label {
                name: Some("TBD Records".into()),
                catalog_number: Some("TBD0001".into())
            }
        );
        assert_eq!(release.genres, ["Electronic", "Rock"]);
        assert!(release.styles.contains(&"Art Rock".to_owned()));
        assert!(release.credits.contains(&Credit {
            role: "Mixed By".into(),
            name: "Nigel Godrich".into()
        }));
        // Vinyl sides: the tracks are numbered in order.
        assert_eq!(release.track_count, 10);
        let positions: Vec<(u32, u32)> = release
            .tracks
            .iter()
            .map(|t| (t.disc, t.position))
            .collect();
        assert_eq!(positions[5], (1, 6));
        assert_eq!(release.tracks[0].title, "15 Step");
        assert_eq!(release.tracks[0].length_ms, Some(237_000));
        assert_eq!(release.has_front_art, None);

        // A credit printed in another script, and the case the CD came in.
        let japanese = parse_release(RELEASES[0].1).unwrap();
        assert_eq!(japanese.artist, "Radiohead = レディオヘッド");
        assert_eq!(japanese.disambiguation.as_deref(), Some("Cardboard Case"));
    }

    #[test]
    fn parses_a_search() {
        let hits = parse_search(SEARCH).unwrap();
        assert_eq!(hits.len(), 3);
        let hit = &hits[1];
        assert_eq!(hit.id, "1187003");
        assert_eq!(
            (hit.artist.as_str(), hit.title.as_str()),
            ("Radiohead", "In Rainbows")
        );
        assert_eq!(hit.date.as_deref(), Some("2008"));
        assert_eq!(hit.formats, [Some("Vinyl".to_owned())]);
        assert_eq!(hit.release_type.as_deref(), Some("Album"));
        assert_eq!(hit.labels[0].catalog_number.as_deref(), Some("TBD0001"));
        assert_eq!(hit.track_count, 0, "unknown until looked up");
        assert!(hit.tracks.is_empty());
        assert_eq!(hits[0].artist, "Radiohead = レディオヘッド");
    }

    #[test]
    fn reads_small_fields() {
        assert_eq!(clean_name("Plank (3)"), "Plank");
        assert_eq!(clean_name("Sunn O)))"), "Sunn O)))");
        assert_eq!(clean_name("Area (Remix)"), "Area (Remix)");
        assert_eq!(
            release_date(Some("2008-01-00"), Some(2008)).as_deref(),
            Some("2008-01")
        );
        assert_eq!(
            release_date(Some("2008-00-00"), None).as_deref(),
            Some("2008")
        );
        assert_eq!(release_date(None, Some(1999)).as_deref(), Some("1999"));
        assert_eq!(release_date(Some(""), Some(0)), None);
        assert_eq!(disc_and_position("2-03"), Some((2, 3)));
        assert_eq!(disc_and_position("CD1-12"), Some((1, 12)));
        assert_eq!(disc_and_position("A1"), None);
        assert_eq!(disc_and_position("7"), None);
        assert_eq!(duration_ms("4:37"), Some(277_000));
        assert_eq!(duration_ms("1:02:03"), Some(3_723_000));
        for bad in ["", "4", "x:10", "0:00"] {
            assert_eq!(duration_ms(bad), None, "{bad}");
        }
        assert_eq!(catalog_number("none"), None);
        let format = RawFormat {
            name: "Vinyl".into(),
            qty: Some("2".into()),
            descriptions: vec!["12\"".into(), "LP".into(), "Album".into(), "180g".into()],
            text: Some("Gatefold".into()),
        };
        assert_eq!(
            media(&format),
            [Some("12\" Vinyl".to_owned()), Some("12\" Vinyl".to_owned())]
        );
        assert_eq!(format_details(&[format]).as_deref(), Some("180g, Gatefold"));
    }

    #[test]
    fn finds_release_ids_in_links() {
        for (text, id) in [
            (
                "https://www.discogs.com/release/1187003-Radiohead-In-Rainbows",
                Some("1187003"),
            ),
            ("https://www.discogs.com/release/1187003", Some("1187003")),
            (" https://www.discogs.com/fr/release/42 ", Some("42")),
            ("[r1187003]", Some("1187003")),
            ("1187003", None),
            ("https://www.discogs.com/master/21520", None),
            ("https://musicbrainz.org/release/1187003", None),
        ] {
            assert_eq!(release_id_in(text).as_deref(), id, "{text}");
        }
    }

    #[test]
    fn sends_the_token_and_names_a_refused_one() {
        let (client, transport, _clock) = fake_client();
        let (id, json) = RELEASES[1];
        transport.push_status(&release_url(id), 200, json);
        let release = lookup_release(&client, "secret", id).unwrap();
        assert_eq!(release.id, id);
        assert_eq!(
            transport.auths.lock().unwrap().as_slice(),
            [Some("Discogs token=secret".to_owned())]
        );
        assert!(!transport.urls()[0].contains("secret"));

        transport.push_status(&search_url("x", None), 401, "{}");
        let error = search_releases(&client, "wrong", "x", None).unwrap_err();
        assert!(error.to_string().contains("token"), "{error}");
        assert!(lookup_release(&client, "secret", "12a").is_err());
    }

    #[test]
    fn builds_urls() {
        assert_eq!(
            search_url("In Rainbows", Some("Radiohead")),
            "https://api.discogs.com/database/search?type=release\
             &release_title=In%20Rainbows&artist=Radiohead&per_page=10"
        );
        assert_eq!(
            page_url("1187003"),
            "https://www.discogs.com/release/1187003"
        );
    }

    mod matching {
        use rusqlite::params;

        use super::super::fixtures::*;
        use super::super::*;
        use crate::library::test_library::{track, Library};
        use crate::metadata::albums::{self, LinkStatus};
        use crate::metadata::http::testing::{fake_client, FakeTransport};
        use crate::metadata::matcher;
        use crate::metadata::musicbrainz::fixtures::{RELEASE_LINKS, RELEASE_LINKS_DISCOGS};
        use crate::metadata::musicbrainz::release_links_url;

        /// The durations of "In Rainbows", plus a little encoder padding.
        const DURATIONS: [f64; 10] = [
            237.4, 242.9, 255.6, 318.5, 228.3, 129.8, 290.5, 328.9, 248.6, 279.2,
        ];

        /// "In Rainbows" by Radiohead: album 1, ten tracks.
        fn library() -> Library {
            let library = Library::new((1..=10).map(|n| {
                track(&format!("Radiohead/In Rainbows/{n:02}.flac"))
                    .artist("Radiohead")
                    .album("In Rainbows")
                    .year(2007)
                    .number(n)
            }));
            for (n, duration) in DURATIONS.iter().enumerate() {
                library
                    .conn
                    .execute(
                        "UPDATE tracks SET duration = ?1, track_total = 10 WHERE track_number = ?2",
                        params![duration, n as u32 + 1],
                    )
                    .unwrap();
            }
            library
        }

        fn discogs<'a>(
            client: &'a Client,
            conn: &'a Connection,
            musicbrainz: bool,
        ) -> DiscogsReleases<'a> {
            DiscogsReleases {
                client,
                conn,
                token: "token".into(),
                musicbrainz,
            }
        }

        fn serve_search(transport: &FakeTransport) {
            transport.push_status(&search_url("In Rainbows", Some("Radiohead")), 200, SEARCH);
            for (id, json) in RELEASES {
                transport.push_status(&release_url(id), 200, json);
            }
        }

        fn set_musicbrainz_match(conn: &Connection, mbid: &str) {
            conn.execute(
                "INSERT OR REPLACE INTO album_links
                     (album_id, source, status, external_id, score, chosen_by, details, checked_at)
                 VALUES (1, 'musicbrainz', 'matched', ?1, 0.9, 'auto', NULL, 0)",
                [mbid],
            )
            .unwrap();
        }

        fn link(conn: &Connection) -> Option<albums::AlbumLink> {
            albums::album_link(conn, 1, SourceId::Discogs).unwrap()
        }

        #[test]
        fn takes_the_release_musicbrainz_links_to() {
            let library = library();
            let (client, transport, _clock) = fake_client();
            let (mbid, json) = RELEASE_LINKS;
            set_musicbrainz_match(&library.conn, mbid);
            transport.push_status(&release_links_url(mbid), 200, json);
            let source = discogs(&client, &library.conn, true);
            assert_eq!(
                albums::match_album_at(&source, &library.conn, 1).unwrap(),
                LinkStatus::Matched
            );
            let link = link(&library.conn).unwrap();
            assert_eq!(link.external_id.as_deref(), Some(RELEASE_LINKS_DISCOGS));
            assert_eq!(link.score, 0.9, "as sure as the MusicBrainz match");
            assert_eq!(link.details, None, "only the id is kept");
            assert_eq!(
                transport.urls(),
                [release_links_url(mbid)],
                "Discogs wasn't asked"
            );
        }

        #[test]
        fn matches_by_search_and_keeps_only_the_id() {
            let library = library();
            let (client, transport, _clock) = fake_client();
            serve_search(&transport);
            let source = discogs(&client, &library.conn, false);
            assert_eq!(
                albums::match_album_at(&source, &library.conn, 1).unwrap(),
                LinkStatus::Matched
            );
            // A search and three lookups, with the token.
            assert_eq!(transport.urls().len(), 4);
            assert!(transport
                .auths
                .lock()
                .unwrap()
                .iter()
                .all(|auth| auth.as_deref() == Some("Discogs token=token")));
            let link = link(&library.conn).unwrap();
            assert!(link.score >= matcher::ACCEPT_SCORE, "{}", link.score);
            assert_eq!(link.details, None);
            assert_eq!(link.release, None);
            let id = link.external_id.clone().unwrap();
            assert!(RELEASES.iter().any(|(release, _)| *release == id));

            // Shown: looked up, from memory while it's fresh.
            let release = albums::linked_release(&source, &link).unwrap().unwrap();
            assert_eq!(release.id, id);
            assert_eq!(transport.urls().len(), 4);

            // The dialog: every release, looked up, with its page.
            let candidates =
                albums::release_candidates_at(&source, &library.conn, 1, None).unwrap();
            assert_eq!(candidates.len(), 3);
            assert!(candidates.iter().all(|c| c.full));
            assert_eq!(
                candidates[0].page_url,
                Some(page_url(&candidates[0].release.id))
            );

            // A MusicBrainz link finds nothing here; a Discogs one is looked up.
            let mb = "https://musicbrainz.org/release/219e7d7c-806c-44b3-9972-cdb3614b3411";
            let found = albums::release_candidates_at(&source, &library.conn, 1, Some((mb, None)));
            // The current match is still offered.
            assert!(found.unwrap().iter().all(|c| c.release.id == id));
            assert_eq!(transport.urls().len(), 4);
            let typed = "https://www.discogs.com/release/1186009-Radiohead-In-Rainbows";
            let found =
                albums::release_candidates_at(&source, &library.conn, 1, Some((typed, None)))
                    .unwrap();
            assert!(found.iter().any(|c| c.release.id == "1186009"));
        }

        #[test]
        fn a_new_musicbrainz_match_drops_an_automatic_one_but_not_the_users() {
            let library = library();
            let conn = &library.conn;
            let (client, transport, _clock) = fake_client();
            let (mbid, json) = RELEASE_LINKS;
            set_musicbrainz_match(conn, mbid);
            transport.push_status(&release_links_url(mbid), 200, json);
            let source = discogs(&client, conn, true);
            albums::match_album_at(&source, conn, 1).unwrap();
            assert!(link(conn).is_some());

            let other = "00000000-0000-0000-0000-000000000001";
            let set_mb = |id: &str| {
                albums::store_source_link(
                    conn,
                    1,
                    SourceId::MusicBrainz,
                    LinkStatus::Matched,
                    Some(id),
                    1.0,
                    None::<&Release>,
                )
                .unwrap()
            };
            set_mb(other);
            assert_eq!(link(conn), None, "it came from the old match");

            // The user's pick stays whatever MusicBrainz says.
            let (id, json) = RELEASES[2];
            transport.push_status(&release_url(id), 200, json);
            let chosen = albums::choose_release_at(&source, conn, 1, id).unwrap();
            assert!(chosen.chosen_by_user);
            assert_eq!(chosen.details, None);
            set_mb(mbid);
            assert_eq!(link(conn).unwrap().external_id.as_deref(), Some(id));
            let requests = transport.urls().len();
            assert_eq!(
                albums::match_album_at(&source, conn, 1).unwrap(),
                LinkStatus::Matched
            );
            assert_eq!(transport.urls().len(), requests, "nothing was looked up");

            albums::reject_releases(conn, 1, SourceId::Discogs).unwrap();
            let rejected = link(conn).unwrap();
            assert_eq!(
                (rejected.status, rejected.chosen_by_user),
                (LinkStatus::None, true)
            );
        }
    }

    /// Needs `DISCOGS_TOKEN` (a personal access token) in the environment.
    #[test]
    #[ignore = "contacts Discogs"]
    fn live_discogs() {
        use crate::metadata::http::{SystemClock, UreqTransport};
        let token = std::env::var("DISCOGS_TOKEN").expect("DISCOGS_TOKEN is set");
        let client = Client::new(Box::new(UreqTransport::new()), Box::new(SystemClock));
        let hits = search_releases(&client, &token, "In Rainbows", Some("Radiohead")).unwrap();
        assert!(hits.iter().any(|hit| hit.id == "1187003"), "{hits:?}");
        let release = lookup_release(&client, &token, "1187003").unwrap();
        assert_eq!(release.title, "In Rainbows");
        assert_eq!(release.track_count, 10);
        assert_eq!(release.release_group_id.as_deref(), Some(MASTER));
    }
}
