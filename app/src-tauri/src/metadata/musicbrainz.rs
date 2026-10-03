//! The MusicBrainz web service (https://musicbrainz.org/doc/MusicBrainz_API):
//! release search and lookup, parsed into `Release`, artist search and
//! lookup, parsed into `Artist`, release group lookup, parsed into
//! `ReleaseGroup` for its links, and an artist's release groups, parsed
//! into `ReleaseGroupEntry` for their discography, and a release's links to
//! Discogs. Requests go through
//! `http::Client`, which keeps to MusicBrainz's one request a second, and
//! responses are cached (`cache`).

use std::time::Duration;

use rusqlite::Connection;
use serde::{Deserialize, Serialize};

use super::http::{percent_encode, Client};
use super::Error;

pub const HOST: &str = "musicbrainz.org";

const BASE: &str = "https://musicbrainz.org/ws/2";

/// What a release lookup includes.
const RELEASE_INC: &str = "recordings+artist-credits+labels+release-groups+genres";

/// What an artist lookup includes: links to other sites (Wikidata, the
/// homepage), and genres.
const ARTIST_INC: &str = "url-rels+genres+artist-rels";

/// What a release group lookup includes: links to other sites (Wikidata).
const RELEASE_GROUP_INC: &str = "url-rels";

/// What a release's links are looked up with. Separate from the release
/// lookup (`RELEASE_INC`), so releases matched before are not fetched again.
const RELEASE_LINKS_INC: &str = "url-rels";

/// What an artist's release groups are browsed with: their artist credits,
/// which show collaborations.
const RELEASE_GROUPS_INC: &str = "artist-credits";

/// Release groups per page of a browse, the most MusicBrainz sends.
pub const BROWSE_LIMIT: u32 = 100;

/// The artist MusicBrainz credits "Various Artists" releases to.
pub const VARIOUS_ARTISTS: &str = "89ad4ac3-39f7-470e-963a-56509c546377";

/// How long cached responses are used before asking again. Releases change
/// rarely; searches pick up new releases sooner.
const LOOKUP_MAX_AGE: Duration = Duration::from_secs(30 * 86400);
const SEARCH_MAX_AGE: Duration = Duration::from_secs(7 * 86400);

/// Search results asked for.
const SEARCH_LIMIT: u32 = 10;

/// A release (one issue of an album) as the app keeps it: the fields shown
/// or used for matching. Stored as JSON in `album_links.details`. Other
/// album-details sources (Discogs) convert their releases to it too, so the
/// matcher and the UI handle every source alike; `release_group_id` is then
/// their grouping of a release's issues (a Discogs master).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct Release {
    pub id: String,
    pub title: String,
    /// The artist credit as printed, e.g. "Simon & Garfunkel".
    pub artist: String,
    pub artist_ids: Vec<String>,
    /// "2007", "2007-12" or "2007-12-26".
    pub date: Option<String>,
    pub country: Option<String>,
    /// "Official", "Promotion", "Bootleg"…
    pub status: Option<String>,
    pub barcode: Option<String>,
    pub labels: Vec<Label>,
    pub release_group_id: Option<String>,
    /// The release group's primary type: "Album", "Single", "EP"…
    pub release_type: Option<String>,
    /// "Compilation", "Live", "Soundtrack"…
    pub secondary_types: Vec<String>,
    /// The release group's first release date: the original year of a
    /// reissue.
    pub first_release_date: Option<String>,
    /// The release's genres, else its release group's, most voted first.
    pub genres: Vec<String>,
    pub track_count: u32,
    /// Whether the Cover Art Archive has a front cover; unknown in search
    /// results.
    pub has_front_art: Option<bool>,
    /// Empty in search results.
    pub tracks: Vec<ReleaseTrack>,
    /// Each medium's format, e.g. ["CD", "CD"] or ["12\" Vinyl"]; `None`
    /// for a medium without one. Missing in details stored before 4.6.
    #[serde(default)]
    pub formats: Vec<Option<String>>,
    /// Tells releases of the same album apart, e.g. "deluxe edition".
    #[serde(default)]
    pub disambiguation: Option<String>,
    /// Finer genres ("Art Rock"); Discogs only.
    #[serde(default)]
    pub styles: Vec<String>,
    /// Who did what on the whole release; Discogs only.
    #[serde(default)]
    pub credits: Vec<Credit>,
}

