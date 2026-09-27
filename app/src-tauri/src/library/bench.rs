//! Timings on a synthetic 50,000-track library, for PLAN.md. Ignored by
//! default; run in release:
//!
//! ```sh
//! cargo test --release --lib bench -- --ignored --nocapture --test-threads 1
//! ```

use std::time::{Duration, Instant};

use super::browse::{browse_rule, node_track_ids_rule, Filter, GroupKey, RuleSpec};
use super::db;
use super::search::{search, SearchKind, ALL_KINDS};
use super::sort_key::fold;
use super::test_library::{track, Library};
use crate::queue::track_infos;

const TRACKS: usize = 50_000;

const WORDS: &[&str] = &[
    "love", "night", "heart", "blue", "dream", "fire", "rain", "light", "time", "road", "home",
    "wild", "gold", "river", "stone", "summer", "winter", "shadow", "ghost", "city", "ocean",
    "star", "moon", "sun", "black", "white", "red", "electric", "golden", "broken", "silent",
    "dancing", "running", "falling", "burning", "sweet", "lonely", "crazy", "little", "big",
    "Élan", "Café", "Noël", "Über", "Señor", "Æther", "Björk", "déjà", "naïve", "façade",
];

const SYLLABLES: &[&str] = &[
    "ka", "lo", "mi", "ren", "sa", "tor", "vel", "an", "do", "ri", "zu", "bel", "cor", "fi", "gan",
    "hol", "is", "jo", "mar", "nu",
];

/// A deterministic word picker: one of `WORDS` a tenth of the time (so
/// "love" is in about 0.6% of titles, as a common word might be), otherwise
/// one of 8,000 made-up words.
struct Words(u64);

impl Words {
    fn random(&mut self) -> usize {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (self.0 >> 33) as usize
    }

    fn next(&mut self) -> String {
        if self.random() % 10 == 0 {
            return WORDS[self.random() % WORDS.len()].to_owned();
        }
        let n = self.random() % 8000;
        [n % 20, n / 20 % 20, n / 400]
            .iter()
            .map(|&i| SYLLABLES[i])
            .collect()
    }

    fn phrase(&mut self, words: usize) -> String {
        (0..words)
            .map(|_| self.next())
            .collect::<Vec<_>>()
            .join(" ")
    }
}

/// 2,000 album artists with 2–3 albums of 10 tracks, some featured artists,
/// genres and years, and a folder per album.
fn library() -> Library {
    let mut words = Words(7);
    let artists: Vec<String> = (0..2000)
        .map(|i| format!("{} {i}", words.phrase(2)))
        .collect();
    let library = Library::new([]);
    library.conn.execute_batch("BEGIN").unwrap();
    for i in 0..TRACKS {
        let album = i / 10;
        let artist = &artists[album % artists.len()];
        let mut track = track(&format!("{artist}/Album {album}/{:02}.flac", i % 10))
            .title(&words.phrase(3))
            .album_artist(artist)
            .album(&format!("{} {album}", words.phrase(2)))
            .genre(["Rock", "Pop", "Jazz; Blues", "Électro", "Folk"][album % 5])
            .year(1960 + (album % 60) as u32)
            .number((i % 10) as u32 + 1);
        track = if i % 7 == 0 {
            track.artist(&artists[(i * 13) % artists.len()])
        } else {
            track.artist(artist)
        };
        library.add(library.folder_id, track);
    }
    library.conn.execute_batch("COMMIT").unwrap();
    library
}

/// The median of `runs` timings of `f`.
fn time<T>(runs: usize, mut f: impl FnMut() -> T) -> Duration {
    let mut times: Vec<Duration> = (0..runs)
        .map(|_| {
            let start = Instant::now();
            std::hint::black_box(f());
            start.elapsed()
        })
        .collect();
    times.sort();
    times[runs / 2]
}

fn ms(duration: Duration) -> String {
    format!("{:.2} ms", duration.as_secs_f64() * 1000.0)
}

