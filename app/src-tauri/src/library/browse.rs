//! Browsing the library under a sort rule (`rules`), one node at a time: the
//! children of a node (e.g. every album artist, then one artist's albums,
//! then an album's tracks) a page at a time, so a large library is never
//! loaded whole.
//!
//! Queries are assembled only from fixed SQL fragments picked by the rule's
//! levels and keys. Everything else (path keys, articles, paging) is a bound
//! parameter.

use rusqlite::types::{Value, ValueRef};
use rusqlite::{Connection, Row};
use serde::{Deserialize, Serialize};

use super::rules::{self, Level, SortRule, TrackKey};
use super::{track_from_row, Error, TrackSummary, TRACKS_FROM, TRACK_COLUMNS};

/// Larger page sizes are reduced to this.
pub const MAX_PAGE_SIZE: u32 = 1000;

/// Identifies a group within its level.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum GroupKey {
    /// An artist, album or library folder id, or a year.
    Number(i64),
    /// A genre, or a folder's name within its parent.
    Text(String),
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Group {
    /// Append it to the path to browse into the group. Null for the tracks
    /// that have no value at this level ("Unknown artist" and so on).
    pub key: Option<GroupKey>,
    pub name: String,
    pub track_count: u32,
    /// Album groups only: the album artist, and the album's year (the
    /// earliest among its tracks).
    pub album_artist: Option<String>,
    pub year: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowsePage {
    /// A node lists its groups, then its tracks. Nodes of tag levels have
    /// one or the other; a folder can have both.
    pub groups: Vec<Group>,
    pub tracks: Vec<TrackSummary>,
    /// Groups plus tracks in the whole node.
    pub total: u32,
}

/// Browses the stored rule `rule_id` with the stored ignored articles; see
/// `browse`.
pub fn browse_rule(
    conn: &Connection,
    rule_id: &str,
    path: &[Option<GroupKey>],
    offset: u32,
    limit: u32,
) -> Result<BrowsePage, Error> {
    let settings = rules::sort_settings(conn)?;
    let rule = settings
        .rules
        .iter()
        .find(|rule| rule.id == rule_id)
        .ok_or_else(|| Error::Invalid(format!("No sort rule with id \"{rule_id}\"")))?;
    browse(conn, rule, &settings.ignored_articles, path, offset, limit)
}

/// Lists up to `limit` children of the node at `path`, from `offset`. The
/// path holds one group key per level browsed so far: `[]` lists the first
/// level's groups, and a path as long as the rule's levels lists tracks.
/// Under the folder rule the path is a library folder id followed by folder
/// names, and each node lists its subfolders, then its tracks.
///
/// Pages come from separate queries, so a scan that changes the library
/// between them can shift rows from one page to the next.
pub fn browse(
    conn: &Connection,
    rule: &SortRule,
    articles: &[String],
    path: &[Option<GroupKey>],
    offset: u32,
    limit: u32,
) -> Result<BrowsePage, Error> {
    rule.validate()?;
    let mut params = Params::new(articles);
    let node = if rule.levels == [Level::Folder] {
        folder_node(rule, path, &mut params)?
    } else {
        tag_node(rule, path, &mut params)?
    };

    let limit = limit.min(MAX_PAGE_SIZE);
    let group_total = match &node.groups {
        Some(groups) => count(conn, &params, &groups.listing)?,
        None => 0,
    };
    let track_total = match &node.tracks {
        Some(tracks) => count(conn, &params, tracks)?,
        None => 0,
    };
    let mut page = BrowsePage {
        groups: Vec::new(),
        tracks: Vec::new(),
        total: group_total + track_total,
    };
    if let Some(groups) = node.groups.as_ref().filter(|_| offset < group_total) {
        page.groups = fetch(conn, &params, &groups.listing, offset, limit, |row| {
            group_from_row(row, groups.unknown)
        })?;
    }
    let track_offset = offset.saturating_sub(group_total);
    let track_limit = limit - page.groups.len() as u32;
    if let Some(tracks) = node
        .tracks
        .as_ref()
        .filter(|_| track_limit > 0 && track_offset < track_total)
    {
        page.tracks = fetch(
            conn,
            &params,
            tracks,
            track_offset,
            track_limit,
            track_from_row,
        )?;
    }
    Ok(page)
}

/// The groups and tracks in a node, either of which may be absent.
struct Node {
    groups: Option<Groups>,
    tracks: Option<Listing>,
}

struct Groups {
    /// Selects the columns `group_from_row` reads.
    listing: Listing,
    /// The name of the group with a null key.
    unknown: &'static str,
}

/// A query for a node's groups or tracks.
struct Listing {
    /// `SELECT … FROM … WHERE … [GROUP BY …]`.
    select: String,
    /// Must end with a unique term, so pages neither repeat nor skip rows.
    order_by: String,
}

/// The bound values of a statement being assembled. `?1` is always the
/// ignored articles, for `anomp_sort_key`.
#[derive(Clone)]
struct Params(Vec<Value>);

impl Params {
    fn new(articles: &[String]) -> Params {
        Params(vec![if articles.is_empty() {
            Value::Null
        } else {
            Value::Text(articles.join("\n"))
        }])
    }

    /// Adds a value, returning its placeholder.
    fn bind(&mut self, value: impl Into<Value>) -> String {
        self.0.push(value.into());
        format!("?{}", self.0.len())
    }
}

/// Each album's year: the earliest among its tracks. Prefixed to every
/// statement; SQLite only evaluates it where `album_years` is used.
const ALBUM_YEARS: &str = "WITH album_years (album_id, year) AS (
         SELECT album_id, min(year) FROM tracks WHERE album_id IS NOT NULL GROUP BY album_id
     )";