/// A credit on a release, e.g. "Producer" by "Nigel Godrich".
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct Credit {
    pub role: String,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct Label {
    pub name: Option<String>,
    pub catalog_number: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct ReleaseTrack {
    /// 1-based medium (disc) position.
    pub disc: u32,
    pub position: u32,
    pub title: String,
    pub length_ms: Option<u32>,
    pub recording_id: Option<String>,
}

impl Release {
    /// The year the release came out, from its date.
    pub fn year(&self) -> Option<u32> {
        year_of(self.date.as_deref())
    }

    /// The year the album first came out, from the release group.
    pub fn original_year(&self) -> Option<u32> {
        year_of(self.first_release_date.as_deref())
    }
}

fn year_of(date: Option<&str>) -> Option<u32> {
    date?.get(..4)?.parse().ok()
}

/// An artist as the app keeps it: the facts shown on the artist page, and
/// the links that lead to a biography. Stored as JSON in
/// `artist_links.details`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[cfg_attr(test, ts(rename = "MusicBrainzArtist"))]
#[serde(rename_all = "camelCase")]
pub struct Artist {
    pub id: String,
    pub name: String,
    pub sort_name: Option<String>,
    /// Tells artists of the same name apart, e.g. "60s band from the UK".
    pub disambiguation: Option<String>,
    /// "Person", "Group", "Orchestra", "Choir", "Character" or "Other".
    #[serde(rename = "type")]
    pub artist_type: Option<String>,
    /// The country (or other area) the artist is from.
    pub area: Option<String>,
    /// Where a person was born or a group was formed, and where a person
    /// died or a group broke up.
    pub begin_area: Option<String>,
    pub end_area: Option<String>,
    /// Born or formed, and died or broke up: "1985", "1985-06" or
    /// "1985-06-01".
    pub begin: Option<String>,
    pub end: Option<String>,
    pub ended: bool,
    /// Other names the artist is known by; only in search results.
    #[serde(default)]
    pub aliases: Vec<String>,
    /// Most voted first.
    pub genres: Vec<String>,
    /// The artist's Wikidata item, e.g. "Q44190"; from a lookup only.
    pub wikidata: Option<String>,
    /// An English Wikipedia article linked directly (older entries have
    /// these instead of Wikidata).
    pub wikipedia: Option<String>,
    /// The official homepage.
    pub homepage: Option<String>,
    /// Bands they were in (or members of the band), collaborations and
    /// subgroups, for library radio (PLAN.md O9). Missing in details
    /// stored before it.
    #[serde(default)]
    pub related: Vec<RelatedArtist>,
}

/// An artist linked to another on MusicBrainz.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct RelatedArtist {
    pub id: String,
    pub name: String,
    /// MusicBrainz's relationship type, e.g. "member of band".
    pub relation: String,
}

/// The artist relationships kept for radio.
const RELATED_KINDS: [&str; 3] = ["member of band", "collaboration", "subgroup"];

/// A release group (an album, across all its releases) as the app keeps it:
/// the links that lead to a description of the album. The rest of it comes
/// with each `Release`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReleaseGroup {
    pub id: String,
    pub title: String,
    /// The album's Wikidata item, e.g. "Q223295".
    pub wikidata: Option<String>,
    /// An English Wikipedia article linked directly (older entries).
    pub wikipedia: Option<String>,
}

/// A release group as an artist's discography lists it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct ReleaseGroupEntry {
    pub id: String,
    pub title: String,
    /// The artist credit as printed, e.g. "DJ Shadow vs. Radiohead".
    pub artist: String,
    /// The primary type: "Album", "Single", "EP", "Broadcast" or "Other".
    pub release_type: Option<String>,
    /// "Compilation", "Live", "Remix"…
    pub secondary_types: Vec<String>,
    /// "2007", "2007-10" or "2007-10-10"; `None` when undated.
    pub first_release_date: Option<String>,
    /// Tells release groups of the same name apart.
    pub disambiguation: Option<String>,
}

/// One page of an artist's release groups.
#[derive(Debug, Clone, PartialEq)]
pub struct ReleaseGroupPage {
    pub groups: Vec<ReleaseGroupEntry>,
    /// How many release groups there are in all pages.
    pub count: u32,
}

/// An artist search hit, with MusicBrainz's score (0 to 100).
#[derive(Debug, Clone, PartialEq)]
pub struct ArtistHit {
    pub artist: Artist,
    pub score: u32,
}

/// A search hit: a release without its tracks, and MusicBrainz's own score
/// (0 to 100) for how well it matched the query.
#[derive(Debug, Clone, PartialEq)]
pub struct SearchHit {
    pub release: Release,
    pub score: u32,
}

/// The MBID in `text`: `text` itself, or the id after `/<entity>/` in a
/// MusicBrainz URL such as "https://musicbrainz.org/release/<id>".
pub fn mbid_in<'a>(text: &'a str, entity: &str) -> Option<&'a str> {
    let text = text.trim();
    if is_mbid(text) {
        return Some(text);
    }
    let (_, rest) = text.split_once(&format!("musicbrainz.org/{entity}/"))?;
    rest.get(..36).filter(|id| is_mbid(id))
}

/// Whether `id` looks like an MBID, so a tag value can go into a URL.
pub fn is_mbid(id: &str) -> bool {
    id.len() == 36
        && id.char_indices().all(|(index, c)| match index {
            8 | 13 | 18 | 23 => c == '-',
            _ => c.is_ascii_hexdigit(),
        })
}

pub fn release_url(id: &str) -> String {
    format!("{BASE}/release/{id}?inc={RELEASE_INC}&fmt=json")
}

/// The search for releases called `title` by `artist` (any artist if
/// `None`), with `track_count` tracks as a hint that ranks such releases
/// higher. Fields are matched as terms, not phrases, so small differences
/// in punctuation still find the release.
pub fn search_url(title: &str, artist: Option<&str>, track_count: Option<u32>) -> String {
    let mut query = format!("release:({})", escape_lucene(title));
    if let Some(artist) = artist {
        query += &format!(" AND artist:({})", escape_lucene(artist));
    }
    if let Some(count) = track_count {
        query += &format!(" tracks:{count}");
    }
    format!(
        "{BASE}/release/?query={}&limit={SEARCH_LIMIT}&fmt=json",
        percent_encode(&query)
    )
}

pub fn artist_url(id: &str) -> String {
    format!("{BASE}/artist/{id}?inc={ARTIST_INC}&fmt=json")
}

/// A release with its links to other sites.
pub fn release_links_url(id: &str) -> String {
    format!("{BASE}/release/{id}?inc={RELEASE_LINKS_INC}&fmt=json")
}

pub fn release_group_url(id: &str) -> String {
    format!("{BASE}/release-group/{id}?inc={RELEASE_GROUP_INC}&fmt=json")
}

