//! An in-memory library of synthetic tracks, for tests.

use std::path::Path;

use rusqlite::Connection;

use super::browse::{browse, BrowsePage, Group, GroupKey, MAX_PAGE_SIZE};
use super::db;
use super::rules::SortRule;
use super::TrackSummary;

/// A track to add to a test library; unset tags are missing.
#[derive(Default)]
pub struct Track {
    path: String,
    title: Option<String>,
    artist: Option<String>,
    album_artist: Option<String>,
    album: Option<String>,
    genre: Option<String>,
    year: Option<u32>,
    disc: Option<u32>,
    number: Option<u32>,
}

pub fn track(path: &str) -> Track {
    Track {
        path: path.into(),
        ..Track::default()
    }
}

impl Track {
    pub fn title(self, title: &str) -> Self {
        Track {
            title: Some(title.into()),
            ..self
        }
    }
    pub fn artist(self, artist: &str) -> Self {
        Track {
            artist: Some(artist.into()),
            ..self
        }
    }
    pub fn album_artist(self, album_artist: &str) -> Self {
        Track {
            album_artist: Some(album_artist.into()),
            ..self
        }
    }
    pub fn album(self, album: &str) -> Self {
        Track {
            album: Some(album.into()),
            ..self
        }
    }
    pub fn genre(self, genre: &str) -> Self {
        Track {
            genre: Some(genre.into()),
            ..self
        }
    }
    pub fn year(self, year: u32) -> Self {
        Track {
            year: Some(year),
            ..self
        }
    }
    pub fn disc(self, disc: u32) -> Self {
        Track {
            disc: Some(disc),
            ..self
        }
    }
    pub fn number(self, number: u32) -> Self {
        Track {
            number: Some(number),
            ..self
        }
    }
}

/// An in-memory library filled like the scanner fills it: the album
/// artist falls back to the artist, and an album is a title by an album
/// artist.
pub struct Library {
    pub conn: Connection,
    pub folder_id: i64,
}

impl Library {
    pub fn new(tracks: impl IntoIterator<Item = Track>) -> Library {
        let library = Library::from_conn(db::open_in_memory().unwrap());
        for track in tracks {
            library.add(library.folder_id, track);
        }
        library
    }

    /// A library in `conn`, with one folder and no tracks.
    pub fn from_conn(conn: Connection) -> Library {
        let mut library = Library { conn, folder_id: 0 };
        library.folder_id = library.add_folder("/Music");
        library
    }

    pub fn add_folder(&self, path: &str) -> i64 {
        self.conn
            .execute(
                "INSERT INTO folders (path, added_at) VALUES (?1, 0)",
                [path],
            )
            .unwrap();
        self.conn.last_insert_rowid()
    }

    pub fn artist(&self, name: Option<&str>) -> Option<i64> {
        name.map(|name| {
            self.conn
                .query_row(
                    "INSERT INTO artists (name) VALUES (?1)
                     ON CONFLICT (name) DO UPDATE SET name = name RETURNING id",
                    [name],
                    |row| row.get(0),
                )
                .unwrap()
        })
    }

    pub fn add(&self, folder_id: i64, track: Track) {
        let artist_id = self.artist(track.artist.as_deref());
        let album_artist_id = match track.album_artist.as_deref() {
            Some(name) => self.artist(Some(name)),
            None => artist_id,
        };
        let album_id: Option<i64> = track.album.as_deref().map(|title| {
            self.conn
                .query_row(
                    "INSERT INTO albums (title, artist_id) VALUES (?1, ?2)
                     ON CONFLICT (IFNULL(artist_id, 0), title) DO UPDATE SET title = title
                     RETURNING id",
                    rusqlite::params![title, album_artist_id],
                    |row| row.get(0),
                )
                .unwrap()
        });
        self.conn
            .execute(
                "INSERT INTO tracks (folder_id, relative_path, file_size, file_mtime_ns, title,
                                     artist_id, album_id, album_artist_id, genre, year,
                                     disc_number, track_number, duration, sample_rate,
                                     channels, scanned_at)
                 VALUES (?1, ?2, 0, 0, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, 60.0, 44100, 2, 0)",
                rusqlite::params![
                    folder_id,
                    track.path,
                    track.title,
                    artist_id,
                    album_id,
                    album_artist_id,
                    track.genre,
                    track.year,
                    track.disc,
                    track.number
                ],
            )
            .unwrap();
    }

    pub fn page(&self, rule: &SortRule, path: &[Option<GroupKey>]) -> BrowsePage {
        self.page_with(rule, &["The".into(), "A".into()], path)
    }

    pub fn page_with(
        &self,
        rule: &SortRule,
        articles: &[String],
        path: &[Option<GroupKey>],
    ) -> BrowsePage {
        let page = browse(&self.conn, rule, articles, path, 0, MAX_PAGE_SIZE).unwrap();
        assert_eq!(page.total as usize, page.groups.len() + page.tracks.len());
        page
    }

    pub fn groups(&self, rule: &SortRule, path: &[Option<GroupKey>]) -> Vec<Group> {
        let page = self.page(rule, path);
        assert!(page.tracks.is_empty());
        page.groups
    }

    /// The groups' names, with their track counts.
    pub fn names(&self, rule: &SortRule, path: &[Option<GroupKey>]) -> Vec<(String, u32)> {
        self.groups(rule, path)
            .into_iter()
            .map(|group| (group.name, group.track_count))
            .collect()
    }

    /// The key of the group called `name`.
    pub fn key(&self, rule: &SortRule, path: &[Option<GroupKey>], name: &str) -> Option<GroupKey> {
        let groups = self.groups(rule, path);
        let group = groups.iter().find(|group| group.name == name);
        group
            .unwrap_or_else(|| panic!("no group {name} in {groups:?}"))
            .key
            .clone()
    }

    pub fn titles(&self, rule: &SortRule, path: &[Option<GroupKey>]) -> Vec<String> {
        titles(&self.page(rule, path).tracks)
    }
}

/// Titles, or file names for tracks without one.
pub fn titles(tracks: &[TrackSummary]) -> Vec<String> {
    tracks
        .iter()
        .map(|track| {
            track.title.clone().unwrap_or_else(|| {
                Path::new(&track.path)
                    .file_name()
                    .unwrap()
                    .to_string_lossy()
                    .into_owned()
            })
        })
        .collect()
}