#[test]
#[ignore]
fn bench_search() {
    let start = Instant::now();
    let library = library();
    println!(
        "built {TRACKS} tracks in {:.1} s",
        start.elapsed().as_secs_f64()
    );

    for query in [
        "love",
        "cafe",
        "blue moon",
        "kalo",
        "ka",
        "k",
        "golden 1234",
        "zzz",
    ] {
        let results = search(&library.conn, query, &ALL_KINDS, 0, 50).unwrap();
        let all = time(9, || {
            search(&library.conn, query, &ALL_KINDS, 0, 50).unwrap()
        });
        let tracks = time(9, || {
            search(&library.conn, query, &[SearchKind::Tracks], 0, 50).unwrap()
        });
        let deep = time(9, || {
            search(&library.conn, query, &[SearchKind::Tracks], 1000, 50).unwrap()
        });
        println!(
            "search {query:>12}: {} artists, {} albums, {} tracks; all kinds (count + 50 each) {}, tracks only {}, tracks at offset 1000 {}",
            results.artist_total,
            results.album_total,
            results.track_total,
            ms(all),
            ms(tracks),
            ms(deep)
        );
    }

    // The alternative: LIKE over folded copies of the same fields, filled
    // from Rust (a migration couldn't fill them).
    library
        .conn
        .execute_batch("ALTER TABLE tracks ADD COLUMN folded TEXT")
        .unwrap();
    let rows: Vec<(i64, String)> = library
        .conn
        .prepare(
            "SELECT t.id, concat_ws(' ', t.title, a.name, al.title, aa.name) FROM tracks t
             LEFT JOIN artists a ON a.id = t.artist_id LEFT JOIN albums al ON al.id = t.album_id
             LEFT JOIN artists aa ON aa.id = t.album_artist_id",
        )
        .unwrap()
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    library.conn.execute_batch("BEGIN").unwrap();
    for (id, text) in rows {
        library
            .conn
            .execute(
                "UPDATE tracks SET folded = ?1 WHERE id = ?2",
                (fold(&text), id),
            )
            .unwrap();
    }
    library.conn.execute_batch("COMMIT").unwrap();
    for query in ["love", "cafe", "blue moon", "kalo", "ka"] {
        let words: Vec<String> = query.split(' ').map(|w| format!("%{}%", fold(w))).collect();
        let sql = format!(
            "SELECT count(*) FROM tracks WHERE {}",
            vec!["folded LIKE ?"; words.len()].join(" AND ")
        );
        let like = time(9, || {
            library
                .conn
                .query_row(&sql, rusqlite::params_from_iter(&words), |row| {
                    row.get::<_, i64>(0)
                })
                .unwrap()
        });
        println!("LIKE count {query:>12}: {}", ms(like));
    }
}

#[test]
#[ignore]
fn bench_node_tracks_and_queue() {
    let library = library();
    let by_artist = RuleSpec::Id("album-artist".into());
    let all = time(5, || {
        node_track_ids_rule(&library.conn, &by_artist, &[], true, Filter::default()).unwrap()
    });
    println!("all tracks under album artist, in order: {}", ms(all));
    let folder = Some(GroupKey::Number(library.folder_id));
    let tree = time(5, || {
        node_track_ids_rule(
            &library.conn,
            &RuleSpec::Id("folder".into()),
            std::slice::from_ref(&folder),
            true,
            Filter::default(),
        )
        .unwrap()
    });
    println!("a library folder's whole tree, in order: {}", ms(tree));
    let first = browse_rule(&library.conn, "album-artist", &[], 0, 1, Filter::default())
        .unwrap()
        .groups[0]
        .key
        .clone();
    let artist = time(9, || {
        node_track_ids_rule(
            &library.conn,
            &by_artist,
            std::slice::from_ref(&first),
            true,
            Filter::default(),
        )
        .unwrap()
    });
    println!("one album artist's tracks: {}", ms(artist));

    let ids = node_track_ids_rule(&library.conn, &by_artist, &[], true, Filter::default()).unwrap();
    let infos = time(5, || {
        track_infos(
            &library.conn,
            &ids,
            &crate::settings::FeatureSettings::default(),
        )
        .unwrap()
    });
    println!("track infos for {} queued tracks: {}", ids.len(), ms(infos));
    let infos = track_infos(
        &library.conn,
        &ids,
        &crate::settings::FeatureSettings::default(),
    )
    .unwrap();
    let json = time(5, || serde_json::to_string(&infos).unwrap());
    println!(
        "queue list as JSON: {} bytes, {}",
        serde_json::to_string(&infos).unwrap().len(),
        ms(json)
    );
}

#[test]
#[ignore]
fn bench_index_cost_when_scanning() {
    // The same inserts with the word indexes (schema 8, the first the test
    // library can write, since it fills the credits) and with the trigram
    // indexes for substring search too (9, F12); and the database's size
    // after.
    for version in [8, 9] {
        let conn = db::open_in_memory_at(version).unwrap();
        let start = Instant::now();
        let library = Library::from_conn(conn);
        library.conn.execute_batch("BEGIN").unwrap();
        let mut words = Words(3);
        for i in 0..TRACKS {
            library.add(
                library.folder_id,
                track(&format!("{i}.flac"))
                    .title(&words.phrase(3))
                    .artist(&format!("Artist {}", i / 25))
                    .album(&format!("Album {}", i / 10)),
            );
        }
        library.conn.execute_batch("COMMIT").unwrap();
        let elapsed = start.elapsed().as_secs_f64();
        let bytes: i64 = library
            .conn
            .query_row(
                "SELECT page_count * page_size FROM pragma_page_count, pragma_page_size",
                [],
                |row| row.get(0),
            )
            .unwrap();
        println!(
            "schema {version}: {TRACKS} inserts in {elapsed:.2} s, database {:.1} MB",
            bytes as f64 / 1e6
        );
    }
}