/// The page of artist `artist`'s release groups (by MBID) starting
/// `offset` groups in. Only those MusicBrainz's own artist page lists
/// ("website-default"): release groups with only bootleg, promotional or
/// other unofficial releases are left out.
pub fn release_groups_url(artist: &str, offset: u32) -> String {
    format!(
        "{BASE}/release-group?artist={artist}&release-group-status=website-default\
         &inc={RELEASE_GROUPS_INC}&limit={BROWSE_LIMIT}&offset={offset}&fmt=json"
    )
}

/// The search for artists called `name` (or with it as an alias).
pub fn artist_search_url(name: &str) -> String {
    let name = escape_lucene(name);
    format!(
        "{BASE}/artist/?query={}&limit={SEARCH_LIMIT}&fmt=json",
        percent_encode(&format!("artist:({name}) OR alias:({name})"))
    )
}

/// `text` with Lucene's special characters escaped, so it's searched for as
/// words.
fn escape_lucene(text: &str) -> String {
    let mut escaped = String::with_capacity(text.len());
    for c in text.chars() {
        if "+-&|!(){}[]^\"~*?:\\/".contains(c) {
            escaped.push('\\');
        }
        escaped.push(c);
    }
    escaped
}

pub fn lookup_release(client: &Client, conn: &Connection, id: &str) -> Result<Release, Error> {
    if !is_mbid(id) {
        return Err(Error::Invalid(format!("Not a MusicBrainz id: {id}")));
    }
    let body = client.get_json(conn, &release_url(id), LOOKUP_MAX_AGE)?;
    parse_release(&body)
}

pub fn search_releases(
    client: &Client,
    conn: &Connection,
    title: &str,
    artist: Option<&str>,
    track_count: Option<u32>,
) -> Result<Vec<SearchHit>, Error> {
    let body = client.get_json(
        conn,
        &search_url(title, artist, track_count),
        SEARCH_MAX_AGE,
    )?;
    parse_search(&body)
}

pub fn lookup_artist(client: &Client, conn: &Connection, id: &str) -> Result<Artist, Error> {
    if !is_mbid(id) {
        return Err(Error::Invalid(format!("Not a MusicBrainz id: {id}")));
    }
    let body = client.get_json(conn, &artist_url(id), LOOKUP_MAX_AGE)?;
    parse_artist(&body)
}

pub fn search_artists(
    client: &Client,
    conn: &Connection,
    name: &str,
) -> Result<Vec<ArtistHit>, Error> {
    let body = client.get_json(conn, &artist_search_url(name), SEARCH_MAX_AGE)?;
    parse_artist_search(&body)
}

/// The Discogs releases that MusicBrainz release `id` links to, as their
/// Discogs ids (usually one).
pub fn discogs_release_ids(
    client: &Client,
    conn: &Connection,
    id: &str,
) -> Result<Vec<String>, Error> {
    if !is_mbid(id) {
        return Err(Error::Invalid(format!("Not a MusicBrainz id: {id}")));
    }
    let body = client.get_json(conn, &release_links_url(id), LOOKUP_MAX_AGE)?;
    parse_discogs_links(&body)
}

pub fn parse_discogs_links(json: &str) -> Result<Vec<String>, Error> {
    #[derive(Deserialize)]
    struct RawLinks {
        #[serde(default)]
        relations: Vec<RawRelation>,
    }
    let raw: RawLinks = serde_json::from_str(json)
        .map_err(|error| Error::Invalid(format!("Unexpected MusicBrainz release: {error}")))?;
    let mut ids: Vec<String> = Vec::new();
    for id in Links(&raw.relations)
        .of("discogs")
        .filter_map(super::discogs::release_id_in)
    {
        if !ids.contains(&id) {
            ids.push(id);
        }
    }
    Ok(ids)
}

pub fn lookup_release_group(
    client: &Client,
    conn: &Connection,
    id: &str,
) -> Result<ReleaseGroup, Error> {
    if !is_mbid(id) {
        return Err(Error::Invalid(format!("Not a MusicBrainz id: {id}")));
    }
    let body = client.get_json(conn, &release_group_url(id), LOOKUP_MAX_AGE)?;
    parse_release_group(&body)
}

pub fn parse_release_group(json: &str) -> Result<ReleaseGroup, Error> {
    #[derive(Deserialize)]
    struct RawGroup {
        id: String,
        #[serde(default)]
        title: String,
        #[serde(default)]
        relations: Vec<RawRelation>,
    }
    let raw: RawGroup = serde_json::from_str(json).map_err(|error| {
        Error::Invalid(format!("Unexpected MusicBrainz release group: {error}"))
    })?;
    let links = Links(&raw.relations);
    Ok(ReleaseGroup {
        wikidata: links.wikidata(),
        wikipedia: links.wikipedia(),
        id: raw.id,
        title: raw.title,
    })
}

pub fn parse_release_groups(json: &str) -> Result<ReleaseGroupPage, Error> {
    #[derive(Deserialize)]
    #[serde(rename_all = "kebab-case")]
    struct RawPage {
        release_group_count: Option<u32>,
        #[serde(default)]
        release_groups: Vec<RawBrowsedGroup>,
    }
    #[derive(Deserialize)]
    #[serde(rename_all = "kebab-case")]
    struct RawBrowsedGroup {
        id: String,
        #[serde(default)]
        title: String,
        #[serde(default)]
        artist_credit: Vec<RawCredit>,
        primary_type: Option<String>,
        #[serde(default)]
        secondary_types: Vec<String>,
        first_release_date: Option<String>,
        disambiguation: Option<String>,
    }
    let raw: RawPage = serde_json::from_str(json).map_err(|error| {
        Error::Invalid(format!("Unexpected MusicBrainz release groups: {error}"))
    })?;
    let groups: Vec<ReleaseGroupEntry> = raw
        .release_groups
        .into_iter()
        .map(|group| ReleaseGroupEntry {
            id: group.id,
            title: group.title,
            artist: credit_text(&group.artist_credit),
            release_type: non_empty(group.primary_type),
            secondary_types: group.secondary_types,
            first_release_date: non_empty(group.first_release_date),
            disambiguation: non_empty(group.disambiguation),
        })
        .collect();
    Ok(ReleaseGroupPage {
        count: raw.release_group_count.unwrap_or(groups.len() as u32),
        groups,
    })
}