/// The year a track is grouped under: its album's, else its own. Needs
/// `album_years` joined as `ay`.
const ALBUM_YEAR: &str = "IFNULL(ay.year, t.year)";

fn tag_node(
    rule: &SortRule,
    path: &[Option<GroupKey>],
    params: &mut Params,
) -> Result<Node, Error> {
    if path.len() > rule.levels.len() {
        return Err(Error::Invalid(format!(
            "The path has {} keys, but sort rule \"{}\" has only {} levels",
            path.len(),
            rule.id,
            rule.levels.len()
        )));
    }
    let mut filter = String::from("1");
    for (&level, key) in rule.levels.iter().zip(path) {
        filter.push_str(" AND ");
        filter.push_str(&level_filter(level, key.as_ref(), params)?);
    }
    let mut from = TRACKS_FROM.to_owned();
    if rule
        .levels
        .iter()
        .take(path.len() + 1)
        .any(|&level| level == Level::Year)
    {
        from.push_str(" LEFT JOIN album_years ay ON ay.album_id = t.album_id");
    }
    Ok(match rule.levels.get(path.len()) {
        Some(&level) => Node {
            groups: Some(level_groups(level, &from, &filter)),
            tracks: None,
        },
        None => Node {
            groups: None,
            tracks: Some(track_listing(&from, &filter, &rule.track_order)),
        },
    })
}

/// A condition for the tracks in the group `key` of `level`.
fn level_filter(
    level: Level,
    key: Option<&GroupKey>,
    params: &mut Params,
) -> Result<String, Error> {
    let value = match (level, key) {
        (_, None) => Value::Null,
        (Level::Genre, Some(GroupKey::Text(genre))) => Value::Text(genre.clone()),
        (Level::Genre, Some(GroupKey::Number(_))) | (_, Some(GroupKey::Text(_))) => {
            return Err(Error::Invalid(format!(
                "Wrong kind of key for the {level:?} level"
            )));
        }
        (_, Some(&GroupKey::Number(number))) => Value::Integer(number),
    };
    let value = params.bind(value);
    Ok(match level {
        Level::AlbumArtist => format!("t.album_artist_id IS {value}"),
        Level::Artist => format!("t.artist_id IS {value}"),
        Level::Album => format!("t.album_id IS {value}"),
        Level::Year => format!("{ALBUM_YEAR} IS {value}"),
        Level::Genre => format!("anomp_has_genre(t.genre, {value})"),
        Level::Folder => unreachable!("rules::validate keeps the folder level on its own"),
    })
}

/// The groups of `level` among the tracks `from` matching `filter`.
fn level_groups(level: Level, from: &str, filter: &str) -> Groups {
    if level == Level::Genre {
        // A row per track and genre (`genres`), grouped by exact spelling
        // first so each is folded once. Genres that differ only in case or
        // accents are one group, shown with the spelling that sorts first
        // bytewise ("Rock" over "rock"). A track can't count twice, since
        // `anomp_genres` drops such repeats.
        return Groups {
            listing: Listing {
                select: format!(
                    "SELECT min(genre), NULL, NULL, NULL, sum(tracks)
                     FROM (SELECT g.value AS genre, count(*) AS tracks
                           {from} JOIN json_each(anomp_genres(t.genre)) g
                           WHERE {filter} GROUP BY g.value)
                     GROUP BY anomp_sort_key(genre, NULL)"
                ),
                order_by: "anomp_sort_key(genre, NULL) NULLS LAST, min(genre)".into(),
            },
            unknown: "Unknown genre",
        };
    }
    // Key, name (null to use the key), album artist, year.
    let (columns, group_by, order_by, unknown) = match level {
        Level::AlbumArtist => (
            "t.album_artist_id, album_artist.name, NULL, NULL".to_owned(),
            "t.album_artist_id".to_owned(),
            "anomp_sort_key(album_artist.name, ?1) NULLS LAST, album_artist.name, t.album_artist_id".to_owned(),
            "Unknown artist",
        ),
        Level::Artist => (
            "t.artist_id, artist.name, NULL, NULL".to_owned(),
            "t.artist_id".to_owned(),
            "anomp_sort_key(artist.name, ?1) NULLS LAST, artist.name, t.artist_id".to_owned(),
            "Unknown artist",
        ),
        // An album's artist is its tracks' album artist.
        Level::Album => (
            "t.album_id, album.title, CASE WHEN t.album_id IS NOT NULL THEN album_artist.name END,
             (SELECT min(y.year) FROM tracks y WHERE y.album_id = t.album_id)"
                .to_owned(),
            "t.album_id".to_owned(),
            "anomp_sort_key(album.title, ?1) NULLS LAST, album.title,
             anomp_sort_key(album_artist.name, ?1) NULLS LAST, album_artist.name, t.album_id"
                .to_owned(),
            "Unknown album",
        ),
        Level::Year => (
            format!("{ALBUM_YEAR}, NULL, NULL, NULL"),
            ALBUM_YEAR.to_owned(),
            format!("{ALBUM_YEAR} NULLS LAST"),
            "Unknown year",
        ),
        Level::Genre => unreachable!("handled above"),
        Level::Folder => unreachable!("rules::validate keeps the folder level on its own"),
    };
    Groups {
        listing: Listing {
            select: format!("SELECT {columns}, count(*) {from} WHERE {filter} GROUP BY {group_by}"),
            order_by,
        },
        unknown,
    }
}

