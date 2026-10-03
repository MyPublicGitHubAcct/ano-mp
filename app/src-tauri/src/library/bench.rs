//! Timings on a synthetic 50,000-track library, for PLAN.md. Ignored by
//! default; `scripts/bench.py` runs them in release and compares the
//! `bench <key> <value> <unit>` lines they print with its baseline and
//! H18's budgets. By hand:
//!
//! ```sh
//! cargo test --release --lib bench -- --ignored --nocapture --test-threads 1
//! ```

use std::time::{Duration, Instant};

use super::browse::{browse_rule, node_track_ids_rule, Filter, GroupKey, RuleSpec};
use super::db;
use super::scanner::{scan_folder, Placeholders, ScanOptions};
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
        if self.random().is_multiple_of(10) {
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

/// A line for `scripts/bench.py`: a key it knows, a number and its unit.
fn report(key: &str, value: f64, unit: &str) {
    println!("bench {key} {value:.3} {unit}");
}

fn report_ms(key: &str, duration: Duration) {
    report(key, duration.as_secs_f64() * 1000.0, "ms");
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
        let key = query.replace(' ', "_");
        report_ms(&format!("search.{key}.all"), all);
        report_ms(&format!("search.{key}.tracks"), tracks);
        report_ms(&format!("search.{key}.offset"), deep);
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
    report_ms("browse.all_tracks", all);
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
    report_ms("browse.folder_tree", tree);
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
    report_ms("browse.one_artist", artist);

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
    report_ms("queue.track_infos", infos);
    let infos = track_infos(
        &library.conn,
        &ids,
        &crate::settings::FeatureSettings::default(),
    )
    .unwrap();
    let json = time(5, || serde_json::to_string(&infos).unwrap());
    let bytes = serde_json::to_string(&infos).unwrap().len();
    println!("queue list as JSON: {bytes} bytes, {}", ms(json));
    report_ms("queue.json", json);
    report("queue.json_size", bytes as f64 / 1e6, "MB");
}

#[test]
#[ignore]
fn bench_browse_pages() {
    // The first page of each built-in grouping, and of its first group: what
    // the browser shows when it opens a view or a node.
    let library = library();
    for rule in ["album-artist", "genre", "year", "folder", "composer"] {
        let page = time(9, || {
            browse_rule(&library.conn, rule, &[], 0, 100, Filter::default()).unwrap()
        });
        let top = browse_rule(&library.conn, rule, &[], 0, 1, Filter::default()).unwrap();
        let path: Vec<Option<GroupKey>> = top
            .groups
            .first()
            .map(|g| g.key.clone())
            .into_iter()
            .collect();
        let node = time(9, || {
            browse_rule(&library.conn, rule, &path, 0, 100, Filter::default()).unwrap()
        });
        println!(
            "browse {rule:>12}: first page {}, its first group's page {}",
            ms(page),
            ms(node)
        );
        report_ms(&format!("browse.page.{rule}"), page);
        report_ms(&format!("browse.node.{rule}"), node);
    }
}

#[test]
#[ignore]
fn bench_scan() {
    // 50,000 files: the core's audio fixtures copied into folders of 21.
    // On APFS `fs::copy` clones, so the copies take no space; the scan still
    // opens and reads each one.
    let fixtures =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../core/tests/fixtures");
    let mut names: Vec<_> = std::fs::read_dir(&fixtures)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.is_file())
        .collect();
    names.sort();
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("Music");
    let start = Instant::now();
    for i in 0..TRACKS {
        let source = &names[i % names.len()];
        let folder = root.join(format!("Artist {}/Album {}", i / 210, i / names.len()));
        if i % names.len() == 0 {
            std::fs::create_dir_all(&folder).unwrap();
        }
        let name = source.file_name().unwrap().to_string_lossy();
        std::fs::copy(
            source,
            folder.join(format!("{:02} {name}", i % names.len())),
        )
        .unwrap();
    }
    println!(
        "copied {TRACKS} files in {:.1} s",
        start.elapsed().as_secs_f64()
    );

    let mut conn = db::open_in_memory().unwrap();
    let folder = super::add_folder(&conn, &root).unwrap();
    let options = ScanOptions {
        parts: true,
        background: false,
        remove_missing: false,
        placeholders: Placeholders::System,
    };
    let start = Instant::now();
    let scanned = scan_folder(&mut conn, folder.id, options, |_| {}).unwrap();
    let first = start.elapsed();
    let start = Instant::now();
    let again = scan_folder(&mut conn, folder.id, options, |_| {}).unwrap();
    let rescan = start.elapsed();
    println!(
        "scan of {TRACKS} files: {} added, {} failed, in {:.1} s; unchanged rescan ({} unchanged) in {:.2} s",
        scanned.added,
        scanned.failed.len(),
        first.as_secs_f64(),
        again.unchanged,
        rescan.as_secs_f64()
    );
    assert_eq!(scanned.added, TRACKS);
    assert!(scanned.failed.is_empty());
    assert_eq!(again.unchanged, TRACKS);
    report("scan.first", first.as_secs_f64(), "s");
    report("scan.rescan", rescan.as_secs_f64(), "s");
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
        report(&format!("insert.schema{version}"), elapsed, "s");
        report(
            &format!("insert.schema{version}_size"),
            bytes as f64 / 1e6,
            "MB",
        );
    }
}

