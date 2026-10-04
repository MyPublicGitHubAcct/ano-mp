//! Recommendations from outside the library (PLAN.md X5): artists the user
//! doesn't own, seeded by the library artists they play most.
//!
//! The seeds are the artists of the user's taste as X4 reads it (the last
//! 90 days' plays, else the last 200, and the favourites), those with a
//! MusicBrainz id; only their ids go out, to ListenBrainz's similar
//! artists (`listenbrainz`). MusicBrainz relations already stored with
//! each seed's match (members, subgroups, collaborations) add candidates
//! without a request. A candidate scores for each seed that suggests it,
//! by how high ListenBrainz ranks it and how much the seed is played.
//!
//! Anything the library has is left out, by MusicBrainz id (the tags' or
//! the artist's match) and by folded name, as are the suggestions the user
//! dismissed (`outside_dismissed`, migration 013). Results are links out
//! (`OutsideLinks`), never streams or downloads. Nothing is stored but the
//! response cache and the dismissals: the list is worked out each time.

use std::collections::{HashMap, HashSet};

use rusqlite::{params, Connection};
use serde::Serialize;

use super::albums::LinkStatus;
use super::artists;
use super::listenbrainz::{self, SimilarArtist};
use super::musicbrainz::{self, Artist, VARIOUS_ARTISTS};
use super::settings::SourceId;
use super::Error;
use crate::library::similar::{Relation, FALLBACK_PLAYS, LATELY};
use crate::library::sort_key::fold;

/// How many suggestions a view shows.
pub const SHOWN: usize = 12;

/// The library artists whose ids are sent for Home's suggestions, at most.
pub const SEEDS: usize = 5;

/// Of each seed's ListenBrainz list, the best this many are considered.
const PER_SEED: usize = 30;

/// A MusicBrainz relation with a seed counts as this share of
/// ListenBrainz's best match for it.
const RELATED: f64 = 0.8;

/// A seed's taste weights: a play, a favourite track, and a favourite
/// album or artist.
const PLAY: f64 = 1.0;
const FAVOURITE_TRACK: f64 = 2.0;
const FAVOURITE: f64 = 3.0;

/// Why an artist is suggested, for the UI to say (`outside.reason.*`).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum OutsideReason {
    /// ListenBrainz's listeners of this library artist play it too.
    ListenBrainz { name: String },
    /// Linked to this library artist on MusicBrainz.
    Linked { name: String, relation: Relation },
}

/// An artist the library doesn't have.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct OutsideArtist {
    /// MusicBrainz's id.
    pub mbid: String,
    pub name: String,
    /// MusicBrainz's disambiguation, e.g. "UK rock band".
    pub disambiguation: Option<String>,
    /// The strongest first; at most two.
    pub reasons: Vec<OutsideReason>,
}

/// Where a suggestion's links lead: its MusicBrainz and ListenBrainz pages
/// always, and its homepage and Bandcamp page when MusicBrainz has them.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct OutsideLinks {
    pub musicbrainz: String,
    pub listenbrainz: String,
    pub homepage: Option<String>,
    pub bandcamp: Option<String>,
}

/// A library artist whose id is sent.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct Seed {
    #[serde(skip)]
    pub artist_id: i64,
    pub mbid: String,
    pub name: String,
    #[serde(skip)]
    pub weight: f64,
}

/// What Settings shows: what is sent, and how many were dismissed.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct OutsideStatus {
    /// The artists whose MusicBrainz ids Home's suggestions send now.
    pub sent: Vec<Seed>,
    pub dismissed: u32,
}

/// Fetches ListenBrainz's similar artists for a MusicBrainz id: online
/// through the client and the cache, or from the cache alone.
pub type Fetch<'a> = dyn FnMut(&str) -> Result<Vec<SimilarArtist>, Error> + 'a;

