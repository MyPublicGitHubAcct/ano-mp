//! Library radio (PLAN.md O9): tracks from the library that fit a seed
//! track, to keep playing when the queue runs out or on "Start radio from
//! this". Everything is local; no service is called. A track scores for
//! sharing a genre with the seed, a year near it, its label, and an artist
//! linked to the seed's (band members, collaborations and subgroups on
//! MusicBrainz, or sharing an album in the library). Tracks played in the
//! last week score less, and the seed's own album is left out. Each pick
//! says why it was chosen.

use std::collections::{HashMap, HashSet};

use rusqlite::{params, Connection, OptionalExtension};

use crate::library::genres;
use crate::library::sort_key::fold;
use crate::library::{unix_now, Error};

/// A track radio picked, and why.
#[derive(Debug, Clone, PartialEq)]
pub struct Pick {
    pub track_id: i64,
    pub reason: String,
}

/// How many tracks radio adds at a time.
pub const BATCH: usize = 8;

/// Tracks played this recently count for less.
const RECENT: i64 = 7 * 86400;

struct TrackRow {
    id: i64,
    artist_id: Option<i64>,
    album_id: Option<i64>,
    genres: Vec<String>,
    year: Option<u32>,
}

/// What a candidate is scored against.
struct Seed {
    track_id: i64,
    artist_id: Option<i64>,
    album_id: Option<i64>,
    /// Folded genre names, and how they're written.
    genres: HashMap<String, String>,
    year: Option<u32>,
    labels: HashSet<String>,
    /// Linked artists and how to say so ("with The Band").
    linked: HashMap<i64, String>,
    artist_name: Option<String>,
}

/// Up to `count` tracks to follow `seed_track_id`, none of them in
/// `exclude`, at most one per album and two per artist, the best first.
/// `random` varies the picks between calls.
pub fn picks(
    conn: &Connection,
    seed_track_id: i64,
    exclude: &HashSet<i64>,
    count: usize,
    random: u64,
) -> Result<Vec<Pick>, Error> {
    let Some(seed) = seed(conn, seed_track_id)? else {
        return Ok(Vec::new());
    };
    let labels = album_labels(conn)?;
    let recent = recently_played(conn)?;
    let mut rng = random | 1;
    let mut next_random = move || {
        rng ^= rng >> 12;
        rng ^= rng << 25;
        rng ^= rng >> 27;
        (rng.wrapping_mul(0x2545_f491_4f6c_dd1d) >> 11) as f64 / (1u64 << 53) as f64
    };

    let mut scored: Vec<(f64, Pick, Option<i64>, Option<i64>)> = Vec::new();
    for track in tracks(conn)? {
        // Radio moves away from the seed's album; the queue has the rest of it.
        if track.id == seed.track_id
            || exclude.contains(&track.id)
            || (track.album_id.is_some() && track.album_id == seed.album_id)
        {
            continue;
        }
        let mut score = 0.0;
        let mut reasons: Vec<(f64, String)> = Vec::new();

        let shared: Vec<&String> = track
            .genres
            .iter()
            .filter_map(|genre| seed.genres.get(genre))
            .collect();
        if let Some(first) = shared.first() {
            let points = 3.0 + (shared.len() as f64 - 1.0).min(1.0);
            score += points;
            reasons.push((points, (*first).clone()));
        }
        if let (Some(a), Some(b)) = (seed.year, track.year) {
            let points = match a.abs_diff(b) {
                0..=2 => 2.0,
                3..=5 => 1.0,
                _ => 0.0,
            };
            if points > 0.0 {
                score += points;
                reasons.push((points, b.to_string()));
            }
        }
        if let Some(album_labels) = track.album_id.and_then(|album| labels.get(&album)) {
            if album_labels.iter().any(|label| seed.labels.contains(label)) {
                score += 3.0;
                reasons.push((3.0, "same label".into()));
            }
        }
        if let Some(link) = track.artist_id.and_then(|artist| seed.linked.get(&artist)) {
            score += 4.0;
            reasons.push((4.0, link.clone()));
        } else if track.artist_id.is_some() && track.artist_id == seed.artist_id {
            score += 1.0;
            if let Some(name) = &seed.artist_name {
                reasons.push((1.0, format!("more by {name}")));
            }
        }
        if score <= 0.0 {
            continue;
        }
        if recent.contains(&track.id) {
            score *= 0.3;
        }
        // Some chance, so the same seed doesn't always give the same picks.
        let score = score * (0.6 + 0.4 * next_random());
        reasons.sort_by(|a, b| b.0.total_cmp(&a.0));
        let reason = reasons
            .into_iter()
            .take(2)
            .map(|(_, reason)| reason)
            .collect::<Vec<_>>()
            .join(", ");
        scored.push((
            score,
            Pick {
                track_id: track.id,
                reason,
            },
            track.album_id,
            track.artist_id,
        ));
    }
    scored.sort_by(|a, b| b.0.total_cmp(&a.0));

    let mut albums = HashSet::new();
    let mut artists: HashMap<i64, usize> = HashMap::new();
    let mut chosen = Vec::new();
    for (_, pick, album, artist) in scored {
        if chosen.len() == count {
            break;
        }
        if album.is_some_and(|album| !albums.insert(album)) {
            continue;
        }
        if let Some(artist) = artist {
            let n = artists.entry(artist).or_default();
            if *n >= 2 {
                continue;
            }
            *n += 1;
        }
        chosen.push(pick);
    }
    Ok(chosen)
}