pub fn parse_artist(json: &str) -> Result<Artist, Error> {
    let raw: RawArtistEntry = serde_json::from_str(json)
        .map_err(|error| Error::Invalid(format!("Unexpected MusicBrainz artist: {error}")))?;
    Ok(raw.into_artist())
}

pub fn parse_artist_search(json: &str) -> Result<Vec<ArtistHit>, Error> {
    #[derive(Deserialize)]
    struct RawSearch {
        #[serde(default)]
        artists: Vec<RawArtistEntry>,
    }
    let raw: RawSearch = serde_json::from_str(json).map_err(|error| {
        Error::Invalid(format!("Unexpected MusicBrainz search result: {error}"))
    })?;
    Ok(raw
        .artists
        .into_iter()
        .map(|artist| ArtistHit {
            score: artist.score.unwrap_or(0),
            artist: artist.into_artist(),
        })
        .collect())
}

pub fn parse_release(json: &str) -> Result<Release, Error> {
    let raw: RawRelease = serde_json::from_str(json)
        .map_err(|error| Error::Invalid(format!("Unexpected MusicBrainz release: {error}")))?;
    Ok(raw.into_release())
}

pub fn parse_search(json: &str) -> Result<Vec<SearchHit>, Error> {
    #[derive(Deserialize)]
    struct RawSearch {
        #[serde(default)]
        releases: Vec<RawRelease>,
    }
    let raw: RawSearch = serde_json::from_str(json).map_err(|error| {
        Error::Invalid(format!("Unexpected MusicBrainz search result: {error}"))
    })?;
    Ok(raw
        .releases
        .into_iter()
        .map(|release| SearchHit {
            score: release.score.unwrap_or(0),
            release: release.into_release(),
        })
        .collect())
}

// The subset of MusicBrainz's JSON that `Release` is made from. Everything
// is optional or defaulted: what's missing is left out, not an error.

#[derive(Deserialize)]
#[serde(rename_all = "kebab-case")]
struct RawRelease {
    id: String,
    #[serde(default)]
    title: String,
    score: Option<u32>,
    #[serde(default)]
    artist_credit: Vec<RawCredit>,
    date: Option<String>,
    country: Option<String>,
    status: Option<String>,
    barcode: Option<String>,
    #[serde(default)]
    label_info: Vec<RawLabelInfo>,
    release_group: Option<RawReleaseGroup>,
    #[serde(default)]
    genres: Vec<RawGenre>,
    track_count: Option<u32>,
    cover_art_archive: Option<RawCoverArt>,
    #[serde(default)]
    media: Vec<RawMedium>,
    disambiguation: Option<String>,
}

#[derive(Deserialize)]
struct RawCredit {
    name: String,
    #[serde(default)]
    joinphrase: String,
    artist: Option<RawArtist>,
}

#[derive(Deserialize)]
struct RawArtist {
    id: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "kebab-case")]
struct RawLabelInfo {
    catalog_number: Option<String>,
    label: Option<RawLabel>,
}

#[derive(Deserialize)]
struct RawLabel {
    name: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "kebab-case")]
struct RawReleaseGroup {
    id: String,
    primary_type: Option<String>,
    #[serde(default)]
    secondary_types: Vec<String>,
    first_release_date: Option<String>,
    #[serde(default)]
    genres: Vec<RawGenre>,
}

#[derive(Deserialize)]
struct RawGenre {
    name: String,
    #[serde(default)]
    count: u32,
}

#[derive(Deserialize)]
struct RawCoverArt {
    #[serde(default)]
    front: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "kebab-case")]
struct RawMedium {
    position: Option<u32>,
    format: Option<String>,
    track_count: Option<u32>,
    #[serde(default)]
    tracks: Vec<RawTrack>,
}

#[derive(Deserialize)]
struct RawTrack {
    position: Option<u32>,
    #[serde(default)]
    title: String,
    length: Option<u32>,
    recording: Option<RawRecording>,
}

#[derive(Deserialize)]
struct RawRecording {
    id: String,
    length: Option<u32>,
}

#[derive(Deserialize)]
#[serde(rename_all = "kebab-case")]
struct RawArtistEntry {
    id: String,
    #[serde(default)]
    name: String,
    score: Option<u32>,
    sort_name: Option<String>,
    disambiguation: Option<String>,
    #[serde(rename = "type")]
    artist_type: Option<String>,
    area: Option<RawArea>,
    begin_area: Option<RawArea>,
    end_area: Option<RawArea>,
    life_span: Option<RawLifeSpan>,
    #[serde(default)]
    aliases: Vec<RawAlias>,
    #[serde(default)]
    genres: Vec<RawGenre>,
    #[serde(default)]
    relations: Vec<RawRelation>,
}

#[derive(Deserialize)]
struct RawArea {
    name: Option<String>,
}

#[derive(Deserialize)]
struct RawLifeSpan {
    begin: Option<String>,
    end: Option<String>,
    ended: Option<bool>,
}

#[derive(Deserialize)]
struct RawAlias {
    name: String,
}

#[derive(Deserialize)]
struct RawRelation {
    #[serde(rename = "type")]
    relation_type: String,
    url: Option<RawUrl>,
    artist: Option<RawRelatedArtist>,
    #[serde(default)]
    ended: bool,
}