/// A player that opens everything at once and plays nothing, for timing
/// the queue alone.
struct Idle;

impl crate::queue::model::Player for Idle {
    fn load(&mut self, _: i64) -> crate::queue::model::Opening {
        crate::queue::model::Opening::Done(Ok(()))
    }
    fn set_next(&mut self, _: Option<i64>, _: bool) -> crate::queue::model::Opening {
        crate::queue::model::Opening::Done(Ok(()))
    }
    fn cancel(&mut self, _: crate::queue::model::Request) {}
    fn volume(&self) -> f64 {
        1.0
    }
    fn set_volume(&mut self, _: f64) {}
    fn play(&mut self) -> bool {
        true
    }
    fn pause(&mut self) {}
    fn stop(&mut self) {}
    fn seek(&mut self, _: f64) -> bool {
        true
    }
    fn state(&self) -> crate::anomp::PlayerState {
        crate::anomp::PlayerState::Paused
    }
    fn position(&self) -> f64 {
        0.0
    }
    fn advance_count(&self) -> i64 {
        0
    }
}

#[test]
#[ignore]
fn bench_queue_storage() {
    // A 50,000-item queue: what one change costs to send and to save, and
    // what the whole queue costs to load at launch (PLAN.md H16).
    use crate::queue::model::Queue;
    let library = library();
    let features = crate::settings::FeatureSettings::default();
    let by_artist = RuleSpec::Id("album-artist".into());
    let ids = node_track_ids_rule(&library.conn, &by_artist, &[], true, Filter::default()).unwrap();
    let infos = track_infos(&library.conn, &ids, &features).unwrap();
    // The saved queue goes into the library's own database, as the app's
    // does, here a file (with WAL), so writes cost what they do there.
    let path = tempfile::tempdir().unwrap();
    let file = path.path().join("library.sqlite3");
    library
        .conn
        .execute("VACUUM INTO ?1", [file.to_str().unwrap()])
        .unwrap();
    let conn = db::open(&file).unwrap();
    let mut store = crate::queue::store::Store::default();
    let mut queue = Queue::new(1);
    queue.replace(&mut Idle, infos.clone(), 0, false);
    let _ = queue.take_state();
    crate::queue::save_to(&conn, &mut queue, &mut store, 0.0, 1.0).unwrap();

    // Moving to the next track: the list doesn't change.
    let next = time(9, || {
        queue.next(&mut Idle);
        let state = queue.take_state();
        crate::queue::save_to(&conn, &mut queue, &mut store, 0.0, 1.0).unwrap();
        state
    });
    // "Play next" with one track, two items after the current one.
    let mut sizes = Vec::new();
    let add = time(9, || {
        queue.add(&mut Idle, vec![infos[7].clone()], true);
        let state = queue.take_state().unwrap();
        sizes.push(serde_json::to_string(&state).unwrap().len());
        crate::queue::save_to(&conn, &mut queue, &mut store, 0.0, 1.0).unwrap();
    });
    let restore = time(5, || crate::queue::restore(&conn, &features, 1).unwrap());
    // Of which: the saved list itself, and the tracks' details for it.
    let rows = time(5, || crate::queue::store::Store::load(&conn).unwrap());
    let details = time(5, || track_infos(&conn, &ids, &features).unwrap());
    println!(
        "restore: the rows {}, the tracks' details {}",
        ms(rows),
        ms(details)
    );
    report_ms("queue.restore_rows", rows);
    let size = sizes[sizes.len() / 2];
    println!(
        "queue of {} items: next track (state and save) {}, play next (state {size} bytes, save) {}, restore {}",
        ids.len(),
        ms(next),
        ms(add),
        ms(restore)
    );
    report_ms("queue.next", next);
    report_ms("queue.add_next", add);
    report("queue.add_next_size", size as f64 / 1e6, "MB");
    report_ms("queue.restore", restore);
}

#[test]
#[ignore]
fn bench_cover_thumbnails() {
    // A 1,500 px cover, as the Cover Art Archive's large size or a scan
    // might be: what making each thumbnail costs, and what it sends.
    use super::thumbs::{make, Size};
    let image = image::RgbImage::from_fn(1500, 1500, |x, y| {
        image::Rgb([(x % 256) as u8, (y % 256) as u8, ((x + y) % 256) as u8])
    });
    let mut cover = Vec::new();
    image::DynamicImage::ImageRgb8(image)
        .write_to(
            &mut std::io::Cursor::new(&mut cover),
            image::ImageFormat::Jpeg,
        )
        .unwrap();
    for size in [Size::List, Size::Header] {
        let thumb = make(&cover, size.pixels()).unwrap();
        let made = time(5, || make(&cover, size.pixels()).unwrap());
        let name = match size {
            Size::List => "list",
            Size::Header => "header",
        };
        println!(
            "thumbnail {name}: {} bytes from {}, made in {}",
            thumb.len(),
            cover.len(),
            ms(made)
        );
        report_ms(&format!("art.thumbnail_{name}"), made);
        report(
            &format!("art.thumbnail_{name}_size"),
            thumb.len() as f64 / 1e6,
            "MB",
        );
    }
    report("art.cover_size", cover.len() as f64 / 1e6, "MB");
}
