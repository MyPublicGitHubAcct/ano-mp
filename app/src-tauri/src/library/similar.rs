//! Recommendations from the library (PLAN.md X4): "More like this" for a
//! track, an album or an artist, and Home's albums the user hasn't played
//! lately that resemble what they play. Everything is local; no service is
//! called.
//!
//! An item (a track, an album, an artist, or the user's taste) is a
//! `Profile` of its tracks: the genres they cover and how much of it, its
//! artists and composers, its albums' labels, its era and its loudness
//! (O1's analysis). A candidate scores (`score`, a pure function) for what
//! its profile shares with the seed's, for an artist linked to the seed's
//! (MusicBrainz relationships, or sharing an album in the library), and for
//! being played in the same listening sessions as the seed (O8). Radio (O9)
//! scores its tracks the same way.

use std::collections::{HashMap, HashSet};
use std::hash::Hash;

use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;

use super::availability::json_ids;
use super::discover::{self, AlbumCard};
use super::genres;
use super::scanner::VARIOUS_ARTISTS;
use super::sort_key::fold;
use super::Error;

/// How many recommendations a view shows.
pub const SHOWN: usize = 12;

/// Below this a candidate has too little in common to recommend (a genre
/// alone is enough; an era alone isn't). Radio takes anything above 0.
const MIN_SCORE: f64 = 3.0;

/// A profile keeps this many of its heaviest genres, artists, composers
/// and labels.
const TOP_GENRES: usize = 10;
const TOP_ARTISTS: usize = 30;
const TOP_LABELS: usize = 10;

/// The seed's artists whose links are followed.
const LINKED_FROM: usize = 10;

/// A profile has an era only while the middle half of its tracks spans at
/// most this many years.
const ERA_SPREAD: u32 = 10;

/// Plays further apart than this (from one's end to the next's start) are
/// in different listening sessions.
const SESSION_GAP: i64 = 30 * 60;

/// Home's suggestions leave out albums played this recently, and take the
/// user's taste from these plays.
pub(crate) const LATELY: i64 = 90 * 86400;

/// Plays the taste falls back on when nothing was played lately.
pub(crate) const FALLBACK_PLAYS: u32 = 200;

/// Why something was recommended, for the UI to say (`similar.reason.*`).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[cfg_attr(test, ts(rename = "SimilarReason"))]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Reason {
    /// The genre they share most.
    Genre { name: String },
    /// Its year, near the seed's.
    Era { year: u32 },
    /// A label they share.
    Label { name: String },
    /// One of its artists, linked to the seed's.
    Linked { name: String, relation: Relation },
    /// An artist they share.
    Artist { name: String },
    /// A composer they share.
    Composer { name: String },
    /// Listening sessions they were both played in.
    Together { times: u32 },
    /// About as loud (O1's analysis).
    Loudness,
}

/// How an artist is linked to the seed's.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[cfg_attr(test, ts(rename = "SimilarRelation"))]
#[serde(rename_all = "camelCase")]
pub enum Relation {
    /// A band and its member, either way (MusicBrainz).
    Member,
    /// A group and its subgroup (MusicBrainz).
    Subgroup,
    /// A collaboration on MusicBrainz, or sharing an album in the library.
    With,
}

/// A reason as scored, before its names are looked up.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum Why {
    Genre(u32),
    Era(u32),
    Label(u32),
    Linked(i64, Relation),
    Artist(i64),
    Composer(i64),
    Together(u32),
    Loudness,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct SimilarTrack {
    pub track_id: i64,
    pub title: String,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub album_id: Option<i64>,
    /// Seconds.
    pub duration: f64,
    /// The strongest first; at most two.
    pub reasons: Vec<Reason>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct SimilarAlbum {
    pub album: AlbumCard,
    pub reasons: Vec<Reason>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct SimilarArtist {
    pub artist_id: i64,
    pub name: String,
    pub reasons: Vec<Reason>,
}

/// What a recommendation is like.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Seed {
    Track(i64),
    Album(i64),
    Artist(i64),
}

/// What is recommended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Grain {
    Track,
    Album,
    Artist,
}

/// A track as the scoring reads it.
#[derive(Debug, Clone)]
pub(crate) struct TrackFacts {
    pub id: i64,
    pub folder_id: i64,
    pub artist_id: Option<i64>,
    pub album_id: Option<i64>,
    album_artist_id: Option<i64>,
    /// Its credited artists in order, then its album artist if another;
    /// never Various Artists.
    artists: Vec<i64>,
    composer_id: Option<i64>,
    genres: Vec<u32>,
    /// Its album's earliest year, else its own.
    year: Option<u32>,
    /// LUFS, when analysed.
    loudness: Option<f64>,
}

/// Every track's facts, read once per recommendation.
pub(crate) struct Catalog {
    tracks: Vec<TrackFacts>,
    index: HashMap<i64, usize>,
    /// By genre id: how the first track with it writes it.
    genre_names: Vec<String>,
    label_names: Vec<String>,
    album_labels: HashMap<i64, Vec<u32>>,
    artist_names: HashMap<i64, String>,
}

/// Interns `name` under `key`.
fn intern(ids: &mut HashMap<String, u32>, names: &mut Vec<String>, key: String, name: &str) -> u32 {
    *ids.entry(key).or_insert_with(|| {
        names.push(name.to_owned());
        names.len() as u32 - 1
    })
}