fn tracks(conn: &Connection) -> Result<Vec<TrackRow>, Error> {
    let mut statement = conn.prepare_cached(
        "SELECT t.id, t.artist_id, t.album_id, t.genre,
                IFNULL((SELECT min(y.year) FROM tracks y WHERE y.album_id = t.album_id), t.year)
         FROM tracks t",
    )?;
    let rows = statement.query_map([], |row| {
        let genre: Option<String> = row.get(3)?;
        Ok(TrackRow {
            id: row.get(0)?,
            artist_id: row.get(1)?,
            album_id: row.get(2)?,
            genres: genres::split(genre.as_deref().unwrap_or(""))
                .into_iter()
                .map(fold)
                .collect(),
            year: row.get(4)?,
        })
    })?;
    Ok(rows.collect::<Result<_, _>>()?)
}

fn seed(conn: &Connection, track_id: i64) -> Result<Option<Seed>, Error> {
    let row: Option<(Option<i64>, Option<i64>, Option<String>, Option<u32>, Option<String>)> = conn
        .query_row(
            "SELECT t.artist_id, t.album_id, t.genre,
                    IFNULL((SELECT min(y.year) FROM tracks y WHERE y.album_id = t.album_id), t.year),
                    ar.name
             FROM tracks t LEFT JOIN artists ar ON ar.id = t.artist_id WHERE t.id = ?1",
            [track_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?)),
        )
        .optional()?;
    let Some((artist_id, album_id, genre, year, artist_name)) = row else {
        return Ok(None);
    };
    let genres = genres::split(genre.as_deref().unwrap_or(""))
        .into_iter()
        .map(|genre| (fold(genre), genre.to_owned()))
        .collect();
    let labels = album_id
        .and_then(|album| album_labels(conn).ok()?.remove(&album))
        .unwrap_or_default();
    let linked = match artist_id {
        Some(artist) => linked_artists(conn, artist)?,
        None => HashMap::new(),
    };
    Ok(Some(Seed {
        track_id,
        artist_id,
        album_id,
        genres,
        year,
        labels,
        linked,
        artist_name,
    }))
}