#[derive(Deserialize)]
struct RawRelatedArtist {
    id: String,
    #[serde(default)]
    name: String,
}

#[derive(Deserialize)]
struct RawUrl {
    resource: String,
}

/// An entity's links to other sites: current ones only, of the kinds the
/// app follows.
struct Links<'a>(&'a [RawRelation]);

impl Links<'_> {
    fn of(&self, kind: &'static str) -> impl Iterator<Item = &str> {
        self.0
            .iter()
            .filter(move |relation| relation.relation_type == kind && !relation.ended)
            .filter_map(|relation| relation.url.as_ref())
            .map(|url| url.resource.as_str())
    }

    fn wikidata(&self) -> Option<String> {
        self.of("wikidata").find_map(wikidata_id)
    }

    fn wikipedia(&self) -> Option<String> {
        self.of("wikipedia")
            .find(|url| url.starts_with("https://en.wikipedia.org/wiki/"))
            .map(String::from)
    }

    fn homepage(&self) -> Option<String> {
        self.of("official homepage")
            .find(|url| url.starts_with("https://") || url.starts_with("http://"))
            .map(String::from)
    }
}

impl RawArtistEntry {
    fn into_artist(self) -> Artist {
        let links = Links(&self.relations);
        let (wikidata, wikipedia, homepage) =
            (links.wikidata(), links.wikipedia(), links.homepage());
        let area = |area: Option<RawArea>| non_empty(area.and_then(|area| area.name));
        let (begin, end, ended) = match self.life_span {
            Some(span) => (span.begin, span.end, span.ended.unwrap_or(false)),
            None => (None, None, false),
        };
        Artist {
            id: self.id,
            name: self.name,
            sort_name: non_empty(self.sort_name),
            disambiguation: non_empty(self.disambiguation),
            artist_type: non_empty(self.artist_type),
            area: area(self.area),
            begin_area: area(self.begin_area),
            end_area: area(self.end_area),
            begin: non_empty(begin),
            end: non_empty(end),
            ended,
            aliases: self.aliases.into_iter().map(|alias| alias.name).collect(),
            genres: genre_names(self.genres),
            wikidata,
            wikipedia,
            homepage,
            related: self
                .relations
                .iter()
                .filter(|relation| RELATED_KINDS.contains(&relation.relation_type.as_str()))
                .filter_map(|relation| {
                    let artist = relation.artist.as_ref()?;
                    Some(RelatedArtist {
                        id: artist.id.clone(),
                        name: artist.name.clone(),
                        relation: relation.relation_type.clone(),
                    })
                })
                .collect(),
        }
    }
}

/// The item id in a Wikidata URL, e.g. "Q44190" from
/// "https://www.wikidata.org/wiki/Q44190".
fn wikidata_id(url: &str) -> Option<String> {
    let id = url.strip_prefix("https://www.wikidata.org/wiki/")?;
    let digits = id.strip_prefix('Q')?;
    (!digits.is_empty() && digits.len() <= 12 && digits.bytes().all(|b| b.is_ascii_digit()))
        .then(|| id.to_owned())
}

/// Genre names, most voted first (ties by name).
fn genre_names(mut genres: Vec<RawGenre>) -> Vec<String> {
    genres.sort_by(|a, b| b.count.cmp(&a.count).then_with(|| a.name.cmp(&b.name)));
    genres.into_iter().map(|genre| genre.name).collect()
}

/// An artist credit as printed: each name followed by its join phrase.
fn credit_text(credit: &[RawCredit]) -> String {
    credit
        .iter()
        .map(|credit| format!("{}{}", credit.name, credit.joinphrase))
        .collect()
}

fn non_empty(text: Option<String>) -> Option<String> {
    text.filter(|text| !text.trim().is_empty())
}

impl RawRelease {
    fn into_release(self) -> Release {
        let artist = credit_text(&self.artist_credit);
        let artist_ids = self
            .artist_credit
            .iter()
            .filter_map(|credit| credit.artist.as_ref().map(|artist| artist.id.clone()))
            .collect();
        let mut tracks = Vec::new();
        for (index, medium) in self.media.iter().enumerate() {
            let disc = medium.position.unwrap_or(index as u32 + 1);
            for (index, track) in medium.tracks.iter().enumerate() {
                tracks.push(ReleaseTrack {
                    disc,
                    position: track.position.unwrap_or(index as u32 + 1),
                    title: track.title.clone(),
                    length_ms: track
                        .length
                        .or_else(|| track.recording.as_ref().and_then(|r| r.length)),
                    recording_id: track.recording.as_ref().map(|r| r.id.clone()),
                });
            }
        }
        let track_count = self.track_count.unwrap_or_else(|| {
            self.media
                .iter()
                .map(|medium| medium.track_count.unwrap_or(medium.tracks.len() as u32))
                .sum()
        });
        let (release_group_id, release_type, secondary_types, first_release_date, group_genres) =
            match self.release_group {
                Some(group) => (
                    Some(group.id),
                    group.primary_type,
                    group.secondary_types,
                    group.first_release_date,
                    group.genres,
                ),
                None => (None, None, Vec::new(), None, Vec::new()),
            };
        let genres = if self.genres.is_empty() {
            group_genres
        } else {
            self.genres
        };
        Release {
            id: self.id,
            title: self.title,
            artist,
            artist_ids,
            date: non_empty(self.date),
            country: non_empty(self.country),
            status: non_empty(self.status),
            barcode: non_empty(self.barcode),
            labels: self
                .label_info
                .into_iter()
                .map(|info| Label {
                    name: non_empty(info.label.and_then(|label| label.name)),
                    catalog_number: non_empty(info.catalog_number),
                })
                .filter(|label| label.name.is_some() || label.catalog_number.is_some())
                .collect(),
            release_group_id,
            release_type: non_empty(release_type),
            secondary_types,
            first_release_date: non_empty(first_release_date),
            genres: genre_names(genres),
            track_count,
            has_front_art: self.cover_art_archive.map(|art| art.front),
            tracks,
            formats: self
                .media
                .into_iter()
                .map(|medium| non_empty(medium.format))
                .collect(),
            disambiguation: non_empty(self.disambiguation),
            styles: Vec::new(),
            credits: Vec::new(),
        }
    }
}