/// The tracks `from` matching `filter`, in `order`.
fn track_listing(from: &str, filter: &str, order: &[TrackKey]) -> Listing {
    let mut terms: Vec<&str> = order
        .iter()
        .map(|key| match key {
            TrackKey::AlbumArtist => "anomp_sort_key(album_artist.name, ?1) NULLS LAST, album_artist.name",
            TrackKey::Artist => "anomp_sort_key(artist.name, ?1) NULLS LAST, artist.name",
            TrackKey::Album => "anomp_sort_key(album.title, ?1) NULLS LAST, album.title",
            TrackKey::Year => "t.year NULLS LAST",
            TrackKey::DiscNumber => "IFNULL(t.disc_number, 1)",
            TrackKey::TrackNumber => "t.track_number NULLS LAST",
            TrackKey::Title => "anomp_sort_key(t.title, ?1) NULLS LAST, t.title",
            TrackKey::Path => {
                "anomp_sort_key(f.path, NULL), f.path, anomp_sort_key(t.relative_path, NULL), t.relative_path"
            }
        })
        .collect();
    terms.push("t.id");
    Listing {
        select: format!("SELECT {TRACK_COLUMNS} {from} WHERE {filter}"),
        order_by: terms.join(", "),
    }
}

/// A node of the folder rule: the library folders, or a folder's
/// subfolders and then its tracks.
fn folder_node(
    rule: &SortRule,
    path: &[Option<GroupKey>],
    params: &mut Params,
) -> Result<Node, Error> {
    let Some((folder, names)) = path.split_first() else {
        return Ok(Node {
            groups: Some(Groups {
                listing: Listing {
                    select: "SELECT f.id, f.path, NULL, NULL,
                                    (SELECT count(*) FROM tracks WHERE folder_id = f.id)
                             FROM folders f"
                        .into(),
                    order_by: "anomp_sort_key(f.path, NULL), f.path, f.id".into(),
                },
                unknown: "",
            }),
            tracks: None,
        });
    };
    let Some(GroupKey::Number(folder_id)) = folder else {
        return Err(Error::Invalid(
            "A folder path starts with a library folder id".into(),
        ));
    };
    let mut prefix = String::new();
    for name in names {
        match name {
            Some(GroupKey::Text(name)) if !name.is_empty() && !name.contains('/') => {
                prefix.push_str(name);
                prefix.push('/');
            }
            _ => {
                return Err(Error::Invalid(
                    "Folder names in a path must be text without '/'".into(),
                ))
            }
        }
    }

    let mut filter = format!("t.folder_id = {}", params.bind(*folder_id));
    if let Some(parent) = prefix.strip_suffix('/') {
        // The paths that start with `prefix` are an index range, since '0'
        // follows '/'.
        let end = format!("{parent}0");
        filter.push_str(&format!(
            " AND t.relative_path >= {} AND t.relative_path < {}",
            params.bind(prefix.clone()),
            params.bind(end)
        ));
    }
    // The path below this folder (`substr` counts characters, not bytes).
    let rest = format!(
        "substr(t.relative_path, {})",
        params.bind(prefix.chars().count() as i64 + 1)
    );
    let subfolder = format!("substr({rest}, 1, instr({rest}, '/') - 1)");
    Ok(Node {
        groups: Some(Groups {
            listing: Listing {
                select: format!(
                    "SELECT {subfolder}, NULL, NULL, NULL, count(*) FROM tracks t
                     WHERE {filter} AND instr({rest}, '/') > 0 GROUP BY {subfolder}"
                ),
                order_by: format!("anomp_sort_key({subfolder}, NULL), {subfolder}"),
            },
            unknown: "",
        }),
        tracks: Some(track_listing(
            TRACKS_FROM,
            &format!("{filter} AND instr({rest}, '/') = 0"),
            &rule.track_order,
        )),
    })
}

fn group_from_row(row: &Row, unknown: &str) -> rusqlite::Result<Group> {
    let key = match row.get_ref(0)? {
        ValueRef::Integer(number) => Some(GroupKey::Number(number)),
        ValueRef::Text(text) => Some(GroupKey::Text(String::from_utf8_lossy(text).into_owned())),
        _ => None,
    };
    let name = row.get::<_, Option<String>>(1)?.or_else(|| match &key {
        Some(GroupKey::Number(number)) => Some(number.to_string()),
        Some(GroupKey::Text(text)) => Some(text.clone()),
        None => None,
    });
    Ok(Group {
        key,
        name: name.unwrap_or_else(|| unknown.to_owned()),
        album_artist: row.get(2)?,
        year: row.get(3)?,
        track_count: row.get(4)?,
    })
}