impl Catalog {
    pub(crate) fn load(conn: &Connection) -> Result<Catalog, Error> {
        let various: Option<i64> = conn
            .query_row(
                "SELECT id FROM artists WHERE name = ?1",
                [VARIOUS_ARTISTS],
                |row| row.get(0),
            )
            .optional()?;
        let mut credits: HashMap<i64, Vec<i64>> = HashMap::new();
        let mut statement = conn.prepare_cached(
            "SELECT track_id, artist_id FROM track_artists ORDER BY track_id, position",
        )?;
        for row in statement.query_map([], |row| Ok((row.get::<_, i64>(0)?, row.get(1)?)))? {
            let (track, artist) = row?;
            credits.entry(track).or_default().push(artist);
        }

        let mut genre_ids = HashMap::new();
        let mut genre_names = Vec::new();
        let mut tracks = Vec::new();
        let mut album_years: HashMap<i64, u32> = HashMap::new();
        let mut statement = conn.prepare_cached(
            "SELECT t.id, t.folder_id, t.artist_id, t.album_id, t.album_artist_id, t.composer_id,
                    t.genre, t.year, a.loudness
             FROM tracks t LEFT JOIN track_analysis a ON a.track_id = t.id
             ORDER BY t.id",
        )?;
        let rows = statement.query_map([], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, Option<i64>>(2)?,
                row.get::<_, Option<i64>>(3)?,
                row.get::<_, Option<i64>>(4)?,
                row.get::<_, Option<i64>>(5)?,
                row.get::<_, Option<String>>(6)?,
                row.get::<_, Option<u32>>(7)?,
                row.get::<_, Option<f64>>(8)?,
            ))
        })?;
        for row in rows {
            let (
                id,
                folder_id,
                artist_id,
                album_id,
                album_artist_id,
                composer_id,
                genre,
                year,
                loudness,
            ) = row?;
            let mut artists = credits
                .remove(&id)
                .unwrap_or_else(|| artist_id.into_iter().collect());
            artists.extend(album_artist_id.filter(|artist| !artists.contains(artist)));
            artists.retain(|&artist| Some(artist) != various);
            let mut genre_list = Vec::new();
            for name in genres::split(genre.as_deref().unwrap_or("")) {
                let id = intern(&mut genre_ids, &mut genre_names, fold(name), name);
                if !genre_list.contains(&id) {
                    genre_list.push(id);
                }
            }
            if let (Some(album), Some(year)) = (album_id, year) {
                let earliest = album_years.entry(album).or_insert(year);
                *earliest = (*earliest).min(year);
            }
            tracks.push(TrackFacts {
                id,
                folder_id,
                artist_id,
                album_id,
                album_artist_id: album_artist_id.filter(|&artist| Some(artist) != various),
                artists,
                composer_id,
                genres: genre_list,
                year,
                loudness,
            });
        }
        for track in &mut tracks {
            if let Some(year) = track.album_id.and_then(|album| album_years.get(&album)) {
                track.year = Some(*year);
            }
        }
        let index = tracks
            .iter()
            .enumerate()
            .map(|(i, track)| (track.id, i))
            .collect();

        let mut label_ids = HashMap::new();
        let mut label_names = Vec::new();
        let mut album_labels: HashMap<i64, Vec<u32>> = HashMap::new();
        let mut statement = conn.prepare_cached(
            "SELECT l.album_id, json_extract(label.value, '$.name')
             FROM album_links l, json_each(l.details, '$.labels') AS label
             WHERE l.source = 'musicbrainz' AND l.status = 'matched' AND l.details IS NOT NULL",
        )?;
        for row in statement.query_map([], |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, Option<String>>(1)?))
        })? {
            let (album, name) = row?;
            if let Some(name) = name.filter(|name| !name.is_empty() && name != "[no label]") {
                let id = intern(&mut label_ids, &mut label_names, fold(&name), &name);
                let labels = album_labels.entry(album).or_default();
                if !labels.contains(&id) {
                    labels.push(id);
                }
            }
        }

        let mut statement = conn.prepare_cached("SELECT id, name FROM artists")?;
        let artist_names = statement
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
            .collect::<Result<_, _>>()?;

        Ok(Catalog {
            tracks,
            index,
            genre_names,
            label_names,
            album_labels,
            artist_names,
        })
    }

    pub(crate) fn track(&self, id: i64) -> Option<&TrackFacts> {
        self.index.get(&id).map(|&i| &self.tracks[i])
    }

    pub(crate) fn genre_name(&self, id: u32) -> &str {
        &self.genre_names[id as usize]
    }

    pub(crate) fn artist_name(&self, id: i64) -> &str {
        self.artist_names.get(&id).map_or("", String::as_str)
    }

    fn label_name(&self, id: u32) -> &str {
        &self.label_names[id as usize]
    }

    /// The tracks a seed is made of, whatever their folders.
    fn seed_tracks(&self, seed: Seed) -> Vec<&TrackFacts> {
        match seed {
            Seed::Track(id) => self.track(id).into_iter().collect(),
            Seed::Album(id) => self
                .tracks
                .iter()
                .filter(|track| track.album_id == Some(id))
                .collect(),
            Seed::Artist(id) => self
                .tracks
                .iter()
                .filter(|track| track.artists.contains(&id))
                .collect(),
        }
    }

    /// The keys of `grain` a track belongs to.
    fn keys<'a>(&self, track: &'a TrackFacts, grain: Grain) -> &'a [i64] {
        match grain {
            Grain::Track => std::slice::from_ref(&track.id),
            Grain::Album => track.album_id.as_slice(),
            Grain::Artist => &track.artists,
        }
    }

    /// The profile of `tracks`, each with its weight.
    pub(crate) fn profile<'a>(
        &self,
        tracks: impl IntoIterator<Item = (&'a TrackFacts, f64)>,
    ) -> Profile {
        let mut total = 0.0;
        let mut genres: HashMap<u32, f64> = HashMap::new();
        let mut artists: HashMap<i64, f64> = HashMap::new();
        let mut composers: HashMap<i64, f64> = HashMap::new();
        let mut labels: HashMap<u32, f64> = HashMap::new();
        let mut years: Vec<(u32, f64)> = Vec::new();
        let (mut loudness, mut loudness_weight) = (0.0, 0.0);
        for (track, weight) in tracks {
            if weight <= 0.0 {
                continue;
            }
            total += weight;
            for &genre in &track.genres {
                *genres.entry(genre).or_default() += weight;
            }
            for &artist in &track.artists {
                *artists.entry(artist).or_default() += weight;
            }
            if let Some(composer) = track.composer_id {
                *composers.entry(composer).or_default() += weight;
            }
            for &label in track
                .album_id
                .and_then(|album| self.album_labels.get(&album))
                .map_or(&[][..], Vec::as_slice)
            {
                *labels.entry(label).or_default() += weight;
            }
            if let Some(year) = track.year {
                years.push((year, weight));
            }
            if let Some(lufs) = track.loudness {
                loudness += lufs * weight;
                loudness_weight += weight;
            }
        }
        if total == 0.0 {
            return Profile::default();
        }
        Profile {
            genres: heaviest(genres, TOP_GENRES)
                .into_iter()
                .map(|(genre, weight)| (genre, weight / total))
                .collect(),
            artists: keys_of(heaviest(artists, TOP_ARTISTS)),
            composers: keys_of(heaviest(composers, TOP_ARTISTS)),
            labels: keys_of(heaviest(labels, TOP_LABELS)),
            year: era(years),
            loudness: (loudness_weight > 0.0).then(|| loudness / loudness_weight),
        }
    }

    /// What candidates are compared with: `profile`, and the artists
    /// linked to its heaviest ones.
    fn seed_index(&self, conn: &Connection, profile: &Profile) -> Result<SeedIndex, Error> {
        let artists: HashSet<i64> = profile.artists.iter().copied().collect();
        let mut linked = HashMap::new();
        for &artist in profile.artists.iter().take(LINKED_FROM) {
            for (other, relation) in linked_artists(conn, artist)? {
                if !artists.contains(&other) {
                    linked.entry(other).or_insert(relation);
                }
            }
        }
        Ok(SeedIndex {
            genres: profile.genres.iter().copied().collect(),
            artists,
            composers: profile.composers.iter().copied().collect(),
            labels: profile.labels.iter().copied().collect(),
            year: profile.year,
            loudness: profile.loudness,
            linked,
        })
    }

    /// The reasons to show for `whys`: the two strongest worth a point.
    fn reasons(&self, whys: &[(f64, Why)]) -> Vec<Reason> {
        whys.iter()
            .filter(|(points, _)| *points >= 1.0)
            .take(2)
            .map(|&(_, why)| match why {
                Why::Genre(id) => Reason::Genre {
                    name: self.genre_name(id).to_owned(),
                },
                Why::Era(year) => Reason::Era { year },
                Why::Label(id) => Reason::Label {
                    name: self.label_name(id).to_owned(),
                },
                Why::Linked(artist, relation) => Reason::Linked {
                    name: self.artist_name(artist).to_owned(),
                    relation,
                },
                Why::Artist(artist) => Reason::Artist {
                    name: self.artist_name(artist).to_owned(),
                },
                Why::Composer(artist) => Reason::Composer {
                    name: self.artist_name(artist).to_owned(),
                },
                Why::Together(times) => Reason::Together { times },
                Why::Loudness => Reason::Loudness,
            })
            .collect()
    }
}