#[cfg(test)]
pub mod fixtures {
    //! Recorded MusicBrainz responses (trimmed; MusicBrainz data is CC0)
    //! for "In Rainbows" by Radiohead: a search and four of its releases.

    pub const SEARCH: &str = include_str!("fixtures/musicbrainz/search-in-rainbows.json");

    /// The releases in the search, by id.
    pub const RELEASES: [(&str, &str); 4] = [
        (
            "3b408cb5-7d51-4188-b07c-fabcf308cda3",
            include_str!("fixtures/musicbrainz/release-3b408cb5-7d51-4188-b07c-fabcf308cda3.json"),
        ),
        (
            "1a33443c-3fff-450f-8298-efbc65659d32",
            include_str!("fixtures/musicbrainz/release-1a33443c-3fff-450f-8298-efbc65659d32.json"),
        ),
        (
            "200f0a5a-41c4-49f3-b997-1ef5f975f060",
            include_str!("fixtures/musicbrainz/release-200f0a5a-41c4-49f3-b997-1ef5f975f060.json"),
        ),
        (
            "219e7d7c-806c-44b3-9972-cdb3614b3411",
            include_str!("fixtures/musicbrainz/release-219e7d7c-806c-44b3-9972-cdb3614b3411.json"),
        ),
    ];

    pub const RELEASE_GROUP: &str = "6e335887-60ba-38f0-95af-fae7774336bf";

    /// `RELEASES[3]`'s links (trimmed to a few fields): one, to Discogs.
    pub const RELEASE_LINKS: (&str, &str) = (
        "219e7d7c-806c-44b3-9972-cdb3614b3411",
        include_str!(
            "fixtures/musicbrainz/release-links-219e7d7c-806c-44b3-9972-cdb3614b3411.json"
        ),
    );
    /// The Discogs release it links to.
    pub const RELEASE_LINKS_DISCOGS: &str = "1187003";

    /// Radiohead: a search for the name, and the artist with a few of its
    /// links (to Wikidata, the homepage, and others the app ignores).
    pub const ARTIST: &str = "a74b1b7f-71a5-4011-9441-d0b5e4122711";
    pub const ARTIST_JSON: &str =
        include_str!("fixtures/musicbrainz/artist-a74b1b7f-71a5-4011-9441-d0b5e4122711.json");
    pub const ARTIST_SEARCH: &str =
        include_str!("fixtures/musicbrainz/search-artist-radiohead.json");

    /// The release group of "In Rainbows" (`RELEASE_GROUP`), with its link
    /// to Wikidata and one the app ignores.
    pub const RELEASE_GROUP_JSON: &str = include_str!(
        "fixtures/musicbrainz/release-group-6e335887-60ba-38f0-95af-fae7774336bf.json"
    );