fn count(conn: &Connection, params: &Params, listing: &Listing) -> Result<u32, Error> {
    let sql = format!("{ALBUM_YEARS} SELECT count(*) FROM ({})", listing.select);
    let counts = query(conn, &sql, params, |row| row.get(0))?;
    Ok(counts.into_iter().next().unwrap_or(0))
}

fn fetch<T>(
    conn: &Connection,
    params: &Params,
    listing: &Listing,
    offset: u32,
    limit: u32,
    map: impl FnMut(&Row) -> rusqlite::Result<T>,
) -> Result<Vec<T>, Error> {
    let mut params = params.clone();
    let sql = format!(
        "{ALBUM_YEARS} {} ORDER BY {} LIMIT {} OFFSET {}",
        listing.select,
        listing.order_by,
        params.bind(limit),
        params.bind(offset)
    );
    query(conn, &sql, &params, map)
}

fn query<T>(
    conn: &Connection,
    sql: &str,
    params: &Params,
    mut map: impl FnMut(&Row) -> rusqlite::Result<T>,
) -> Result<Vec<T>, Error> {
    let mut statement = conn.prepare(sql)?;
    // Bound by index, since a statement may not use every parameter
    // (e.g. `?1` in a count).
    for index in 1..=statement.parameter_count() {
        let value = params
            .0
            .get(index - 1)
            .ok_or(rusqlite::Error::InvalidParameterCount(
                params.0.len(),
                index,
            ))?;
        statement.raw_bind_parameter(index, value)?;
    }
    let mut rows = statement.raw_query();
    let mut result = Vec::new();
    while let Some(row) = rows.next()? {
        result.push(map(row)?);
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;
    use std::path::Path;

    use super::*;
    use crate::library::db;
    use crate::library::rules::{default_rules, set_ignored_articles};

    /// A track to add to a test library; unset tags are missing.
    #[derive(Default)]
    struct Track {
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

    fn track(path: &str) -> Track {
        Track {
            path: path.into(),
            ..Track::default()
        }
    }

    impl Track {
        fn title(self, title: &str) -> Self {
            Track {
                title: Some(title.into()),
                ..self
            }
        }
        fn artist(self, artist: &str) -> Self {
            Track {
                artist: Some(artist.into()),
                ..self
            }
        }
        fn album_artist(self, album_artist: &str) -> Self {
            Track {
                album_artist: Some(album_artist.into()),
                ..self
            }
        }
        fn album(self, album: &str) -> Self {
            Track {
                album: Some(album.into()),
                ..self
            }
        }
        fn genre(self, genre: &str) -> Self {
            Track {
                genre: Some(genre.into()),
                ..self
            }
        }
        fn year(self, year: u32) -> Self {
            Track {
                year: Some(year),
                ..self
            }
        }
        fn disc(self, disc: u32) -> Self {
            Track {
                disc: Some(disc),
                ..self
            }
        }
        fn number(self, number: u32) -> Self {
            Track {
                number: Some(number),
                ..self
            }
        }
    }

    /// An in-memory library filled like the scanner fills it: the album
    /// artist falls back to the artist, and an album is a title by an album
    /// artist.
    struct Library {
        conn: Connection,
        folder_id: i64,
    }

    impl Library {
        fn new(tracks: impl IntoIterator<Item = Track>) -> Library {
            let conn = db::open_in_memory().unwrap();
            let mut library = Library { conn, folder_id: 0 };
            library.folder_id = library.add_folder("/Music");
            for track in tracks {
                library.add(library.folder_id, track);
            }
            library
        }

        fn add_folder(&self, path: &str) -> i64 {
            self.conn
                .execute(
                    "INSERT INTO folders (path, added_at) VALUES (?1, 0)",
                    [path],
                )
                .unwrap();
            self.conn.last_insert_rowid()
        }

        fn artist(&self, name: Option<&str>) -> Option<i64> {
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

        fn add(&self, folder_id: i64, track: Track) {
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

        fn page(&self, rule: &SortRule, path: &[Option<GroupKey>]) -> BrowsePage {
            self.page_with(rule, &["The".into(), "A".into()], path)
        }

        fn page_with(
            &self,
            rule: &SortRule,
            articles: &[String],
            path: &[Option<GroupKey>],
        ) -> BrowsePage {
            let page = browse(&self.conn, rule, articles, path, 0, MAX_PAGE_SIZE).unwrap();
            assert_eq!(page.total as usize, page.groups.len() + page.tracks.len());
            page
        }

        fn groups(&self, rule: &SortRule, path: &[Option<GroupKey>]) -> Vec<Group> {
            let page = self.page(rule, path);
            assert!(page.tracks.is_empty());
            page.groups
        }

        /// The groups' names, with their track counts.
        fn names(&self, rule: &SortRule, path: &[Option<GroupKey>]) -> Vec<(String, u32)> {
            self.groups(rule, path)
                .into_iter()
                .map(|group| (group.name, group.track_count))
                .collect()
        }

        /// The key of the group called `name`.
        fn key(&self, rule: &SortRule, path: &[Option<GroupKey>], name: &str) -> Option<GroupKey> {
            let groups = self.groups(rule, path);
            let group = groups.iter().find(|group| group.name == name);
            group
                .unwrap_or_else(|| panic!("no group {name} in {groups:?}"))
                .key
                .clone()
        }

        fn titles(&self, rule: &SortRule, path: &[Option<GroupKey>]) -> Vec<String> {
            titles(&self.page(rule, path).tracks)
        }
    }

    /// Titles, or file names for tracks without one.
    fn titles(tracks: &[TrackSummary]) -> Vec<String> {
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

    fn rule(id: &str) -> SortRule {
        default_rules()
            .into_iter()
            .find(|rule| rule.id == id)
            .unwrap()
    }

    fn songs_rule(order: &[TrackKey]) -> SortRule {
        SortRule {
            id: "songs".into(),
            name: "Songs".into(),
            levels: Vec::new(),
            track_order: order.to_vec(),
        }
    }

    fn named(names: &[(&str, u32)]) -> Vec<(String, u32)> {
        names
            .iter()
            .map(|&(name, count)| (name.to_owned(), count))
            .collect()
    }

    fn text(key: &str) -> Option<GroupKey> {
        Some(GroupKey::Text(key.into()))
    }

    #[test]
    fn browses_album_artist_album_and_tracks_by_disc_and_number() {
        let album = |path: &str| track(path).artist("Band").album("Album");
        let library = Library::new([
            album("a/1.flac").title("Second").disc(1).number(2),
            album("a/2.flac").title("Encore").disc(2).number(1),
            album("a/3.flac").title("Middle").disc(1).number(10),
            // No disc number counts as disc 1.
            album("a/4.flac").title("Opening").number(1),
            album("a/5.flac").title("Bonus").disc(2),
            album("a/6.flac").title("Hidden").disc(1),
            track("b/1.flac")
                .title("Guest spot")
                .artist("Singer")
                .album_artist("Band")
                .album("Album")
                .disc(2)
                .number(2),
        ]);
        let rule = rule("album-artist");

        assert_eq!(library.names(&rule, &[]), named(&[("Band", 7)]));
        let band = library.key(&rule, &[], "Band");
        let albums = library.groups(&rule, std::slice::from_ref(&band));
        assert_eq!(albums.len(), 1);
        assert_eq!(
            (albums[0].name.as_str(), albums[0].track_count),
            ("Album", 7)
        );
        assert_eq!(albums[0].album_artist.as_deref(), Some("Band"));

        let page = library.page(&rule, &[band, albums[0].key.clone()]);
        assert!(page.groups.is_empty());
        assert_eq!(
            titles(&page.tracks),
            [
                "Opening",
                "Second",
                "Middle",
                "Hidden",
                "Encore",
                "Guest spot",
                "Bonus"
            ]
        );
        assert_eq!(page.tracks[5].artist.as_deref(), Some("Singer"));
        assert_eq!(page.tracks[5].album_artist.as_deref(), Some("Band"));
    }

    #[test]
    fn missing_values_sort_last() {
        let library = Library::new([
            track("z.flac")
                .title("Zebra")
                .artist("Zed")
                .album("Zulu")
                .genre("Zouk")
                .year(2000),
            track("untitled 2.flac"),
            track("untitled 10.flac"),
            track("a.flac")
                .title("Aardvark")
                .artist("Abe")
                .album("Alpha")
                .genre("Ambient")
                .year(1990),
            track("b.flac").title("Bee").artist("Abe"),
        ]);

        let by_artist = rule("album-artist");
        assert_eq!(
            library.names(&by_artist, &[]),
            named(&[("Abe", 2), ("Zed", 1), ("Unknown artist", 2)])
        );
        assert_eq!(library.key(&by_artist, &[], "Unknown artist"), None);
        let abe = library.key(&by_artist, &[], "Abe");
        assert_eq!(
            library.names(&by_artist, &[abe]),
            named(&[("Alpha", 1), ("Unknown album", 1)])
        );
        assert_eq!(
            library.names(&by_artist, &[None]),
            named(&[("Unknown album", 2)])
        );
        assert_eq!(
            library.titles(&by_artist, &[None, None]),
            ["untitled 2.flac", "untitled 10.flac"]
        );

        assert_eq!(
            library.names(&rule("genre"), &[]),
            named(&[("Ambient", 1), ("Zouk", 1), ("Unknown genre", 3)])
        );
        assert_eq!(
            library.names(&rule("year"), &[]),
            named(&[("1990", 1), ("2000", 1), ("Unknown year", 3)])
        );
        let songs = songs_rule(&[TrackKey::Title, TrackKey::Path]);
        assert_eq!(
            library.titles(&songs, &[]),
            [
                "Aardvark",
                "Bee",
                "Zebra",
                "untitled 2.flac",
                "untitled 10.flac"
            ]
        );
        let by_year = songs_rule(&[TrackKey::Year, TrackKey::Title]);
        assert_eq!(library.titles(&by_year, &[])[..2], ["Aardvark", "Zebra"]);
    }

    #[test]
    fn sorts_ignoring_case_and_accents() {
        let library = Library::new(
            ["Zoë", "élodie", "Eve", "Émile", "abba", "Ólafur"]
                .into_iter()
                .enumerate()
                .map(|(index, artist)| track(&format!("{index}.flac")).artist(artist).title(artist))
                .chain([
                    track("g1.flac").genre("Électro"),
                    track("g2.flac").genre("electro"),
                    track("g3.flac").genre("Rock"),
                ]),
        );
        assert_eq!(
            library.names(&rule("album-artist"), &[]),
            named(&[
                ("abba", 1),
                ("élodie", 1),
                ("Émile", 1),
                ("Eve", 1),
                ("Ólafur", 1),
                ("Zoë", 1),
                ("Unknown artist", 3)
            ])
        );
        assert_eq!(
            library.titles(&songs_rule(&[TrackKey::Title]), &[])[..6],
            ["abba", "élodie", "Émile", "Eve", "Ólafur", "Zoë"]
        );
        // Genres that differ only in case or accents are one group.
        let genre = rule("genre");
        assert_eq!(
            library.names(&genre, &[]),
            named(&[("electro", 2), ("Rock", 1), ("Unknown genre", 6)])
        );
        assert_eq!(
            library.names(&genre, &[text("ÉLECTRO")]),
            named(&[("Unknown artist", 2)])
        );
    }

    #[test]
    fn ignores_leading_articles_when_sorting() {
        let library = Library::new(
            [
                "The Beatles",
                "Beach Boys",
                "A Tribe Called Quest",
                "Abba",
                "The The",
            ]
            .into_iter()
            .map(|artist| track(&format!("{artist}.flac")).artist(artist))
            .chain([
                track("w.flac")
                    .artist("Beach Boys")
                    .album("The White Album"),
                track("r.flac").artist("Beach Boys").album("Abbey Road"),
                track("l.flac").artist("Beach Boys").album("Let It Be"),
            ]),
        );
        let rule = rule("album-artist");
        let names = |articles: &[&str]| -> Vec<String> {
            let articles: Vec<String> =
                articles.iter().map(|article| article.to_string()).collect();
            let page = library.page_with(&rule, &articles, &[]);
            page.groups.into_iter().map(|group| group.name).collect()
        };
        assert_eq!(
            names(&["The", "A"]),
            [
                "Abba",
                "Beach Boys",
                "The Beatles",
                "The The",
                "A Tribe Called Quest"
            ]
        );
        assert_eq!(
            names(&[]),
            [
                "A Tribe Called Quest",
                "Abba",
                "Beach Boys",
                "The Beatles",
                "The The"
            ]
        );
        let beach_boys = library.key(&rule, &[], "Beach Boys");
        assert_eq!(
            library.names(&rule, &[beach_boys]),
            named(&[
                ("Abbey Road", 1),
                ("Let It Be", 1),
                ("The White Album", 1),
                ("Unknown album", 1)
            ])
        );

        // `browse_rule` takes the articles from the settings.
        let first = |conn: &Connection| {
            browse_rule(conn, "album-artist", &[], 0, 1).unwrap().groups[0]
                .name
                .clone()
        };
        assert_eq!(first(&library.conn), "Abba");
        set_ignored_articles(&library.conn, Vec::new()).unwrap();
        assert_eq!(first(&library.conn), "A Tribe Called Quest");
    }

    #[test]
    fn orders_numbers_naturally() {
        let library = Library::new([
            track("Disc 10/1.flac"),
            track("Disc 2/1.flac"),
            track("Track 10.flac"),
            track("track 2.flac"),
            track("Track 1.flac"),
            track("v10.flac").artist("Band").album("Vol. 10"),
            track("v9.flac").artist("Band").album("Vol. 9"),
        ]);
        let folder = rule("folder");
        let music = Some(GroupKey::Number(library.folder_id));
        let page = library.page(&folder, &[music]);
        let groups: Vec<_> = page
            .groups
            .iter()
            .map(|group| group.name.as_str())
            .collect();
        assert_eq!(groups, ["Disc 2", "Disc 10"]);
        assert_eq!(
            titles(&page.tracks),
            [
                "Track 1.flac",
                "track 2.flac",
                "Track 10.flac",
                "v9.flac",
                "v10.flac"
            ]
        );

        let by_artist = rule("album-artist");
        let band = library.key(&by_artist, &[], "Band");
        assert_eq!(
            library.names(&by_artist, &[band]),
            named(&[("Vol. 9", 1), ("Vol. 10", 1)])
        );
    }

    #[test]
    fn browses_the_folder_tree() {
        let library = Library::new(
            [
                "Rock/Band/Album/02.flac",
                "Rock/Band/Album/01.flac",
                "Rock/Band/single.flac",
                "Rock/loose.flac",
                "Rocks/x.flac",
                "Rock (live)/y.flac",
                "東京/曲.flac",
                "top.flac",
            ]
            .map(track),
        );
        let other = library.add_folder("/Other");
        library.add(other, track("elsewhere.flac"));
        library.add_folder("/Empty");
        let rule = rule("folder");

        let roots = library.groups(&rule, &[]);
        let roots: Vec<_> = roots
            .iter()
            .map(|g| (g.name.as_str(), g.track_count, g.key.clone()))
            .collect();
        assert_eq!(
            roots,
            [
                ("/Empty", 0, Some(GroupKey::Number(3))),
                ("/Music", 8, Some(GroupKey::Number(library.folder_id))),
                ("/Other", 1, Some(GroupKey::Number(other)))
            ]
        );

        let music = Some(GroupKey::Number(library.folder_id));
        let at = |names: &[&str]| -> (Vec<(String, u32)>, Vec<String>) {
            let mut path = vec![music.clone()];
            path.extend(names.iter().map(|name| text(name)));
            let page = library.page(&rule, &path);
            let groups = page.groups.into_iter().map(|group| {
                assert_eq!(group.key, text(&group.name));
                (group.name, group.track_count)
            });
            (groups.collect(), titles(&page.tracks))
        };
        assert_eq!(
            at(&[]),
            (
                named(&[("Rock", 4), ("Rock (live)", 1), ("Rocks", 1), ("東京", 1)]),
                vec!["top.flac".into()]
            )
        );
        assert_eq!(
            at(&["Rock"]),
            (named(&[("Band", 3)]), vec!["loose.flac".into()])
        );
        assert_eq!(
            at(&["Rock", "Band"]),
            (named(&[("Album", 2)]), vec!["single.flac".into()])
        );
        assert_eq!(
            at(&["Rock", "Band", "Album"]),
            (vec![], vec!["01.flac".into(), "02.flac".into()])
        );
        // `substr` counts characters, so non-ASCII names work.
        assert_eq!(at(&["東京"]), (vec![], vec!["曲.flac".into()]));
        assert_eq!(at(&["Missing"]), (vec![], vec![]));
        let page = library.page(&rule, &[Some(GroupKey::Number(other))]);
        assert_eq!(titles(&page.tracks), ["elsewhere.flac"]);
    }

    #[test]
    fn lists_tracks_under_each_of_their_genres() {
        let library = Library::new([
            track("1.flac").artist("A").genre("Rock; Pop"),
            track("2.flac").artist("B").genre("rock"),
            track("3.flac").artist("C").genre("Jazz"),
            track("4.flac").artist("D"),
            track("5.flac").artist("E").genre("; "),
            track("6.flac").artist("F").genre("Pop; pop; Rock ;"),
        ]);
        let rule = rule("genre");
        assert_eq!(
            library.names(&rule, &[]),
            named(&[("Jazz", 1), ("Pop", 2), ("Rock", 3), ("Unknown genre", 2)])
        );
        let artists = |genre: Option<GroupKey>| -> Vec<String> {
            library
                .groups(&rule, &[genre])
                .into_iter()
                .map(|group| group.name)
                .collect()
        };
        assert_eq!(artists(text("Rock")), ["A", "B", "F"]);
        assert_eq!(artists(text("rock")), ["A", "B", "F"]);
        assert_eq!(artists(text("Pop")), ["A", "F"]);
        assert_eq!(artists(None), ["D", "E"]);
        assert_eq!(artists(text("Polka")), Vec::<String>::new());

        // A track tagged with a genre twice is listed once.
        let f = library.key(&rule, &[text("Pop")], "F");
        let page = library.page(&rule, &[text("Pop"), f, None]);
        assert_eq!(titles(&page.tracks), ["6.flac"]);
    }

    #[test]
    fn groups_years_by_the_albums_earliest_year() {
        let library = Library::new([
            track("r1.flac").artist("A").album("Reissue").year(2001),
            track("r2.flac").artist("A").album("Reissue").year(1997),
            track("r3.flac").artist("A").album("Reissue"),
            track("later.flac").artist("B").album("Later").year(2003),
            track("loose.flac").artist("C").year(1999),
            track("undated.flac").artist("D").album("Undated"),
        ]);
        let rule = rule("year");
        assert_eq!(
            library.names(&rule, &[]),
            named(&[("1997", 3), ("1999", 1), ("2003", 1), ("Unknown year", 1)])
        );
        assert_eq!(
            library.key(&rule, &[], "1997"),
            Some(GroupKey::Number(1997))
        );

        let albums = library.groups(&rule, &[Some(GroupKey::Number(1997))]);
        assert_eq!(albums.len(), 1);
        assert_eq!(
            (
                albums[0].name.as_str(),
                albums[0].track_count,
                albums[0].year,
                albums[0].album_artist.as_deref()
            ),
            ("Reissue", 3, Some(1997), Some("A"))
        );
        let reissue = albums[0].key.clone();
        assert_eq!(
            library.titles(&rule, &[Some(GroupKey::Number(1997)), reissue]),
            ["r1.flac", "r2.flac", "r3.flac"]
        );
        assert_eq!(
            library.names(&rule, &[Some(GroupKey::Number(1999))]),
            named(&[("Unknown album", 1)])
        );
        let undated = library.groups(&rule, &[None]);
        assert_eq!(
            (undated[0].name.as_str(), undated[0].year),
            ("Undated", None)
        );
    }

    /// Every item of a node, read `limit` at a time: (group names, track ids).
    fn read_in_pages(
        library: &Library,
        rule: &SortRule,
        path: &[Option<GroupKey>],
        limit: u32,
    ) -> (Vec<String>, Vec<i64>) {
        let articles = ["The".to_owned(), "A".to_owned()];
        let (mut groups, mut tracks) = (Vec::new(), Vec::new());
        let mut offset = 0;
        loop {
            let page = browse(&library.conn, rule, &articles, path, offset, limit).unwrap();
            let items = page.groups.len() + page.tracks.len();
            assert!(items <= limit as usize);
            groups.extend(page.groups.into_iter().map(|group| group.name));
            tracks.extend(page.tracks.into_iter().map(|track| track.id));
            offset += limit;
            if offset >= page.total {
                assert_eq!(groups.len() + tracks.len(), page.total as usize);
                return (groups, tracks);
            }
            assert_eq!(items, limit as usize, "only the last page is short");
        }
    }

    #[test]
    fn pages_neither_repeat_nor_skip() {
        // Many ties (same artist and title), so order rests on the tie-breaks.
        let library = Library::new((0..45).map(|index| {
            let track = track(&format!(
                "{}/{index}.flac",
                ["Rock", "Jazz", "Pop"][index % 3]
            ));
            match index % 4 {
                0 => track,
                1 => track.artist("Same").title("Same"),
                2 => track.artist(&format!("Artist {}", index % 7)).title("Same"),
                _ => track.artist(&format!("artist {}", index % 5)),
            }
        }));
        library.add(library.folder_id, track("top 1.flac"));
        library.add(library.folder_id, track("top 2.flac"));

        let songs = songs_rule(&[TrackKey::Artist, TrackKey::Title]);
        let (_, all) = read_in_pages(&library, &songs, &[], MAX_PAGE_SIZE);
        assert_eq!(all.len(), 47);
        assert_eq!(all.iter().collect::<HashSet<_>>().len(), 47, "no repeats");
        for limit in [1, 7, 46, 47, 48] {
            assert_eq!(
                read_in_pages(&library, &songs, &[], limit).1,
                all,
                "limit {limit}"
            );
        }

        let by_artist = rule("album-artist");
        let (artists, _) = read_in_pages(&library, &by_artist, &[], MAX_PAGE_SIZE);
        assert_eq!(read_in_pages(&library, &by_artist, &[], 3).0, artists);

        // A folder's pages run on from its subfolders into its tracks.
        let music = [Some(GroupKey::Number(library.folder_id))];
        let everything = read_in_pages(&library, &rule("folder"), &music, MAX_PAGE_SIZE);
        assert_eq!(everything.0, ["Jazz", "Pop", "Rock"]);
        assert_eq!(everything.1.len(), 2);
        for limit in [1, 2, 3, 4] {
            assert_eq!(
                read_in_pages(&library, &rule("folder"), &music, limit),
                everything
            );
        }

        let past_the_end = browse(&library.conn, &songs, &[], &[], 100, 10).unwrap();
        assert_eq!((past_the_end.tracks.len(), past_the_end.total), (0, 47));
        let capped = browse(&library.conn, &songs, &[], &[], 0, u32::MAX).unwrap();
        assert_eq!(capped.tracks.len(), 47);
    }

    #[test]
    fn refuses_bad_paths_and_rules() {
        let library = Library::new([track("a.flac").artist("A").genre("Rock")]);
        let error = |rule: &SortRule, path: &[Option<GroupKey>]| {
            browse(&library.conn, rule, &[], path, 0, 10)
                .unwrap_err()
                .to_string()
        };
        let number = Some(GroupKey::Number(1));
        let by_artist = rule("album-artist");
        assert!(error(&by_artist, &[number.clone(), None, None]).contains("only 2 levels"));
        assert!(error(&by_artist, &[text("A")]).contains("Wrong kind of key"));
        assert!(error(&rule("genre"), std::slice::from_ref(&number)).contains("Wrong kind of key"));
        let folder = rule("folder");
        assert!(error(&folder, &[text("Music")]).contains("library folder id"));
        assert!(error(&folder, &[number.clone(), number.clone()]).contains("without '/'"));
        assert!(error(&folder, &[number.clone(), text("a/b")]).contains("without '/'"));
        assert!(error(&folder, &[number.clone(), None]).contains("without '/'"));
        let mut mixed = rule("album-artist");
        mixed.levels.push(Level::Folder);
        assert!(error(&mixed, &[]).contains("folder level"));

        let unknown = browse_rule(&library.conn, "nope", &[], 0, 10).unwrap_err();
        assert!(unknown.to_string().contains("No sort rule"), "{unknown}");
        // A key that matches nothing is just an empty node.
        let page = browse(
            &library.conn,
            &by_artist,
            &[],
            &[Some(GroupKey::Number(999))],
            0,
            10,
        )
        .unwrap();
        assert_eq!(page.total, 0);
    }
}