/// The MusicBrainz id of library artist `artist_id`: its match, else the
/// tags'. Never Various Artists'.
pub fn artist_mbid(conn: &Connection, artist_id: i64) -> Result<Option<String>, Error> {
    Ok(mbids(conn)?.remove(&artist_id))
}

/// Every library artist's MusicBrainz id, as `artist_mbid`.
fn mbids(conn: &Connection) -> Result<HashMap<i64, String>, Error> {
    let rows: Vec<(i64, Option<String>, Option<String>)> = conn
        .prepare_cached(
            "SELECT a.id, l.external_id, a.musicbrainz_id
             FROM artists a
             LEFT JOIN artist_links l
                 ON l.artist_id = a.id AND l.source = ?1 AND l.status = ?2",
        )?
        .query_map(
            params![SourceId::MusicBrainz.as_str(), LinkStatus::Matched.as_str()],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )?
        .collect::<Result<_, _>>()?;
    Ok(rows
        .into_iter()
        .filter_map(|(id, linked, tagged)| {
            let mbid = [linked, tagged].into_iter().flatten().find(|mbid| {
                musicbrainz::is_mbid(mbid) && !mbid.eq_ignore_ascii_case(VARIOUS_ARTISTS)
            })?;
            Some((id, mbid.to_ascii_lowercase()))
        })
        .collect())
}

/// The artists of the user's taste (before `now`) with a MusicBrainz id,
/// the most played first, at most `count`.
pub fn seeds(conn: &Connection, now: i64, count: usize) -> Result<Vec<Seed>, Error> {
    let mut weights: HashMap<i64, f64> = HashMap::new();
    let mut add = |rows: Vec<(i64, f64)>| {
        for (artist, weight) in rows {
            *weights.entry(artist).or_default() += weight;
        }
    };
    let rows = |sql: &str, param: i64| -> Result<Vec<(i64, f64)>, Error> {
        Ok(conn
            .prepare_cached(sql)?
            .query_map([param], |row| Ok((row.get(0)?, row.get(1)?)))?
            .collect::<Result<_, _>>()?)
    };
    let mut played = rows(
        "SELECT t.artist_id, count(*) FROM plays p JOIN tracks t ON t.id = p.track_id
         WHERE p.played_at >= ?1 AND t.artist_id IS NOT NULL GROUP BY t.artist_id",
        now - LATELY,
    )?;
    if played.is_empty() {
        played = rows(
            "SELECT t.artist_id, count(*) FROM
                 (SELECT track_id FROM plays ORDER BY played_at DESC, id DESC LIMIT ?1) p
             JOIN tracks t ON t.id = p.track_id
             WHERE t.artist_id IS NOT NULL GROUP BY t.artist_id",
            FALLBACK_PLAYS as i64,
        )?;
    }
    add(played
        .into_iter()
        .map(|(artist, plays)| (artist, plays * PLAY))
        .collect());
    let favourites = |sql: &str, weight: f64| -> Result<Vec<(i64, f64)>, Error> {
        Ok(conn
            .prepare_cached(sql)?
            .query_map([], |row| Ok((row.get(0)?, weight)))?
            .collect::<Result<_, _>>()?)
    };
    add(favourites(
        "SELECT t.artist_id FROM track_favourites f JOIN tracks t ON t.id = f.track_id
         WHERE t.artist_id IS NOT NULL",
        FAVOURITE_TRACK,
    )?);
    add(favourites(
        "SELECT a.artist_id FROM album_favourites f JOIN albums a ON a.id = f.album_id
         WHERE a.artist_id IS NOT NULL",
        FAVOURITE,
    )?);
    add(favourites(
        "SELECT artist_id FROM artist_favourites",
        FAVOURITE,
    )?);

    let mut mbids = mbids(conn)?;
    let mut ranked: Vec<(i64, f64)> = weights.into_iter().collect();
    ranked.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
    let mut seeds = Vec::new();
    for (artist_id, weight) in ranked {
        if seeds.len() == count {
            break;
        }
        let Some(mbid) = mbids.remove(&artist_id) else {
            continue;
        };
        let name: String = conn.query_row(
            "SELECT name FROM artists WHERE id = ?1",
            [artist_id],
            |row| row.get(0),
        )?;
        seeds.push(Seed {
            artist_id,
            mbid,
            name,
            weight,
        });
    }
    Ok(seeds)
}