/// Each matched album's labels (folded), from its MusicBrainz release.
fn album_labels(conn: &Connection) -> Result<HashMap<i64, HashSet<String>>, Error> {
    let mut statement = conn.prepare_cached(
        "SELECT l.album_id, json_extract(label.value, '$.name')
         FROM album_links l, json_each(l.details, '$.labels') AS label
         WHERE l.source = 'musicbrainz' AND l.status = 'matched' AND l.details IS NOT NULL",
    )?;
    let mut labels: HashMap<i64, HashSet<String>> = HashMap::new();
    for row in statement.query_map([], |row| {
        Ok((row.get::<_, i64>(0)?, row.get::<_, Option<String>>(1)?))
    })? {
        let (album, name) = row?;
        if let Some(name) = name.filter(|name| !name.is_empty() && name != "[no label]") {
            labels.entry(album).or_default().insert(fold(&name));
        }
    }
    Ok(labels)
}

/// Artists linked to `artist_id`: MusicBrainz relationships either way,
/// and artists sharing an album with them in the library.
fn linked_artists(conn: &Connection, artist_id: i64) -> Result<HashMap<i64, String>, Error> {
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
                linked.insert(local, describe(&relation.relation, &relation.name));
            }
        }
        // Those whose relationships name them.
        if let Some(mbid) = mbid {
            let mut statement = conn.prepare_cached(
                "SELECT l.artist_id, a.name FROM artist_links l JOIN artists a ON a.id = l.artist_id
                 WHERE l.source = 'musicbrainz' AND l.status = 'matched' AND l.artist_id != ?1
                   AND instr(l.details, ?2) > 0",
            )?;
            for row in statement
                .query_map(params![artist_id, format!("\"id\":\"{mbid}\"")], |row| {
                    Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
                })?
            {
                let (other, name) = row?;
                linked
                    .entry(other)
                    .or_insert_with(|| format!("with {name}"));
            }
        }
    }
    // Sharing an album: a guest on their album, or theirs on another's.
    let mut statement = conn.prepare_cached(
        "SELECT DISTINCT t.artist_id, a.name FROM tracks t JOIN artists a ON a.id = t.artist_id
         WHERE t.artist_id != ?1 AND t.album_id IN (
             SELECT album_id FROM tracks WHERE album_id IS NOT NULL
                 AND (artist_id = ?1 OR album_artist_id = ?1))
         UNION
         SELECT DISTINCT t.album_artist_id, a.name FROM tracks t JOIN artists a ON a.id = t.album_artist_id
         WHERE t.album_artist_id != ?1 AND t.artist_id = ?1",
    )?;
    for row in statement.query_map([artist_id], |row| {
        Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
    })? {
        let (other, name) = row?;
        linked
            .entry(other)
            .or_insert_with(|| format!("with {name}"));
    }
    Ok(linked)
}

fn describe(relation: &str, name: &str) -> String {
    match relation {
        "member of band" => format!("{name} member"),
        "subgroup" => format!("related to {name}"),
        _ => format!("with {name}"),
    }
}