    /// Radiohead's release groups (`ARTIST`), trimmed to 14 of the 106
    /// with their count to match: albums, EPs, singles, live albums,
    /// compilations, a collaboration and an undated broadcast.
    pub const RELEASE_GROUPS_JSON: &str = include_str!(
        "fixtures/musicbrainz/release-groups-a74b1b7f-71a5-4011-9441-d0b5e4122711.json"
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::library::db;
    use crate::metadata::http::testing::fake_client;

    #[test]
    fn recognizes_mbids() {
        assert!(is_mbid("3b408cb5-7d51-4188-b07c-fabcf308cda3"));
        assert!(is_mbid("3B408CB5-7D51-4188-B07C-FABCF308CDA3"));
        for bad in [
            "",
            "3b408cb5-7d51-4188-b07c-fabcf308cda",
            "3b408cb5-7d51-4188-b07c-fabcf308cdaz",
            "3b408cb5_7d51-4188-b07c-fabcf308cda3",
            "../../../../../../../../../../../..",
        ] {
            assert!(!is_mbid(bad), "{bad}");
        }
    }

    #[test]
    fn finds_mbids_in_urls() {
        let id = "3b408cb5-7d51-4188-b07c-fabcf308cda3";
        assert_eq!(mbid_in(&format!(" {id}\n"), "release"), Some(id));
        assert_eq!(
            mbid_in(
                &format!("https://musicbrainz.org/release/{id}/cover-art"),
                "release"
            ),
            Some(id)
        );
        for bad in [
            format!("https://musicbrainz.org/artist/{id}"),
            format!("https://example.com/release/{id}"),
            "https://musicbrainz.org/release/3b408cb5".into(),
            "In Rainbows".into(),
        ] {
            assert_eq!(mbid_in(&bad, "release"), None, "{bad}");
        }
    }

    #[test]
    fn builds_escaped_search_urls() {
        assert_eq!(
            search_url("In Rainbows", Some("Radiohead"), Some(10)),
            format!(
                "{BASE}/release/?query=release%3A%28In%20Rainbows%29%20AND%20\
                 artist%3A%28Radiohead%29%20tracks%3A10&limit=10&fmt=json"
            )
        );
        // Lucene syntax in a title is searched for as text.
        assert_eq!(
            escape_lucene("AC/DC: (Live!) \"1991\""),
            "AC\\/DC\\: \\(Live\\!\\) \\\"1991\\\""
        );
        assert_eq!(percent_encode("é&?=#"), "%C3%A9%26%3F%3D%23");
        assert!(!search_url("x", None, None).contains("artist"));
    }

    #[test]
    fn parses_a_search() {
        let hits = parse_search(fixtures::SEARCH).unwrap();
        assert_eq!(hits.len(), 5);
        let first = &hits[0];
        assert_eq!(first.score, 100);
        assert_eq!(first.release.id, "3b408cb5-7d51-4188-b07c-fabcf308cda3");
        assert_eq!(first.release.title, "In Rainbows");
        assert_eq!(first.release.artist, "Radiohead");
        assert_eq!(first.release.track_count, 10);
        assert_eq!(first.release.year(), Some(2007));
        assert_eq!(first.release.release_type.as_deref(), Some("Album"));
        assert!(first.release.tracks.is_empty());
        assert_eq!(first.release.has_front_art, None);
        assert!(hits
            .iter()
            .all(|hit| hit.release.release_group_id.as_deref() == Some(fixtures::RELEASE_GROUP)));
    }

    #[test]
    fn parses_a_release() {
        let release = parse_release(fixtures::RELEASES[1].1).unwrap();
        assert_eq!(release.id, fixtures::RELEASES[1].0);
        assert_eq!(release.date.as_deref(), Some("2016-05-06"));
        assert_eq!(release.country.as_deref(), Some("XW"));
        assert_eq!(release.barcode.as_deref(), Some("634904032463"));
        assert_eq!(
            release.labels,
            [Label {
                name: Some("XL Recordings".into()),
                catalog_number: Some("XLDA324".into())
            }]
        );
        assert_eq!(release.original_year(), Some(2007));
        assert_eq!(release.has_front_art, Some(true));
        assert_eq!(
            release.genres.first().map(String::as_str),
            Some("alternative rock")
        );
        assert_eq!(release.tracks.len(), 10);
        let first = &release.tracks[0];
        assert_eq!((first.disc, first.position), (1, 1));
        assert_eq!(first.title, "15 Step");
        assert_eq!(first.length_ms, Some(237000));
        assert_eq!(release.artist_ids, ["a74b1b7f-71a5-4011-9441-d0b5e4122711"]);
        assert_eq!(release.formats, [Some("Digital Media".to_owned())]);

        let vinyl = parse_release(fixtures::RELEASES[3].1).unwrap();
        assert_eq!(vinyl.formats, [Some("12\" Vinyl".to_owned())]);
        // Details stored before formats were kept still parse.
        let mut old = serde_json::to_value(&vinyl).unwrap();
        old.as_object_mut().unwrap().remove("formats");
        old.as_object_mut().unwrap().remove("disambiguation");
        let old: Release = serde_json::from_value(old).unwrap();
        assert!(old.formats.is_empty());
    }

    #[test]
    fn parses_an_artist_and_its_links() {
        let artist = parse_artist(fixtures::ARTIST_JSON).unwrap();
        assert_eq!(artist.id, fixtures::ARTIST);
        assert_eq!(artist.name, "Radiohead");
        assert_eq!(artist.artist_type.as_deref(), Some("Group"));
        assert_eq!(artist.area.as_deref(), Some("United Kingdom"));
        assert_eq!(artist.begin_area.as_deref(), Some("Abingdon-on-Thames"));
        assert_eq!(
            (artist.begin.as_deref(), artist.end, artist.ended),
            (Some("1991"), None, false)
        );
        assert_eq!(artist.disambiguation, None, "an empty one is none");
        assert_eq!(artist.genres[..2], ["alternative rock", "art rock"]);
        assert_eq!(artist.wikidata.as_deref(), Some("Q44190"));
        assert_eq!(artist.wikipedia, None);
        assert_eq!(
            artist.homepage.as_deref(),
            Some("http://www.radiohead.com/")
        );
    }

    #[test]
    fn follows_only_usable_links() {
        let artist = parse_artist(
            r#"{"id": "x", "relations": [
                {"type": "wikidata", "url": {"resource": "https://www.wikidata.org/wiki/Q1"}, "ended": true},
                {"type": "wikidata", "url": {"resource": "https://www.wikidata.org/wiki/Q2/../Q3"}},
                {"type": "wikidata", "url": {"resource": "https://www.wikidata.org/wiki/Q42"}},
                {"type": "wikipedia", "url": {"resource": "https://fr.wikipedia.org/wiki/X"}},
                {"type": "wikipedia", "url": {"resource": "https://en.wikipedia.org/wiki/X"}},
                {"type": "official homepage", "url": {"resource": "ftp://x.example/"}},
                {"type": "official homepage"}
            ]}"#,
        )
        .unwrap();
        assert_eq!(artist.wikidata.as_deref(), Some("Q42"));
        assert_eq!(
            artist.wikipedia.as_deref(),
            Some("https://en.wikipedia.org/wiki/X")
        );
        assert_eq!(artist.homepage, None);
    }

    #[test]
    fn parses_a_release_group_and_its_links() {
        let group = parse_release_group(fixtures::RELEASE_GROUP_JSON).unwrap();
        assert_eq!(group.id, fixtures::RELEASE_GROUP);
        assert_eq!(group.title, "In Rainbows");
        assert_eq!(group.wikidata.as_deref(), Some("Q223295"));
        assert_eq!(group.wikipedia, None);
        let bare = parse_release_group(r#"{"id": "x"}"#).unwrap();
        assert_eq!((bare.wikidata, bare.wikipedia), (None, None));
        assert!(parse_release_group("[]").is_err());
    }

    #[test]
    fn parses_an_artists_release_groups() {
        let page = parse_release_groups(fixtures::RELEASE_GROUPS_JSON).unwrap();
        assert_eq!(page.count, 14);
        assert_eq!(page.groups.len(), 14);
        let in_rainbows = page
            .groups
            .iter()
            .find(|group| group.id == fixtures::RELEASE_GROUP)
            .unwrap();
        assert_eq!(in_rainbows.title, "In Rainbows");
        assert_eq!(in_rainbows.artist, "Radiohead");
        assert_eq!(in_rainbows.release_type.as_deref(), Some("Album"));
        assert_eq!(
            in_rainbows.first_release_date.as_deref(),
            Some("2007-10-10")
        );
        assert_eq!(in_rainbows.disambiguation, None, "an empty one is none");
        let remix = page
            .groups
            .iter()
            .find(|group| group.artist != "Radiohead")
            .unwrap();
        assert_eq!(remix.artist, "DJ Shadow vs. Radiohead");
        assert_eq!(remix.secondary_types, ["Remix"]);
        let undated = page
            .groups
            .iter()
            .find(|group| group.first_release_date.is_none())
            .unwrap();
        assert_eq!(undated.release_type.as_deref(), Some("Broadcast"));

        assert_eq!(
            release_groups_url(fixtures::ARTIST, 100),
            format!(
                "{BASE}/release-group?artist={}&release-group-status=website-default\
                 &inc=artist-credits&limit=100&offset=100&fmt=json",
                fixtures::ARTIST
            )
        );
        let bare = parse_release_groups(r#"{"release-groups": [{"id": "x"}]}"#).unwrap();
        assert_eq!(bare.count, 1);
        assert!(parse_release_groups("[]").is_err());
    }

    #[test]
    fn parses_an_artist_search() {
        let hits = parse_artist_search(fixtures::ARTIST_SEARCH).unwrap();
        assert_eq!(hits[0].score, 100);
        assert_eq!(hits[0].artist.id, fixtures::ARTIST);
        assert!(hits[1..].iter().all(|hit| hit.score <= 60));
        assert_eq!(
            artist_search_url("AC/DC"),
            format!(
                "{BASE}/artist/?query=artist%3A%28AC%5C%2FDC%29%20OR%20\
                 alias%3A%28AC%5C%2FDC%29&limit=10&fmt=json"
            )
        );
    }

    #[test]
    fn tolerates_missing_fields_and_refuses_garbage() {
        let release = parse_release(r#"{"id": "x", "media": [{"tracks": [{}, {}]}]}"#).unwrap();
        assert_eq!(release.title, "");
        assert_eq!(release.track_count, 2);
        assert_eq!(release.tracks[1].position, 2);
        assert!(parse_release("[]").is_err());
        assert!(parse_search("not json").is_err());
    }

    /// Against the real service: `cargo test live_ -- --ignored`.
    #[test]
    #[ignore]
    fn live_search_and_lookup() {
        use crate::metadata::http::{SystemClock, UreqTransport};
        let conn = db::open_in_memory().unwrap();
        let client = Client::new(Box::new(UreqTransport::new()), Box::new(SystemClock));
        let hits =
            search_releases(&client, &conn, "In Rainbows", Some("Radiohead"), Some(10)).unwrap();
        let hit = hits
            .iter()
            .find(|hit| hit.release.release_group_id.as_deref() == Some(fixtures::RELEASE_GROUP))
            .expect("In Rainbows is found");
        let release = lookup_release(&client, &conn, &hit.release.id).unwrap();
        assert_eq!(release.title, "In Rainbows");
        assert!(!release.tracks.is_empty());
    }

    #[test]
    fn looks_up_through_the_client_and_cache() {
        let conn = db::open_in_memory().unwrap();
        let (client, transport, _clock) = fake_client();
        let (id, json) = fixtures::RELEASES[0];
        transport.push_status(&release_url(id), 200, json);
        assert_eq!(lookup_release(&client, &conn, id).unwrap().id, id);
        assert_eq!(lookup_release(&client, &conn, id).unwrap().id, id);
        assert_eq!(transport.urls().len(), 1);
        assert!(lookup_release(&client, &conn, "../admin").is_err());
        assert_eq!(transport.urls().len(), 1, "a bad id is never requested");
    }

    #[test]
    fn parses_related_artists() {
        let artist = parse_artist(
            r#"{"id": "a", "name": "Singer", "relations": [
                {"type": "member of band", "target-type": "artist", "direction": "forward",
                 "artist": {"id": "b", "name": "The Band"}},
                {"type": "collaboration", "artist": {"id": "c", "name": "Duo"}},
                {"type": "teacher", "artist": {"id": "d", "name": "Mentor"}},
                {"type": "wikidata", "url": {"resource": "https://www.wikidata.org/wiki/Q1"}}
            ]}"#,
        )
        .unwrap();
        let related: Vec<(&str, &str)> = artist
            .related
            .iter()
            .map(|r| (r.name.as_str(), r.relation.as_str()))
            .collect();
        assert_eq!(
            related,
            [("The Band", "member of band"), ("Duo", "collaboration")]
        );
        assert_eq!(artist.wikidata.as_deref(), Some("Q1"));
        // Details stored before relationships were kept still read.
        let stored: Artist = serde_json::from_str(
            r#"{"id": "a", "name": "Old", "sortName": null, "disambiguation": null, "type": null,
                "area": null, "beginArea": null, "endArea": null, "begin": null, "end": null,
                "ended": false, "aliases": [], "genres": [], "wikidata": null, "wikipedia": null,
                "homepage": null}"#,
        )
        .unwrap();
        assert!(stored.related.is_empty());
    }
}