/// The seed made of library artist `artist_id` alone, if it has an id.
pub fn artist_seed(conn: &Connection, artist_id: i64) -> Result<Option<Seed>, Error> {
    let Some(mbid) = artist_mbid(conn, artist_id)? else {
        return Ok(None);
    };
    let name: String = conn.query_row(
        "SELECT name FROM artists WHERE id = ?1",
        [artist_id],
        |row| row.get(0),
    )?;
    Ok(Some(Seed {
        artist_id,
        mbid,
        name,
        weight: 1.0,
    }))
}

/// What the library has: every artist's MusicBrainz id and folded name.
struct Owned {
    mbids: HashSet<String>,
    names: HashSet<String>,
}

impl Owned {
    fn load(conn: &Connection) -> Result<Owned, Error> {
        let mut mbids: HashSet<String> = mbids(conn)?.into_values().collect();
        let mut names = HashSet::new();
        let rows: Vec<(String, Option<String>)> = conn
            .prepare_cached("SELECT name, musicbrainz_id FROM artists")?
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
            .collect::<Result<_, _>>()?;
        for (name, tagged) in rows {
            names.insert(fold(&name));
            // A tag's id counts even where a match differs from it.
            if let Some(tagged) = tagged {
                mbids.insert(tagged.to_ascii_lowercase());
            }
        }
        Ok(Owned { mbids, names })
    }

    fn has(&self, mbid: &str, name: &str) -> bool {
        self.mbids.contains(&mbid.to_ascii_lowercase()) || self.names.contains(&fold(name))
    }
}

/// A candidate as it is scored.
struct Candidate {
    name: String,
    disambiguation: Option<String>,
    score: f64,
    /// With what each reason added.
    reasons: Vec<(f64, OutsideReason)>,
}

/// How MusicBrainz's relation type reads as X4's.
fn relation(kind: &str) -> Relation {
    match kind {
        "member of band" => Relation::Member,
        "subgroup" => Relation::Subgroup,
        _ => Relation::With,
    }
}