/// The `n` heaviest entries, heaviest first (ties by key).
fn heaviest<K: Copy + Ord + Hash>(weights: HashMap<K, f64>, n: usize) -> Vec<(K, f64)> {
    let mut entries: Vec<(K, f64)> = weights.into_iter().collect();
    entries.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
    entries.truncate(n);
    entries
}

fn keys_of<K>(entries: Vec<(K, f64)>) -> Vec<K> {
    entries.into_iter().map(|(key, _)| key).collect()
}

/// The weighted median year, unless the middle half spans more than
/// `ERA_SPREAD` years.
fn era(mut years: Vec<(u32, f64)>) -> Option<u32> {
    let total: f64 = years.iter().map(|(_, weight)| weight).sum();
    if total == 0.0 {
        return None;
    }
    years.sort_by_key(|&(year, _)| year);
    let quantile = |p: f64| {
        let mut sum = 0.0;
        for &(year, weight) in &years {
            sum += weight;
            if sum >= p * total {
                return year;
            }
        }
        years[years.len() - 1].0
    };
    (quantile(0.75) - quantile(0.25) <= ERA_SPREAD).then(|| quantile(0.5))
}

/// What an item is like.
#[derive(Debug, Default, Clone, PartialEq)]
pub(crate) struct Profile {
    /// Genre ids and the share of its tracks (by weight) in each, the
    /// largest first.
    pub genres: Vec<(u32, f64)>,
    /// The heaviest first.
    pub artists: Vec<i64>,
    pub composers: Vec<i64>,
    pub labels: Vec<u32>,
    pub year: Option<u32>,
    pub loudness: Option<f64>,
}

/// A seed's profile, for looking up what candidates share with it.
#[derive(Debug, Default)]
pub(crate) struct SeedIndex {
    pub genres: HashMap<u32, f64>,
    pub artists: HashSet<i64>,
    pub composers: HashSet<i64>,
    pub labels: HashSet<u32>,
    pub year: Option<u32>,
    pub loudness: Option<f64>,
    /// Artists linked to the seed's (never the seed's own), and how.
    pub linked: HashMap<i64, Relation>,
}

/// How much `candidate` is like `seed`, and why, the strongest reason
/// first. `together` is how many listening sessions they share. A shared
/// artist counts for less between tracks (radio's "more by") and not at
/// all between artists, where it only means they played together, which
/// `linked` already says.
pub(crate) fn score(
    seed: &SeedIndex,
    candidate: &Profile,
    together: u32,
    grain: Grain,
) -> (f64, Vec<(f64, Why)>) {
    let mut whys: Vec<(f64, Why)> = Vec::new();

    let mut overlap = 0.0;
    let mut best: Option<(f64, u32)> = None;
    for &(genre, share) in &candidate.genres {
        if let Some(&theirs) = seed.genres.get(&genre) {
            let common = share.min(theirs);
            overlap += common;
            if best.is_none_or(|(most, _)| common > most) {
                best = Some((common, genre));
            }
        }
    }
    if let Some((_, genre)) = best {
        // 3 for one genre all through, 1 more for a second.
        let points = 3.0 * overlap.min(1.0) + (overlap - 1.0).clamp(0.0, 1.0);
        whys.push((points, Why::Genre(genre)));
    }
    if let (Some(a), Some(b)) = (seed.year, candidate.year) {
        let points = match a.abs_diff(b) {
            0..=2 => 2.0,
            3..=5 => 1.0,
            _ => 0.0,
        };
        if points > 0.0 {
            whys.push((points, Why::Era(b)));
        }
    }
    if let Some(&label) = candidate
        .labels
        .iter()
        .find(|label| seed.labels.contains(label))
    {
        whys.push((3.0, Why::Label(label)));
    }
    if let Some((&artist, &relation)) = candidate
        .artists
        .iter()
        .find_map(|artist| seed.linked.get_key_value(artist))
    {
        whys.push((4.0, Why::Linked(artist, relation)));
    }
    let shared_artist = candidate
        .artists
        .iter()
        .find(|artist| seed.artists.contains(artist));
    let artist_points = match grain {
        Grain::Track => 1.0,
        Grain::Album => 2.0,
        Grain::Artist => 0.0,
    };
    if let Some(&artist) = shared_artist.filter(|_| artist_points > 0.0) {
        whys.push((artist_points, Why::Artist(artist)));
    }
    if let Some(&composer) = candidate
        .composers
        .iter()
        .find(|composer| seed.composers.contains(composer) && !candidate.artists.contains(composer))
    {
        whys.push((2.0, Why::Composer(composer)));
    }
    if together > 0 {
        let points = (2.0 * (1.0 + together as f64).log2()).min(4.0);
        whys.push((points, Why::Together(together)));
    }
    let mut total: f64 = whys.iter().map(|(points, _)| points).sum();
    // Loudness alone says nothing; it tips the balance between others.
    if total >= 1.0 {
        if let (Some(a), Some(b)) = (seed.loudness, candidate.loudness) {
            if (a - b).abs() <= 1.5 {
                whys.push((1.0, Why::Loudness));
                total += 1.0;
            }
        }
    }
    whys.sort_by(|a, b| b.0.total_cmp(&a.0));
    (total, whys)
}

/// A candidate and its score.
#[derive(Debug, Clone)]
pub(crate) struct Scored {
    pub key: i64,
    pub score: f64,
    pub whys: Vec<(f64, Why)>,
}