fn recently_played(conn: &Connection) -> Result<HashSet<i64>, Error> {
    let mut statement =
        conn.prepare_cached("SELECT DISTINCT track_id FROM plays WHERE played_at >= ?1")?;
    let ids = statement
        .query_map([unix_now() - RECENT], |row| row.get(0))?
        .collect::<Result<_, _>>()?;
    Ok(ids)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::library::test_library::{track, Library};

    fn id(library: &Library, path: &str) -> i64 {
        library
            .conn
            .query_row(
                "SELECT id FROM tracks WHERE relative_path = ?1",
                [path],
                |row| row.get(0),
            )
            .unwrap()
    }

    #[test]
    fn picks_tracks_like_the_seed_and_says_why() {
        let library = Library::new([
            track("seed.flac")
                .artist("Seed")
                .album("S")
                .genre("Jazz")
                .year(1994),
            track("jazz94.flac")
                .artist("J")
                .album("J")
                .genre("jazz; Soul")
                .year(1995),
            track("rock94.flac")
                .artist("R")
                .album("R")
                .genre("Rock")
                .year(1994),
            track("metal.flac")
                .artist("M")
                .album("M")
                .genre("Metal")
                .year(1970),
            track("guest.flac")
                .artist("Guest")
                .album_artist("Seed")
                .album("S")
                .genre("Pop")
                .year(1994),
            track("guestalbum.flac")
                .artist("Guest")
                .album("G")
                .genre("Pop")
                .year(1980),
        ]);
        let seed = id(&library, "seed.flac");
        let picks = picks(&library.conn, seed, &HashSet::new(), 10, 1).unwrap();
        let by_id: HashMap<i64, &str> = picks
            .iter()
            .map(|p| (p.track_id, p.reason.as_str()))
            .collect();
        assert!(!by_id.contains_key(&seed));
        assert!(
            !by_id.contains_key(&id(&library, "metal.flac")),
            "nothing in common"
        );
        assert_eq!(by_id[&id(&library, "jazz94.flac")], "Jazz, 1995");
        assert_eq!(by_id[&id(&library, "rock94.flac")], "1994");
        // A guest on the seed's album is linked, on their own album too.
        assert!(by_id[&id(&library, "guestalbum.flac")].starts_with("with Guest"));

        // Excluded tracks and the count.
        let excluded: HashSet<i64> = [id(&library, "jazz94.flac")].into();
        let fewer = super::picks(&library.conn, seed, &excluded, 1, 1).unwrap();
        assert_eq!(fewer.len(), 1);
        assert_ne!(fewer[0].track_id, id(&library, "jazz94.flac"));
        assert!(super::picks(&library.conn, 999, &HashSet::new(), 5, 1)
            .unwrap()
            .is_empty());
    }

    #[test]
    fn follows_labels_and_musicbrainz_relationships() {
        let library = Library::new([
            track("seed.flac").artist("Singer").album("Solo"),
            track("band.flac").artist("The Band").album("Band Album"),
            track("label.flac").artist("Other").album("Same Label"),
        ]);
        library
            .conn
            .execute_batch(
                "UPDATE artists SET musicbrainz_id = 'band-mbid' WHERE name = 'The Band';
                 INSERT INTO artist_links (artist_id, source, status, external_id, score, chosen_by,
                                           details, checked_at)
                 SELECT id, 'musicbrainz', 'matched', 'singer-mbid', 1, 'auto',
                        '{\"related\": [{\"id\": \"band-mbid\", \"name\": \"The Band\",
                                         \"relation\": \"member of band\"}]}', 0
                 FROM artists WHERE name = 'Singer';
                 INSERT INTO album_links (album_id, source, status, external_id, score, chosen_by,
                                          details, checked_at)
                 SELECT id, 'musicbrainz', 'matched', 'x', 1, 'auto',
                        '{\"labels\": [{\"name\": \"Blue Note\", \"catalogNumber\": null}]}', 0
                 FROM albums WHERE title IN ('Solo', 'Same Label');",
            )
            .unwrap();
        let seed = id(&library, "seed.flac");
        let picks = picks(&library.conn, seed, &HashSet::new(), 10, 3).unwrap();
        let by_id: HashMap<i64, &str> = picks
            .iter()
            .map(|p| (p.track_id, p.reason.as_str()))
            .collect();
        assert_eq!(by_id[&id(&library, "band.flac")], "The Band member");
        assert_eq!(by_id[&id(&library, "label.flac")], "same label");
    }

    #[test]
    fn recently_played_tracks_come_later() {
        let library = Library::new([
            track("seed.flac").album("S").genre("Jazz"),
            track("a.flac").album("A").genre("Jazz"),
            track("b.flac").album("B").genre("Jazz"),
        ]);
        crate::history::record(&library.conn, id(&library, "a.flac"), unix_now(), 60.0).unwrap();
        let mut first_a = 0;
        for random in 0..50 {
            let picks = picks(
                &library.conn,
                id(&library, "seed.flac"),
                &HashSet::new(),
                2,
                random,
            )
            .unwrap();
            if picks[0].track_id == id(&library, "a.flac") {
                first_a += 1;
            }
        }
        assert_eq!(first_a, 0, "0.3 of the score never beats the random 0.6..1");
    }
}