/// The artists like `seeds` that the library doesn't have and the user
/// hasn't dismissed, the best first, at most `count`. `fetch` gives each
/// seed's ListenBrainz list; a seed whose list fails still counts for its
/// MusicBrainz relations, and the first failure is returned only when
/// nothing at all was found.
pub fn recommend(
    conn: &Connection,
    seeds: &[Seed],
    fetch: &mut Fetch,
    count: usize,
) -> Result<Vec<OutsideArtist>, Error> {
    let total: f64 = seeds.iter().map(|seed| seed.weight).sum();
    if seeds.is_empty() || total <= 0.0 {
        return Ok(Vec::new());
    }
    let owned = Owned::load(conn)?;
    let dismissed = dismissed(conn)?;
    let mut found: HashMap<String, Candidate> = HashMap::new();
    let mut add = |mbid: &str,
                   name: &str,
                   disambiguation: Option<&String>,
                   score: f64,
                   reason: OutsideReason| {
        let mbid = mbid.to_ascii_lowercase();
        if mbid == VARIOUS_ARTISTS || owned.has(&mbid, name) || dismissed.contains(&mbid) {
            return;
        }
        let candidate = found.entry(mbid).or_insert_with(|| Candidate {
            name: name.to_owned(),
            disambiguation: None,
            score: 0.0,
            reasons: Vec::new(),
        });
        if candidate.disambiguation.is_none() {
            candidate.disambiguation = disambiguation.cloned();
        }
        candidate.score += score;
        candidate.reasons.push((score, reason));
    };

    let mut failure = None;
    for seed in seeds {
        let share = seed.weight / total;
        match fetch(&seed.mbid) {
            Ok(similar) => {
                let best = similar
                    .iter()
                    .map(|artist| artist.score)
                    .fold(0.0, f64::max);
                for artist in similar.iter().take(PER_SEED) {
                    let rank = if best > 0.0 { artist.score / best } else { 0.0 };
                    add(
                        &artist.mbid,
                        &artist.name,
                        artist.comment.as_ref(),
                        share * rank,
                        OutsideReason::ListenBrainz {
                            name: seed.name.clone(),
                        },
                    );
                }
            }
            Err(error) => {
                failure.get_or_insert(error);
            }
        }
        let stored = artists::artist_link(conn, seed.artist_id, SourceId::MusicBrainz)?
            .filter(|link| link.matched_id() == Some(seed.mbid.as_str()))
            .and_then(|link| link.details::<Artist>());
        let mut seen = HashSet::new();
        for related in stored.iter().flat_map(|artist| &artist.related) {
            if musicbrainz::is_mbid(&related.id) && seen.insert(related.id.clone()) {
                add(
                    &related.id,
                    &related.name,
                    None,
                    share * RELATED,
                    OutsideReason::Linked {
                        name: seed.name.clone(),
                        relation: relation(&related.relation),
                    },
                );
            }
        }
    }
    if found.is_empty() {
        if let Some(error) = failure {
            return Err(error);
        }
    }

    let mut ranked: Vec<(String, Candidate)> = found.into_iter().collect();
    ranked.sort_by(|(a_id, a), (b_id, b)| {
        b.score
            .total_cmp(&a.score)
            .then_with(|| a.name.cmp(&b.name))
            .then_with(|| a_id.cmp(b_id))
    });
    Ok(ranked
        .into_iter()
        .take(count)
        .map(|(mbid, mut candidate)| {
            candidate.reasons.sort_by(|a, b| b.0.total_cmp(&a.0));
            let mut reasons: Vec<OutsideReason> = Vec::new();
            for (_, reason) in candidate.reasons {
                if reasons.len() < 2 && !reasons.contains(&reason) {
                    reasons.push(reason);
                }
            }
            OutsideArtist {
                mbid,
                name: candidate.name,
                disambiguation: candidate.disambiguation,
                reasons,
            }
        })
        .collect())
}

/// The links for MusicBrainz artist `mbid`, with its homepage and Bandcamp
/// page from `artist` (its MusicBrainz lookup) when there is one.
pub fn links(mbid: &str, artist: Option<&Artist>) -> OutsideLinks {
    OutsideLinks {
        musicbrainz: format!("https://musicbrainz.org/artist/{mbid}"),
        listenbrainz: listenbrainz::artist_page(mbid),
        homepage: artist.and_then(|artist| artist.homepage.clone()),
        bandcamp: artist.and_then(|artist| artist.bandcamp.clone()),
    }
}

/// Remembers that the user doesn't want `mbid` suggested again.
pub fn dismiss(conn: &Connection, mbid: &str, name: &str, now: i64) -> Result<(), Error> {
    if !musicbrainz::is_mbid(mbid) {
        return Err(Error::Invalid(format!("Not a MusicBrainz id: {mbid}")));
    }
    conn.execute(
        "INSERT INTO outside_dismissed (musicbrainz_id, name, dismissed_at) VALUES (?1, ?2, ?3)
         ON CONFLICT (musicbrainz_id) DO UPDATE SET name = excluded.name",
        params![mbid.to_ascii_lowercase(), name, now],
    )?;
    Ok(())
}

fn dismissed(conn: &Connection) -> Result<HashSet<String>, Error> {
    Ok(conn
        .prepare_cached("SELECT musicbrainz_id FROM outside_dismissed")?
        .query_map([], |row| row.get(0))?
        .collect::<Result<_, _>>()?)
}

/// Forgets every dismissal; returns how many there were.
pub fn forget_dismissed(conn: &Connection) -> Result<usize, Error> {
    Ok(conn.execute("DELETE FROM outside_dismissed", [])?)
}