/// Every candidate of `grain` but those in `exclude`, made of its tracks
/// outside the folders `unreadable`, scored against `seed`, best first;
/// those with nothing in common are left out.
pub(crate) fn scored(
    conn: &Connection,
    catalog: &Catalog,
    seed: Seed,
    grain: Grain,
    exclude: &HashSet<i64>,
    unreadable: &[i64],
) -> Result<Vec<Scored>, Error> {
    let tracks = catalog.seed_tracks(seed);
    if tracks.is_empty() {
        return Ok(Vec::new());
    }
    let profile = catalog.profile(tracks.iter().map(|&track| (track, 1.0)));
    let index = catalog.seed_index(conn, &profile)?;
    let ids: HashSet<i64> = tracks.iter().map(|track| track.id).collect();
    let together = together(conn, catalog, &ids, grain)?;
    Ok(candidates(
        catalog, &index, grain, exclude, unreadable, &together,
    ))
}

fn candidates(
    catalog: &Catalog,
    seed: &SeedIndex,
    grain: Grain,
    exclude: &HashSet<i64>,
    unreadable: &[i64],
    together: &HashMap<i64, u32>,
) -> Vec<Scored> {
    let unreadable: HashSet<i64> = unreadable.iter().copied().collect();
    let mut groups: HashMap<i64, Vec<&TrackFacts>> = HashMap::new();
    for track in &catalog.tracks {
        if unreadable.contains(&track.folder_id) {
            continue;
        }
        for &key in catalog.keys(track, grain) {
            if !exclude.contains(&key) {
                groups.entry(key).or_default().push(track);
            }
        }
    }
    let mut scored: Vec<Scored> = groups
        .into_iter()
        .filter_map(|(key, tracks)| {
            let profile = catalog.profile(tracks.into_iter().map(|track| (track, 1.0)));
            let times = together.get(&key).copied().unwrap_or(0);
            let (score, whys) = score(seed, &profile, times, grain);
            (score > 0.0).then_some(Scored { key, score, whys })
        })
        .collect();
    scored.sort_by(|a, b| b.score.total_cmp(&a.score).then(a.key.cmp(&b.key)));
    scored
}

/// For each key of `grain`, how many listening sessions played one of
/// its tracks along with one of `seed` (a key's tracks among the seed's
/// don't count).
fn together(
    conn: &Connection,
    catalog: &Catalog,
    seed: &HashSet<i64>,
    grain: Grain,
) -> Result<HashMap<i64, u32>, Error> {
    let mut counts: HashMap<i64, u32> = HashMap::new();
    let mut close = |session: &mut Vec<i64>| {
        if session.iter().any(|track| seed.contains(track)) {
            let keys: HashSet<i64> = session
                .iter()
                .filter(|track| !seed.contains(track))
                .filter_map(|&track| catalog.track(track))
                .flat_map(|track| catalog.keys(track, grain).iter().copied())
                .collect();
            for key in keys {
                *counts.entry(key).or_default() += 1;
            }
        }
        session.clear();
    };
    let mut statement = conn
        .prepare_cached("SELECT track_id, played_at, seconds FROM plays ORDER BY played_at, id")?;
    let mut session: Vec<i64> = Vec::new();
    let mut end = i64::MIN;
    for row in statement.query_map([], |row| {
        Ok((
            row.get::<_, i64>(0)?,
            row.get::<_, i64>(1)?,
            row.get::<_, f64>(2)?,
        ))
    })? {
        let (track, at, seconds) = row?;
        if at > end.saturating_add(SESSION_GAP) {
            close(&mut session);
        }
        session.push(track);
        end = end.max(at + seconds as i64);
    }
    close(&mut session);
    Ok(counts)
}

