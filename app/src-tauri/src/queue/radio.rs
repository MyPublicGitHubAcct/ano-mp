//! Library radio (PLAN.md O9): tracks from the library that fit a seed
//! track, to keep playing when the queue runs out or on "Start radio from
//! this". Everything is local; no service is called. Tracks are scored as
//! the library's recommendations score them (`library::similar`, X4): a
//! shared genre, a year near the seed's, its label, an artist linked to the
//! seed's (band members, collaborations and subgroups on MusicBrainz, or
//! sharing an album in the library), a shared composer, and listening
//! sessions together. Tracks played in the last week score less, and the
//! seed's own album is left out. Each pick says why it was chosen.

use std::collections::{HashMap, HashSet};

use rusqlite::Connection;

use crate::library::similar::{self, Catalog, Grain, Relation, Seed, Why};
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

/// Up to `count` tracks to follow `seed_track_id`, none of them in
/// `exclude` or in the folders `unreadable` (PLAN.md H22b), at most one per
/// album and two per artist, the best first. `random` varies the picks
/// between calls.
pub fn picks(
    conn: &Connection,
    seed_track_id: i64,
    exclude: &HashSet<i64>,
    unreadable: &[i64],
    count: usize,
    random: u64,
) -> Result<Vec<Pick>, Error> {
    let catalog = Catalog::load(conn)?;
    let Some(seed) = catalog.track(seed_track_id) else {
        return Ok(Vec::new());
    };
    // Radio moves away from the seed's album; the queue has the rest of it.
    let mut skip = similar::same_album(&catalog, seed);
    skip.extend(exclude);
    let scored = similar::scored(
        conn,
        &catalog,
        Seed::Track(seed_track_id),
        Grain::Track,
        &skip,
        unreadable,
    )?;
    let recent = recently_played(conn)?;
    let mut rng = random | 1;
    let mut next_random = move || {
        rng ^= rng >> 12;
        rng ^= rng << 25;
        rng ^= rng >> 27;
        (rng.wrapping_mul(0x2545_f491_4f6c_dd1d) >> 11) as f64 / (1u64 << 53) as f64
    };

    let mut ranked: Vec<(f64, Pick, Option<i64>, Option<i64>)> = scored
        .into_iter()
        .filter_map(|item| {
            let track = catalog.track(item.key)?;
            let mut score = item.score;
            if recent.contains(&item.key) {
                score *= 0.3;
            }
            // Some chance, so the same seed doesn't always give the same picks.
            let score = score * (0.6 + 0.4 * next_random());
            let reason = item
                .whys
                .iter()
                .filter(|(points, _)| *points >= 1.0)
                .take(2)
                .map(|&(_, why)| describe(&catalog, why))
                .collect::<Vec<_>>()
                .join(", ");
            Some((
                score,
                Pick {
                    track_id: item.key,
                    reason,
                },
                track.album_id,
                track.artist_id,
            ))
        })
        .collect();
    ranked.sort_by(|a, b| b.0.total_cmp(&a.0));

    let mut albums = HashSet::new();
    let mut artists: HashMap<i64, usize> = HashMap::new();
    let mut chosen = Vec::new();
    for (_, pick, album, artist) in ranked {
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

/// The queue's few words for a reason.
fn describe(catalog: &Catalog, why: Why) -> String {
    match why {
        Why::Genre(genre) => catalog.genre_name(genre).to_owned(),
        Why::Era(year) => year.to_string(),
        Why::Label(_) => "same label".into(),
        Why::Linked(artist, relation) => {
            let name = catalog.artist_name(artist);
            match relation {
                Relation::Member => format!("{name} member"),
                Relation::Subgroup => format!("related to {name}"),
                Relation::With => format!("with {name}"),
            }
        }
        Why::Artist(artist) => format!("more by {}", catalog.artist_name(artist)),
        Why::Composer(artist) => format!("composed by {}", catalog.artist_name(artist)),
        Why::Together(_) => "played together".into(),
        Why::Loudness => "as loud".into(),
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
        let picks = picks(&library.conn, seed, &HashSet::new(), &[], 10, 1).unwrap();
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
        let fewer = super::picks(&library.conn, seed, &excluded, &[], 1, 1).unwrap();
        assert_eq!(fewer.len(), 1);
        assert_ne!(fewer[0].track_id, id(&library, "jazz94.flac"));
        assert!(super::picks(&library.conn, 999, &HashSet::new(), &[], 5, 1)
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
        let picks = picks(&library.conn, seed, &HashSet::new(), &[], 10, 3).unwrap();
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
                &[],
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

    #[test]
    fn leaves_out_tracks_of_unreadable_folders() {
        let library = Library::new([
            track("seed.flac").album("S").genre("Jazz"),
            track("here.flac").album("H").genre("Jazz"),
        ]);
        let away = library.add_folder("/Volumes/Away");
        library.add(away, track("away.flac").album("A").genre("Jazz"));
        let seed = id(&library, "seed.flac");
        let picked = |unreadable: &[i64]| -> Vec<i64> {
            picks(&library.conn, seed, &HashSet::new(), unreadable, 10, 1)
                .unwrap()
                .into_iter()
                .map(|pick| pick.track_id)
                .collect()
        };
        assert_eq!(picked(&[]).len(), 2);
        assert_eq!(picked(&[away]), [id(&library, "here.flac")]);
    }
}