pub fn status(conn: &Connection, now: i64) -> Result<OutsideStatus, Error> {
    let dismissed: u32 = conn.query_row("SELECT count(*) FROM outside_dismissed", [], |row| {
        row.get(0)
    })?;
    Ok(OutsideStatus {
        sent: seeds(conn, now, SEEDS)?,
        dismissed,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::library::test_library::{track, Library};
    use crate::metadata::listenbrainz::fixtures::{RADIOHEAD, SIMILAR};
    use crate::metadata::musicbrainz::fixtures::ARTIST_JSON;

    const NOW: i64 = 1_800_000_000;
    const BEATLES: &str = "b10bbbfc-cf9e-42e0-be17-e2c3e1d2600d";
    const NIRVANA: &str = "5b11f4ce-a62d-471e-81fc-a69a8278c7da";
    const GILMOUR: &str = "1dce970e-34bc-48b2-ab51-48d87544a4c2";
    const THOM: &str = "8ed2e0b3-aa4c-4e13-bec3-dc7393ed4d6b";
    const SMILE: &str = "9055fca5-69bc-4d47-b569-ab2012a19565";
    const PORTISHEAD: &str = "8f6bd1e4-fbe1-4f50-aa9b-94c450ec0f11";

    /// Radiohead (matched on MusicBrainz, with its relations), the Beatles
    /// by name only, Nirvana by its tag id, and Portishead, played less.
    fn library() -> Library {
        let library = Library::new([
            track("rh/1.flac")
                .title("15 Step")
                .artist("Radiohead")
                .album("In Rainbows"),
            track("rh/2.flac")
                .title("Bodysnatchers")
                .artist("Radiohead")
                .album("In Rainbows"),
            track("b/1.flac")
                .title("Help!")
                .artist("the beatles")
                .album("Help!"),
            track("n/1.flac")
                .title("Lithium")
                .artist("Nirvana (US)")
                .album("Nevermind"),
            track("p/1.flac")
                .title("Roads")
                .artist("Portishead")
                .album("Dummy"),
        ]);
        let conn = &library.conn;
        let id = |name: &str| -> i64 {
            conn.query_row("SELECT id FROM artists WHERE name = ?1", [name], |row| {
                row.get(0)
            })
            .unwrap()
        };
        conn.execute(
            "UPDATE artists SET musicbrainz_id = ?1 WHERE id = ?2",
            params![NIRVANA, id("Nirvana (US)")],
        )
        .unwrap();
        conn.execute(
            "UPDATE artists SET musicbrainz_id = ?1 WHERE id = ?2",
            params![PORTISHEAD, id("Portishead")],
        )
        .unwrap();
        let radiohead = musicbrainz::parse_artist(ARTIST_JSON).unwrap();
        artists::store_link(
            conn,
            id("Radiohead"),
            SourceId::MusicBrainz,
            LinkStatus::Matched,
            Some(RADIOHEAD),
            1.0,
            Some(&radiohead),
        )
        .unwrap();
        library
    }

    fn play(library: &Library, title: &str, times: usize, at: i64) {
        for i in 0..times {
            library
                .conn
                .execute(
                    "INSERT INTO plays (track_id, played_at, seconds)
                     SELECT id, ?2, 60 FROM tracks WHERE title = ?1",
                    params![title, at + i as i64],
                )
                .unwrap();
        }
    }

    fn id(library: &Library, name: &str) -> i64 {
        library
            .conn
            .query_row("SELECT id FROM artists WHERE name = ?1", [name], |row| {
                row.get(0)
            })
            .unwrap()
    }

    fn seed(library: &Library, name: &str) -> Seed {
        artist_seed(&library.conn, id(library, name))
            .unwrap()
            .unwrap()
    }

    fn names(found: &[OutsideArtist]) -> Vec<&str> {
        found.iter().map(|artist| artist.name.as_str()).collect()
    }

    /// ListenBrainz's recorded list for Radiohead; nothing for others.
    fn recorded(
        requests: &mut Vec<String>,
    ) -> impl FnMut(&str) -> Result<Vec<SimilarArtist>, Error> + '_ {
        move |mbid| {
            requests.push(mbid.to_owned());
            if mbid == RADIOHEAD {
                listenbrainz::parse_similar_artists(SIMILAR)
            } else {
                Ok(Vec::new())
            }
        }
    }

    #[test]
    fn seeds_are_the_most_played_artists_with_an_id() {
        let library = library();
        play(&library, "Roads", 2, NOW - 3600);
        play(&library, "15 Step", 3, NOW - 3600);
        play(&library, "Help!", 9, NOW - 3600);
        // Played long ago: outside the 90 days, so not counted.
        play(&library, "Lithium", 20, NOW - 200 * 86400);
        let seeds = seeds(&library.conn, NOW, SEEDS).unwrap();
        // The Beatles are played most but have no id; Nirvana only long ago.
        let named: Vec<(&str, &str)> = seeds
            .iter()
            .map(|seed| (seed.name.as_str(), seed.mbid.as_str()))
            .collect();
        assert_eq!(
            named,
            [("Radiohead", RADIOHEAD), ("Portishead", PORTISHEAD)]
        );
        // A favourite artist outweighs two plays.
        library
            .conn
            .execute(
                "INSERT INTO artist_favourites (artist_id, added_at)
                 SELECT id, 0 FROM artists WHERE name = 'Nirvana (US)'",
                [],
            )
            .unwrap();
        let seeds = super::seeds(&library.conn, NOW, 2).unwrap();
        assert_eq!(seeds[1].name, "Nirvana (US)");
    }

    #[test]
    fn seeds_fall_back_to_the_last_plays() {
        let library = library();
        play(&library, "Roads", 1, NOW - 200 * 86400);
        let seeds = seeds(&library.conn, NOW, SEEDS).unwrap();
        assert_eq!(seeds.len(), 1);
        assert_eq!(seeds[0].name, "Portishead");
        assert!(super::seeds(&Library::new([]).conn, NOW, SEEDS)
            .unwrap()
            .is_empty());
    }

    #[test]
    fn suggests_what_the_library_lacks() {
        let library = library();
        let seed = seed(&library, "Radiohead");
        assert_eq!(seed.mbid, RADIOHEAD);
        assert!(artist_seed(&library.conn, id(&library, "the beatles"))
            .unwrap()
            .is_none());
        let mut requests = Vec::new();
        let found = recommend(&library.conn, &[seed], &mut recorded(&mut requests), SHOWN).unwrap();
        assert_eq!(requests, [RADIOHEAD]);
        let shown = names(&found);
        // The Beatles are in the library by folded name, Nirvana by id.
        assert!(found.iter().all(|artist| artist.mbid != BEATLES));
        assert!(!shown.contains(&"Nirvana"));
        assert_eq!(shown[0], "Red Hot Chili Peppers");
        assert!(shown.contains(&"David Gilmour"));
        // MusicBrainz's relations come in too, with their own reasons.
        let thom = found.iter().find(|artist| artist.mbid == THOM).unwrap();
        assert_eq!(
            thom.reasons,
            [OutsideReason::Linked {
                name: "Radiohead".into(),
                relation: Relation::Member
            }]
        );
        let smile = found.iter().find(|artist| artist.mbid == SMILE).unwrap();
        assert_eq!(
            smile.reasons[0],
            OutsideReason::Linked {
                name: "Radiohead".into(),
                relation: Relation::Subgroup
            }
        );
        let gilmour = found.iter().find(|artist| artist.mbid == GILMOUR).unwrap();
        assert_eq!(gilmour.disambiguation.as_deref(), Some("Pink Floyd"));
        assert_eq!(
            gilmour.reasons,
            [OutsideReason::ListenBrainz {
                name: "Radiohead".into()
            }]
        );
        assert_eq!(found.len(), 8, "six from ListenBrainz and two relations");
        assert!(
            recommend(&library.conn, &[], &mut recorded(&mut Vec::new()), SHOWN)
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn dismissed_suggestions_stay_away() {
        let library = library();
        let seed = seed(&library, "Radiohead");
        dismiss(&library.conn, &GILMOUR.to_uppercase(), "David Gilmour", NOW).unwrap();
        dismiss(&library.conn, THOM, "Thom Yorke", NOW).unwrap();
        assert!(dismiss(&library.conn, "../x", "X", NOW).is_err());
        let found = recommend(
            &library.conn,
            std::slice::from_ref(&seed),
            &mut recorded(&mut Vec::new()),
            SHOWN,
        )
        .unwrap();
        assert!(!names(&found).contains(&"David Gilmour"));
        assert!(!names(&found).contains(&"Thom Yorke"));
        assert_eq!(status(&library.conn, NOW).unwrap().dismissed, 2);
        assert_eq!(forget_dismissed(&library.conn).unwrap(), 2);
        let found = recommend(
            &library.conn,
            &[seed],
            &mut recorded(&mut Vec::new()),
            SHOWN,
        )
        .unwrap();
        assert!(names(&found).contains(&"David Gilmour"));
    }

    #[test]
    fn several_seeds_add_up_and_a_failure_is_tolerated() {
        let library = library();
        let radiohead = seed(&library, "Radiohead");
        let portishead = Seed {
            weight: 3.0,
            ..seed(&library, "Portishead")
        };
        // Portishead's listeners play David Gilmour too, ranked first.
        let mut fetch = |mbid: &str| -> Result<Vec<SimilarArtist>, Error> {
            match mbid {
                RADIOHEAD => listenbrainz::parse_similar_artists(SIMILAR),
                _ => Ok(vec![SimilarArtist {
                    mbid: GILMOUR.into(),
                    name: "David Gilmour".into(),
                    comment: None,
                    artist_type: None,
                    score: 10.0,
                }]),
            }
        };
        let found = recommend(
            &library.conn,
            &[radiohead.clone(), portishead.clone()],
            &mut fetch,
            SHOWN,
        )
        .unwrap();
        assert_eq!(found[0].name, "David Gilmour");
        assert_eq!(
            found[0].reasons,
            [
                OutsideReason::ListenBrainz {
                    name: "Portishead".into()
                },
                OutsideReason::ListenBrainz {
                    name: "Radiohead".into()
                }
            ]
        );
        // ListenBrainz offline: Radiohead's relations still show.
        let mut offline =
            |_: &str| -> Result<Vec<SimilarArtist>, Error> { Err(Error::Offline("down".into())) };
        let found = recommend(
            &library.conn,
            &[radiohead, portishead.clone()],
            &mut offline,
            SHOWN,
        )
        .unwrap();
        assert_eq!(names(&found), ["The Smile", "Thom Yorke"]);
        // With nothing at all, the failure is the answer.
        assert!(matches!(
            recommend(&library.conn, &[portishead], &mut offline, SHOWN),
            Err(Error::Offline(_))
        ));
    }

    #[test]
    fn links_lead_out() {
        let radiohead = musicbrainz::parse_artist(ARTIST_JSON).unwrap();
        let links = links(RADIOHEAD, Some(&radiohead));
        assert_eq!(
            links.musicbrainz,
            format!("https://musicbrainz.org/artist/{RADIOHEAD}")
        );
        assert_eq!(
            links.listenbrainz,
            format!("https://listenbrainz.org/artist/{RADIOHEAD}/")
        );
        assert_eq!(links.homepage.as_deref(), Some("http://www.radiohead.com/"));
        assert_eq!(
            links.bandcamp.as_deref(),
            Some("https://radiohead.bandcamp.com/")
        );
        assert_eq!(super::links(RADIOHEAD, None).homepage, None);
    }
}