/// Artists linked to `artist_id`: MusicBrainz relationships either way,
/// and artists sharing an album with them in the library.
pub(crate) fn linked_artists(
    conn: &Connection,
    artist_id: i64,
) -> Result<HashMap<i64, Relation>, Error> {
    let mut linked = HashMap::new();
    let details: Option<(Option<String>, Option<String>)> = conn
        .query_row(
            "SELECT external_id, details FROM artist_links
             WHERE artist_id = ?1 AND source = 'musicbrainz' AND status = 'matched'",
            [artist_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()?;
    if let Some((mbid, details)) = details {
        // Theirs.
        let related: Vec<crate::metadata::musicbrainz::RelatedArtist> = details
            .and_then(|details| serde_json::from_str::<serde_json::Value>(&details).ok())
            .and_then(|details| serde_json::from_value(details["related"].clone()).ok())
            .unwrap_or_default();
        for relation in related {
            let local: Option<i64> = conn
                .query_row(
                    "SELECT id FROM artists WHERE musicbrainz_id = ?1
                     UNION SELECT artist_id FROM artist_links
                           WHERE source = 'musicbrainz' AND status = 'matched' AND external_id = ?1
                     LIMIT 1",
                    [&relation.id],
                    |row| row.get(0),
                )
                .optional()?;
            if let Some(local) = local.filter(|&local| local != artist_id) {
                let kind = match relation.relation.as_str() {
                    "member of band" => Relation::Member,
                    "subgroup" => Relation::Subgroup,
                    _ => Relation::With,
                };
                linked.insert(local, kind);
            }
        }
        // Those whose relationships name them.
        if let Some(mbid) = mbid {
            let mut statement = conn.prepare_cached(
                "SELECT artist_id FROM artist_links
                 WHERE source = 'musicbrainz' AND status = 'matched' AND artist_id != ?1
                   AND instr(details, ?2) > 0",
            )?;
            for row in statement
                .query_map(params![artist_id, format!("\"id\":\"{mbid}\"")], |row| {
                    row.get::<_, i64>(0)
                })?
            {
                linked.entry(row?).or_insert(Relation::With);
            }
        }
    }
    // Sharing an album: a guest on their album, or theirs on another's.
    let mut statement = conn.prepare_cached(
        "SELECT DISTINCT t.artist_id FROM tracks t
         WHERE t.artist_id != ?1 AND t.album_id IN (
             SELECT album_id FROM tracks WHERE album_id IS NOT NULL
                 AND (artist_id = ?1 OR album_artist_id = ?1))
         UNION
         SELECT DISTINCT t.album_artist_id FROM tracks t
         WHERE t.album_artist_id != ?1 AND t.artist_id = ?1",
    )?;
    for row in statement.query_map([artist_id], |row| row.get::<_, i64>(0))? {
        linked.entry(row?).or_insert(Relation::With);
    }
    Ok(linked)
}

/// Takes the best of `scored` above `MIN_SCORE`, at most `per_group` from
/// each group `group` puts a key in (none: no limit), until `count`.
fn best(
    scored: Vec<Scored>,
    count: usize,
    per_group: usize,
    group: impl Fn(i64) -> Option<i64>,
) -> Vec<Scored> {
    let mut groups: HashMap<i64, usize> = HashMap::new();
    let mut chosen = Vec::new();
    for item in scored {
        if chosen.len() == count || item.score < MIN_SCORE {
            break;
        }
        if let Some(group) = group(item.key) {
            let taken = groups.entry(group).or_default();
            if *taken >= per_group {
                continue;
            }
            *taken += 1;
        }
        chosen.push(item);
    }
    chosen
}

/// Up to `count` tracks like `track_id` from other albums, at most one an
/// album and two an artist, none in the folders `unreadable`.
pub fn similar_tracks(
    conn: &Connection,
    track_id: i64,
    unreadable: &[i64],
    count: usize,
) -> Result<Vec<SimilarTrack>, Error> {
    let catalog = Catalog::load(conn)?;
    let Some(seed) = catalog.track(track_id) else {
        return Ok(Vec::new());
    };
    let exclude = same_album(&catalog, seed);
    let scored = scored(
        conn,
        &catalog,
        Seed::Track(track_id),
        Grain::Track,
        &exclude,
        unreadable,
    )?;
    // One an album first, then two an artist.
    let mut albums = HashSet::new();
    let scored: Vec<Scored> = scored
        .into_iter()
        .filter(|item| {
            catalog
                .track(item.key)
                .and_then(|track| track.album_id)
                .is_none_or(|album| albums.insert(album))
        })
        .collect();
    let chosen = best(scored, count, 2, |key| {
        catalog.track(key).and_then(|track| track.artist_id)
    });
    track_rows(conn, &catalog, chosen)
}

/// The seed track and the rest of its album.
pub(crate) fn same_album(catalog: &Catalog, seed: &TrackFacts) -> HashSet<i64> {
    let mut tracks: HashSet<i64> = match seed.album_id {
        Some(album) => catalog
            .tracks
            .iter()
            .filter(|track| track.album_id == Some(album))
            .map(|track| track.id)
            .collect(),
        None => HashSet::new(),
    };
    tracks.insert(seed.id);
    tracks
}

fn track_rows(
    conn: &Connection,
    catalog: &Catalog,
    chosen: Vec<Scored>,
) -> Result<Vec<SimilarTrack>, Error> {
    let ids: Vec<i64> = chosen.iter().map(|item| item.key).collect();
    let mut statement = conn.prepare_cached(
        "SELECT t.id, IFNULL(t.title, t.relative_path), IFNULL(t.artist_credit, ar.name),
                al.title, t.album_id, t.duration
         FROM tracks t LEFT JOIN artists ar ON ar.id = t.artist_id
         LEFT JOIN albums al ON al.id = t.album_id
         WHERE t.id IN (SELECT value FROM json_each(?1))",
    )?;
    let mut rows: HashMap<i64, SimilarTrack> = statement
        .query_map([json_ids(&ids)], |row| {
            let title: String = row.get(1)?;
            Ok(SimilarTrack {
                track_id: row.get(0)?,
                title: title.rsplit('/').next().unwrap_or(&title).to_owned(),
                artist: row.get(2)?,
                album: row.get(3)?,
                album_id: row.get(4)?,
                duration: row.get(5)?,
                reasons: Vec::new(),
            })
        })?
        .map(|row| row.map(|track| (track.track_id, track)))
        .collect::<Result<_, _>>()?;
    Ok(chosen
        .into_iter()
        .filter_map(|item| {
            let mut track = rows.remove(&item.key)?;
            track.reasons = catalog.reasons(&item.whys);
            Some(track)
        })
        .collect())
}

/// The album artist of each album, from its tracks.
fn album_artists(catalog: &Catalog) -> HashMap<i64, i64> {
    catalog
        .tracks
        .iter()
        .filter_map(|track| Some((track.album_id?, track.album_artist_id?)))
        .collect()
}

/// Up to `count` other albums like `album_id`, at most two by an album
/// artist, none only in the folders `unreadable`.
pub fn similar_albums(
    conn: &Connection,
    album_id: i64,
    unreadable: &[i64],
    count: usize,
) -> Result<Vec<SimilarAlbum>, Error> {
    let catalog = Catalog::load(conn)?;
    let exclude = HashSet::from([album_id]);
    let scored = scored(
        conn,
        &catalog,
        Seed::Album(album_id),
        Grain::Album,
        &exclude,
        unreadable,
    )?;
    let artists = album_artists(&catalog);
    let chosen = best(scored, count, 2, |key| artists.get(&key).copied());
    album_rows(conn, &catalog, chosen)
}

fn album_rows(
    conn: &Connection,
    catalog: &Catalog,
    chosen: Vec<Scored>,
) -> Result<Vec<SimilarAlbum>, Error> {
    let ids: Vec<i64> = chosen.iter().map(|item| item.key).collect();
    let mut cards: HashMap<i64, AlbumCard> = discover::cards(conn, &ids)?
        .into_iter()
        .map(|card| (card.id, card))
        .collect();
    Ok(chosen
        .into_iter()
        .filter_map(|item| {
            Some(SimilarAlbum {
                album: cards.remove(&item.key)?,
                reasons: catalog.reasons(&item.whys),
            })
        })
        .collect())
}

/// Up to `count` other artists like `artist_id`, none of them only in
/// the folders `unreadable`. Various Artists is never one.
pub fn similar_artists(
    conn: &Connection,
    artist_id: i64,
    unreadable: &[i64],
    count: usize,
) -> Result<Vec<SimilarArtist>, Error> {
    let catalog = Catalog::load(conn)?;
    let exclude = HashSet::from([artist_id]);
    let scored = scored(
        conn,
        &catalog,
        Seed::Artist(artist_id),
        Grain::Artist,
        &exclude,
        unreadable,
    )?;
    Ok(best(scored, count, usize::MAX, |_| None)
        .into_iter()
        .map(|item| SimilarArtist {
            artist_id: item.key,
            name: catalog.artist_name(item.key).to_owned(),
            reasons: catalog.reasons(&item.whys),
        })
        .collect())
}

/// Home's suggestions: up to `count` albums not played in the last 90
/// days (before `now`) that resemble what the user plays. The taste is
/// the tracks played in those 90 days, a play a point, and the
/// favourites (a track two points, an album or an artist three spread
/// over its tracks); with nothing played lately, the last 200 plays. The
/// albums it comes from and the favourite albums are left out, as are
/// albums only in the folders `unreadable`. `vary` (the day, say) raises
/// scores by up to 15% at random, so the row changes from day to day.
pub fn for_you(
    conn: &Connection,
    unreadable: &[i64],
    count: usize,
    now: i64,
    vary: Option<u64>,
) -> Result<Vec<SimilarAlbum>, Error> {
    let catalog = Catalog::load(conn)?;
    let mut weights: HashMap<i64, f64> = HashMap::new();
    let mut played: Vec<(i64, u32)> = conn
        .prepare_cached(
            "SELECT track_id, count(*) FROM plays WHERE played_at >= ?1 GROUP BY track_id",
        )?
        .query_map([now - LATELY], |row| Ok((row.get(0)?, row.get(1)?)))?
        .collect::<Result<_, _>>()?;
    if played.is_empty() {
        played = conn
            .prepare_cached(
                "SELECT track_id, count(*) FROM
                     (SELECT track_id FROM plays ORDER BY played_at DESC, id DESC LIMIT ?1)
                 GROUP BY track_id",
            )?
            .query_map([FALLBACK_PLAYS], |row| Ok((row.get(0)?, row.get(1)?)))?
            .collect::<Result<_, _>>()?;
    }
    let mut exclude: HashSet<i64> = HashSet::new();
    for &(track, plays) in &played {
        *weights.entry(track).or_default() += plays as f64;
        if let Some(album) = catalog.track(track).and_then(|track| track.album_id) {
            exclude.insert(album);
        }
    }
    let ids = |sql: &str| -> Result<Vec<i64>, Error> {
        Ok(conn
            .prepare_cached(sql)?
            .query_map([], |row| row.get(0))?
            .collect::<Result<_, _>>()?)
    };
    for track in ids("SELECT track_id FROM track_favourites")? {
        *weights.entry(track).or_default() += 2.0;
    }
    let mut spread = |tracks: Vec<i64>| {
        let share = 3.0 / tracks.len().max(1) as f64;
        for track in tracks {
            *weights.entry(track).or_default() += share;
        }
    };
    for album in ids("SELECT album_id FROM album_favourites")? {
        exclude.insert(album);
        spread(
            catalog
                .seed_tracks(Seed::Album(album))
                .iter()
                .map(|t| t.id)
                .collect(),
        );
    }
    for artist in ids("SELECT artist_id FROM artist_favourites")? {
        spread(
            catalog
                .seed_tracks(Seed::Artist(artist))
                .iter()
                .map(|t| t.id)
                .collect(),
        );
    }
    if weights.is_empty() {
        return Ok(Vec::new());
    }

    let profile = catalog.profile(
        weights
            .iter()
            .filter_map(|(&track, &weight)| Some((catalog.track(track)?, weight))),
    );
    let index = catalog.seed_index(conn, &profile)?;
    let mut scored = candidates(
        &catalog,
        &index,
        Grain::Album,
        &exclude,
        unreadable,
        &HashMap::new(),
    );
    if let Some(vary) = vary {
        for item in &mut scored {
            item.score *=
                1.0 + 0.15 * unit(item.key as u64 ^ vary.wrapping_mul(0x9e37_79b9_7f4a_7c15));
        }
        scored.sort_by(|a, b| b.score.total_cmp(&a.score).then(a.key.cmp(&b.key)));
    }
    let artists = album_artists(&catalog);
    let chosen = best(scored, count, 2, |key| artists.get(&key).copied());
    album_rows(conn, &catalog, chosen)
}

/// A number in [0, 1) that `x` always gives (splitmix64).
fn unit(x: u64) -> f64 {
    let mut z = x.wrapping_add(0x9e37_79b9_7f4a_7c15);
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    (((z ^ (z >> 31)) >> 11) as f64) / (1u64 << 53) as f64
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::library::test_library::{track, Library};

    fn id(library: &Library, sql: &str, value: &str) -> i64 {
        library
            .conn
            .query_row(sql, [value], |row| row.get(0))
            .unwrap()
    }

    fn album(library: &Library, title: &str) -> i64 {
        id(library, "SELECT id FROM albums WHERE title = ?1", title)
    }

    fn artist(library: &Library, name: &str) -> i64 {
        id(library, "SELECT id FROM artists WHERE name = ?1", name)
    }

    fn track_id(library: &Library, path: &str) -> i64 {
        id(
            library,
            "SELECT id FROM tracks WHERE relative_path = ?1",
            path,
        )
    }

    fn titles(albums: &[SimilarAlbum]) -> Vec<&str> {
        albums.iter().map(|a| a.album.title.as_str()).collect()
    }

    fn play(library: &Library, path: &str, at: i64) {
        crate::history::record(&library.conn, track_id(library, path), at, 60.0).unwrap();
    }

    /// Ten tracks an album, by `artist`, in `genre`, from `year`.
    fn album_tracks(
        title: &str,
        artist: &str,
        genre: &str,
        year: u32,
    ) -> Vec<crate::library::test_library::Track> {
        (1..=10)
            .map(|n| {
                track(&format!("{title}/{n:02}.flac"))
                    .album(title)
                    .artist(artist)
                    .genre(genre)
                    .year(year)
                    .number(n)
            })
            .collect()
    }

    /// A small library with clear neighbours: two jazz albums of the
    /// nineties, a jazz album of the sixties, a rock album, and a jazz
    /// album by the seed's artist.
    fn fixture() -> Library {
        let mut tracks = Vec::new();
        tracks.extend(album_tracks("Seed", "Quartet", "Jazz", 1994));
        tracks.extend(album_tracks("Near", "Trio", "Jazz; Soul", 1995));
        tracks.extend(album_tracks("Old", "Elder", "Jazz", 1962));
        tracks.extend(album_tracks("Loud", "Band", "Rock", 1994));
        tracks.extend(album_tracks("Later", "Quartet", "Jazz", 2010));
        tracks.extend(album_tracks("Far", "Other", "Metal", 1980));
        Library::new(tracks)
    }

    fn profile(genres: &[(u32, f64)], artists: &[i64], year: Option<u32>) -> Profile {
        Profile {
            genres: genres.to_vec(),
            artists: artists.to_vec(),
            year,
            ..Profile::default()
        }
    }

    #[test]
    fn scores_what_a_candidate_shares() {
        let seed = SeedIndex {
            genres: HashMap::from([(0, 1.0), (1, 0.5)]),
            artists: HashSet::from([10]),
            labels: HashSet::from([7]),
            year: Some(1994),
            linked: HashMap::from([(11, Relation::Member)]),
            ..SeedIndex::default()
        };
        // One genre all through, and the same year.
        let (points, whys) = score(
            &seed,
            &profile(&[(0, 1.0)], &[], Some(1995)),
            0,
            Grain::Album,
        );
        assert_eq!(points, 5.0);
        assert_eq!(whys, [(3.0, Why::Genre(0)), (2.0, Why::Era(1995))]);
        // A second genre adds up to a point; one partly shared, less.
        assert_eq!(
            score(
                &seed,
                &profile(&[(0, 1.0), (1, 1.0)], &[], None),
                0,
                Grain::Album
            )
            .0,
            3.5
        );
        assert_eq!(
            score(&seed, &profile(&[(0, 0.5)], &[], None), 0, Grain::Album).0,
            1.5
        );
        // Eras drift apart.
        assert_eq!(
            score(&seed, &profile(&[], &[], Some(1999)), 0, Grain::Album).0,
            1.0
        );
        assert_eq!(
            score(&seed, &profile(&[], &[], Some(2000)), 0, Grain::Album).0,
            0.0
        );
        // A linked artist, the seed's own (worth less between tracks, nothing
        // between artists), a label, sessions together.
        let linked = score(&seed, &profile(&[], &[11], None), 0, Grain::Album);
        assert_eq!(linked.1, [(4.0, Why::Linked(11, Relation::Member))]);
        assert_eq!(
            score(&seed, &profile(&[], &[10], None), 0, Grain::Album).0,
            2.0
        );
        assert_eq!(
            score(&seed, &profile(&[], &[10], None), 0, Grain::Track).0,
            1.0
        );
        assert_eq!(
            score(&seed, &profile(&[], &[10], None), 0, Grain::Artist).0,
            0.0
        );
        let label = Profile {
            labels: vec![3, 7],
            ..Profile::default()
        };
        assert_eq!(
            score(&seed, &label, 0, Grain::Album).1,
            [(3.0, Why::Label(7))]
        );
        assert_eq!(score(&seed, &Profile::default(), 1, Grain::Album).0, 2.0);
        assert_eq!(score(&seed, &Profile::default(), 3, Grain::Album).0, 4.0);
        assert_eq!(score(&seed, &Profile::default(), 50, Grain::Album).0, 4.0);
    }

    #[test]
    fn loudness_only_tips_the_balance() {
        let seed = SeedIndex {
            genres: HashMap::from([(0, 1.0)]),
            loudness: Some(-9.0),
            ..SeedIndex::default()
        };
        let quiet = Profile {
            loudness: Some(-9.5),
            ..Profile::default()
        };
        assert_eq!(
            score(&seed, &quiet, 0, Grain::Album).0,
            0.0,
            "alone, nothing"
        );
        let same_genre = Profile {
            genres: vec![(0, 1.0)],
            ..quiet.clone()
        };
        assert_eq!(score(&seed, &same_genre, 0, Grain::Album).0, 4.0);
        let louder = Profile {
            loudness: Some(-6.0),
            ..same_genre
        };
        assert_eq!(score(&seed, &louder, 0, Grain::Album).0, 3.0);
    }

    #[test]
    fn a_profile_weighs_its_tracks() {
        let library = Library::new([
            track("1.flac")
                .album("A")
                .artist("X")
                .genre("Jazz")
                .year(1990),
            track("2.flac")
                .album("A")
                .artist("X; Guest")
                .genre("Jazz; Soul")
                .year(1992),
            track("3.flac")
                .album("B")
                .artist("Y")
                .genre("jazz")
                .year(2020),
        ]);
        let catalog = Catalog::load(&library.conn).unwrap();
        let tracks: Vec<&TrackFacts> = catalog.seed_tracks(Seed::Album(album(&library, "A")));
        assert_eq!(tracks.len(), 2);
        // The album's earliest year stands for every track on it.
        assert!(tracks.iter().all(|t| t.year == Some(1990)));
        let profile = catalog.profile(tracks.iter().map(|&t| (t, 1.0)));
        let named: Vec<(&str, f64)> = profile
            .genres
            .iter()
            .map(|&(genre, share)| (catalog.genre_name(genre), share))
            .collect();
        assert_eq!(named, [("Jazz", 1.0), ("Soul", 0.5)]);
        assert_eq!(profile.artists[0], artist(&library, "X"));
        assert!(profile.artists.contains(&artist(&library, "Guest")));
        assert_eq!(profile.year, Some(1990));
        // "jazz" is the same genre, however it's written.
        let both = catalog.profile(catalog.tracks.iter().map(|t| (t, 1.0)));
        assert_eq!(both.genres[0].1, 1.0);
        // Weights: a track weighing three times as much.
        let weighted = catalog.profile(catalog.tracks.iter().map(|t| {
            (
                t,
                if t.album_id == Some(album(&library, "B")) {
                    3.0
                } else {
                    1.0
                },
            )
        }));
        assert_eq!(weighted.artists[0], artist(&library, "Y"));
    }

    #[test]
    fn an_era_needs_its_years_close_together() {
        assert_eq!(era(vec![(1990, 1.0), (1994, 1.0), (1996, 1.0)]), Some(1994));
        assert_eq!(era(vec![(1960, 1.0), (1990, 1.0), (2020, 1.0)]), None);
        // The weights pick the middle.
        assert_eq!(era(vec![(1960, 1.0), (2000, 8.0), (2001, 1.0)]), Some(2000));
        assert_eq!(era(Vec::new()), None);
    }

    #[test]
    fn finds_albums_like_an_album() {
        let library = fixture();
        let similar = similar_albums(&library.conn, album(&library, "Seed"), &[], 10).unwrap();
        // The nineties jazz album first; then the seed's artist's other
        // album and the sixties one, which share the genre; the rock album
        // shares only the year, the metal one nothing.
        assert_eq!(titles(&similar), ["Near", "Later", "Old"]);
        assert_eq!(
            similar[0].reasons,
            [
                Reason::Genre {
                    name: "Jazz".into()
                },
                Reason::Era { year: 1995 }
            ]
        );
        assert_eq!(
            similar[1].reasons,
            [
                Reason::Genre {
                    name: "Jazz".into()
                },
                Reason::Artist {
                    name: "Quartet".into()
                }
            ]
        );
        assert_eq!(
            similar_albums(&library.conn, album(&library, "Seed"), &[], 1)
                .unwrap()
                .len(),
            1
        );
        assert!(similar_albums(&library.conn, 999, &[], 10)
            .unwrap()
            .is_empty());
    }

    #[test]
    fn listening_sessions_bring_albums_together() {
        let library = fixture();
        // The metal album shares nothing with the seed, but was played
        // straight after it three times; once, hours later, doesn't count.
        for day in 0..3 {
            let start = 1_000_000 + day * 86400;
            play(&library, "Seed/01.flac", start);
            play(&library, "Far/01.flac", start + 600);
        }
        play(&library, "Seed/01.flac", 2_000_000);
        play(&library, "Old/01.flac", 2_000_000 + 4 * 3600);
        let similar = similar_albums(&library.conn, album(&library, "Seed"), &[], 10).unwrap();
        let far = similar
            .iter()
            .find(|a| a.album.title == "Far")
            .expect("played together");
        assert_eq!(far.reasons, [Reason::Together { times: 3 }]);
        let old = similar.iter().find(|a| a.album.title == "Old").unwrap();
        assert!(!old
            .reasons
            .iter()
            .any(|r| matches!(r, Reason::Together { .. })));
    }

    #[test]
    fn finds_tracks_on_other_albums() {
        let library = fixture();
        let seed = track_id(&library, "Seed/01.flac");
        let similar = similar_tracks(&library.conn, seed, &[], 10).unwrap();
        // One an album, none from the seed's.
        let albums: Vec<&str> = similar.iter().filter_map(|t| t.album.as_deref()).collect();
        assert_eq!(albums, ["Near", "Later", "Old"]);
        assert_eq!(similar[0].title, "01.flac");
        assert_eq!(similar[0].artist.as_deref(), Some("Trio"));
        assert!(similar_tracks(&library.conn, 999, &[], 10)
            .unwrap()
            .is_empty());
    }

    #[test]
    fn finds_artists_by_their_links() {
        let library = fixture();
        library
            .conn
            .execute_batch(
                "UPDATE artists SET musicbrainz_id = 'elder-mbid' WHERE name = 'Elder';
                 INSERT INTO artist_links (artist_id, source, status, external_id, score, chosen_by,
                                           details, checked_at)
                 SELECT id, 'musicbrainz', 'matched', 'quartet-mbid', 1, 'auto',
                        '{\"related\": [{\"id\": \"elder-mbid\", \"name\": \"Elder\",
                                         \"relation\": \"member of band\"}]}', 0
                 FROM artists WHERE name = 'Quartet';",
            )
            .unwrap();
        let similar = similar_artists(&library.conn, artist(&library, "Quartet"), &[], 10).unwrap();
        let names: Vec<&str> = similar.iter().map(|a| a.name.as_str()).collect();
        // Elder: linked and jazz; Trio: jazz, but Quartet's years spread too
        // far for an era.
        assert_eq!(names, ["Elder", "Trio"]);
        assert_eq!(
            similar[0].reasons[0],
            Reason::Linked {
                name: "Elder".into(),
                relation: Relation::Member
            }
        );
    }

    #[test]
    fn various_artists_is_never_an_artist_like_another() {
        let library = Library::new([
            track("a.flac")
                .album("Mix")
                .artist("One")
                .album_artist(VARIOUS_ARTISTS)
                .genre("Jazz"),
            track("b.flac")
                .album("Mix")
                .artist("Two")
                .album_artist(VARIOUS_ARTISTS)
                .genre("Jazz"),
        ]);
        let similar = similar_artists(&library.conn, artist(&library, "One"), &[], 10).unwrap();
        let names: Vec<&str> = similar.iter().map(|a| a.name.as_str()).collect();
        assert_eq!(names, ["Two"]);
    }

    /// X7's library half: artists with too little in common, or only in
    /// unreadable folders, are left out.
    #[test]
    fn artists_need_enough_in_common_and_a_readable_folder() {
        let library = fixture();
        let away = library.add_folder("/Volumes/Away");
        for t in album_tracks("Away", "Septet", "Jazz", 1994) {
            library.add(away, t);
        }
        let seed = artist(&library, "Quartet");
        let names_with = |unreadable: &[i64]| -> Vec<String> {
            similar_artists(&library.conn, seed, unreadable, 10)
                .unwrap()
                .into_iter()
                .map(|a| a.name)
                .collect()
        };
        let names = names_with(&[]);
        // Jazz is enough; Band's year alone and Other's metal aren't.
        for name in ["Trio", "Elder", "Septet"] {
            assert!(names.contains(&name.to_owned()), "{name} in {names:?}");
        }
        for name in ["Band", "Other", "Quartet"] {
            assert!(!names.contains(&name.to_owned()), "{name} in {names:?}");
        }
        assert!(!names_with(&[away]).contains(&"Septet".to_owned()));
    }

    #[test]
    fn suggests_albums_not_played_lately() {
        let now = 100 * 86400;
        let library = fixture();
        assert!(
            for_you(&library.conn, &[], 10, now, None)
                .unwrap()
                .is_empty(),
            "no taste yet"
        );

        // Jazz played this month: other jazz albums are suggested, the
        // played one isn't; the seed's artist's other album comes with the
        // artist.
        for n in 1..=5 {
            play(
                &library,
                &format!("Seed/{n:02}.flac"),
                now - 86400 + n * 300,
            );
        }
        let suggested = for_you(&library.conn, &[], 10, now, None).unwrap();
        assert_eq!(titles(&suggested), ["Near", "Later", "Old"]);

        // An album played lately isn't suggested; a favourite album isn't
        // either, but its rock now dilutes the jazz, so the sixties album
        // no longer has enough in common.
        play(&library, "Near/01.flac", now - 3600);
        library
            .conn
            .execute(
                "INSERT INTO album_favourites (album_id, added_at) VALUES (?1, 0)",
                [album(&library, "Loud")],
            )
            .unwrap();
        let suggested = for_you(&library.conn, &[], 10, now, None).unwrap();
        assert_eq!(titles(&suggested), ["Later"]);

        // Nothing played lately: the last plays stand in, and their albums
        // are still left out.
        let later = now + 365 * 86400;
        let suggested = for_you(&library.conn, &[], 10, later, None).unwrap();
        assert!(!titles(&suggested).contains(&"Seed"));
        assert!(!titles(&suggested).contains(&"Near"));
        assert!(titles(&suggested).contains(&"Later"));
    }

    #[test]
    fn leaves_out_unreadable_folders() {
        let library = fixture();
        let away = library.add_folder("/Volumes/Away");
        for t in album_tracks("Away", "Trio", "Jazz", 1994) {
            library.add(away, t);
        }
        let seed = album(&library, "Seed");
        let titles_with = |unreadable: &[i64]| -> Vec<String> {
            similar_albums(&library.conn, seed, unreadable, 10)
                .unwrap()
                .into_iter()
                .map(|a| a.album.title)
                .collect()
        };
        assert!(titles_with(&[]).contains(&"Away".to_owned()));
        assert!(!titles_with(&[away]).contains(&"Away".to_owned()));
        // The seed can be unreadable itself, and still be compared.
        let away_album = album(&library, "Away");
        let from_away = similar_albums(&library.conn, away_album, &[away], 10).unwrap();
        assert!(titles(&from_away).contains(&"Seed"));
    }

    #[test]
    fn caps_albums_by_one_artist() {
        let mut tracks = album_tracks("Seed", "Someone", "Jazz", 1994);
        for n in 0..4 {
            tracks.extend(album_tracks(
                &format!("Prolific {n}"),
                "Prolific",
                "Jazz",
                1994,
            ));
        }
        tracks.extend(album_tracks("Other", "Other", "Jazz", 1994));
        let library = Library::new(tracks);
        let similar = similar_albums(&library.conn, album(&library, "Seed"), &[], 10).unwrap();
        let by_prolific = similar
            .iter()
            .filter(|a| a.album.artist.as_deref() == Some("Prolific"))
            .count();
        assert_eq!(by_prolific, 2);
        assert!(titles(&similar).contains(&"Other"));
    }
}
