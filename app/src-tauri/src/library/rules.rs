//! Sort and grouping rules: how the library is browsed (`browse`). A rule is
//! a list of grouping levels, e.g. album artist → album, and the order of
//! the tracks within the last one. The rules and the leading articles that
//! sorting ignores are stored as JSON in `settings`, falling back to the
//! built-in defaults.

use std::collections::HashSet;

use rusqlite::{Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::Error;

const SETTINGS_KEY: &str = "library.sort";

/// What tracks are grouped by at one level of a rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Level {
    AlbumArtist,
    Artist,
    Album,
    /// One group per genre; a track tagged "Rock; Pop" is in both.
    Genre,
    /// The album's year (the earliest among its tracks), or a track's own
    /// year if it has no album.
    Year,
    /// The folder tree, starting from the library folders. Only valid as a
    /// rule's only level.
    Folder,
}

/// What tracks are sorted by within a group, in order of precedence. Ties
/// left after all of them fall back on the track id.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TrackKey {
    AlbumArtist,
    Artist,
    Album,
    Year,
    /// A missing disc number counts as disc 1.
    DiscNumber,
    TrackNumber,
    Title,
    /// The library folder, then the path within it.
    Path,
}

/// How albums are ordered where a rule lists them.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AlbumOrder {
    /// By title (ignoring leading articles), then album artist.
    #[default]
    Title,
    /// By the album's year (the earliest among its tracks), oldest first;
    /// albums without one last. Ties by title.
    Year,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SortRule {
    /// Unique among the rules.
    pub id: String,
    pub name: String,
    pub levels: Vec<Level>,
    pub track_order: Vec<TrackKey>,
    /// Missing in rules saved before it existed.
    #[serde(default)]
    pub album_order: AlbumOrder,
}

impl SortRule {
    fn new(id: &str, name: &str, levels: &[Level], track_order: &[TrackKey]) -> SortRule {
        SortRule {
            id: id.into(),
            name: name.into(),
            levels: levels.to_vec(),
            track_order: track_order.to_vec(),
            album_order: AlbumOrder::Title,
        }
    }

    pub fn validate(&self) -> Result<(), Error> {
        let invalid = |message: &str| {
            Err(Error::Invalid(format!(
                "Sort rule \"{}\": {message}",
                self.id
            )))
        };
        if self.id.trim().is_empty() || self.id.len() > 64 {
            return invalid("the id must be 1 to 64 bytes");
        }
        if self.name.trim().is_empty() {
            return invalid("the name is empty");
        }
        if self.levels.contains(&Level::Folder) && self.levels.len() > 1 {
            return invalid("the folder level can't be combined with others");
        }
        if !all_different(&self.levels) {
            return invalid("a level is repeated");
        }
        if !all_different(&self.track_order) {
            return invalid("a track sort key is repeated");
        }
        Ok(())
    }
}

fn all_different<T: Eq + std::hash::Hash>(items: &[T]) -> bool {
    let mut seen = HashSet::new();
    items.iter().all(|item| seen.insert(item))
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SortSettings {
    /// Never empty.
    pub rules: Vec<SortRule>,
    /// Words skipped at the start of artist names and album and track titles
    /// when sorting (not when displaying), compared ignoring case and
    /// accents. Empty turns this off.
    pub ignored_articles: Vec<String>,
}

impl Default for SortSettings {
    fn default() -> Self {
        SortSettings {
            rules: default_rules(),
            ignored_articles: default_articles(),
        }
    }
}

pub fn default_rules() -> Vec<SortRule> {
    use Level::*;
    use TrackKey::{DiscNumber, Path, Title, TrackNumber};
    let album_order = [DiscNumber, TrackNumber, Title, Path];
    vec![
        SortRule::new(
            "album-artist",
            "Album artist",
            &[AlbumArtist, Album],
            &album_order,
        ),
        SortRule::new("genre", "Genre", &[Genre, AlbumArtist, Album], &album_order),
        SortRule::new("year", "Year", &[Year, Album], &album_order),
        SortRule::new("folder", "Folder", &[Folder], &[Path]),
    ]
}

fn default_articles() -> Vec<String> {
    vec!["The".into(), "A".into()]
}

impl SortSettings {
    /// Reads stored settings, keeping what is usable: a rule that doesn't
    /// parse or validate (e.g. one saved by a newer version with a level this
    /// one doesn't know) is dropped, as is a repeated id, and anything else
    /// missing or invalid falls back to the defaults.
    fn from_json(json: &str) -> SortSettings {
        let Ok(Value::Object(mut stored)) = serde_json::from_str(json) else {
            return SortSettings::default();
        };
        let mut ids = HashSet::new();
        let rules: Vec<SortRule> = match stored.remove("rules") {
            Some(Value::Array(rules)) => rules
                .into_iter()
                .filter_map(|rule| serde_json::from_value::<SortRule>(rule).ok())
                .filter(|rule| rule.validate().is_ok() && ids.insert(rule.id.clone()))
                .collect(),
            _ => Vec::new(),
        };
        let ignored_articles = stored
            .remove("ignoredArticles")
            .and_then(|articles| serde_json::from_value::<Vec<String>>(articles).ok())
            .and_then(|articles| clean_articles(articles).ok())
            .unwrap_or_else(default_articles);
        SortSettings {
            rules: if rules.is_empty() {
                default_rules()
            } else {
                rules
            },
            ignored_articles,
        }
    }
}

/// Trims each article and checks that it's usable.
fn clean_articles(articles: Vec<String>) -> Result<Vec<String>, Error> {
    if articles.len() > 20 {
        return Err(Error::Invalid("At most 20 articles can be ignored".into()));
    }
    articles
        .into_iter()
        .map(|article| {
            let article = article.trim();
            if article.is_empty() || article.len() > 32 || article.contains(char::is_whitespace) {
                Err(Error::Invalid(format!("Not a single word: \"{article}\"")))
            } else {
                Ok(article.to_owned())
            }
        })
        .collect()
}

pub fn sort_settings(conn: &Connection) -> Result<SortSettings, Error> {
    let stored: Option<String> = conn
        .query_row(
            "SELECT value FROM settings WHERE key = ?1",
            [SETTINGS_KEY],
            |row| row.get(0),
        )
        .optional()?;
    Ok(stored.map_or_else(SortSettings::default, |json| SortSettings::from_json(&json)))
}

fn store(conn: &Connection, settings: &SortSettings) -> Result<(), Error> {
    let json = serde_json::to_string(settings).expect("sort settings serialize to JSON");
    conn.execute(
        "INSERT INTO settings (key, value) VALUES (?1, ?2)
         ON CONFLICT (key) DO UPDATE SET value = excluded.value",
        [SETTINGS_KEY, &json],
    )?;
    Ok(())
}

/// Adds `rule`, or replaces the rule with its id.
pub fn save_sort_rule(conn: &Connection, mut rule: SortRule) -> Result<SortSettings, Error> {
    rule.id = rule.id.trim().to_owned();
    rule.name = rule.name.trim().to_owned();
    rule.validate()?;
    let mut settings = sort_settings(conn)?;
    match settings
        .rules
        .iter_mut()
        .find(|existing| existing.id == rule.id)
    {
        Some(existing) => *existing = rule,
        None => settings.rules.push(rule),
    }
    store(conn, &settings)?;
    Ok(settings)
}

/// Removes a rule; the last one can't be removed.
pub fn remove_sort_rule(conn: &Connection, id: &str) -> Result<SortSettings, Error> {
    let mut settings = sort_settings(conn)?;
    let count = settings.rules.len();
    settings.rules.retain(|rule| rule.id != id);
    if settings.rules.len() == count {
        return Err(Error::Invalid(format!("No sort rule with id \"{id}\"")));
    }
    if settings.rules.is_empty() {
        return Err(Error::Invalid("The last sort rule can't be removed".into()));
    }
    store(conn, &settings)?;
    Ok(settings)
}

pub fn set_ignored_articles(
    conn: &Connection,
    articles: Vec<String>,
) -> Result<SortSettings, Error> {
    let mut settings = sort_settings(conn)?;
    settings.ignored_articles = clean_articles(articles)?;
    store(conn, &settings)?;
    Ok(settings)
}

/// Back to the built-in rules and articles.
pub fn reset_sort_settings(conn: &Connection) -> Result<SortSettings, Error> {
    conn.execute("DELETE FROM settings WHERE key = ?1", [SETTINGS_KEY])?;
    Ok(SortSettings::default())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::library::db;

    fn store_json(conn: &Connection, json: &str) {
        conn.execute(
            "INSERT OR REPLACE INTO settings (key, value) VALUES (?1, ?2)",
            [SETTINGS_KEY, json],
        )
        .unwrap();
    }

    fn ids(settings: &SortSettings) -> Vec<&str> {
        settings.rules.iter().map(|rule| rule.id.as_str()).collect()
    }

    #[test]
    fn album_order_defaults_to_title_and_is_saved() {
        let conn = db::open_in_memory().unwrap();
        // A rule saved before album orders existed.
        store_json(
            &conn,
            r#"{"rules": [{"id": "a", "name": "A", "levels": ["albumArtist", "album"],
                           "trackOrder": ["trackNumber"]}]}"#,
        );
        let settings = sort_settings(&conn).unwrap();
        assert_eq!(settings.rules[0].album_order, AlbumOrder::Title);

        let mut rule = settings.rules[0].clone();
        rule.album_order = AlbumOrder::Year;
        let saved = save_sort_rule(&conn, rule).unwrap();
        assert_eq!(saved.rules[0].album_order, AlbumOrder::Year);
        assert_eq!(
            sort_settings(&conn).unwrap().rules[0].album_order,
            AlbumOrder::Year
        );
    }

    #[test]
    fn defaults_until_something_is_saved() {
        let conn = db::open_in_memory().unwrap();
        let settings = sort_settings(&conn).unwrap();
        assert_eq!(settings, SortSettings::default());
        assert_eq!(ids(&settings), ["album-artist", "genre", "year", "folder"]);
        assert_eq!(settings.ignored_articles, ["The", "A"]);
        for rule in &settings.rules {
            rule.validate().unwrap();
        }
    }

    #[test]
    fn saves_removes_and_resets() {
        let conn = db::open_in_memory().unwrap();
        let songs = SortRule::new(" songs ", "Songs ", &[], &[TrackKey::Title]);
        let settings = save_sort_rule(&conn, songs).unwrap();
        assert_eq!(
            ids(&settings),
            ["album-artist", "genre", "year", "folder", "songs"]
        );
        assert_eq!(settings.rules[4].name, "Songs");
        assert_eq!(sort_settings(&conn).unwrap(), settings);

        // The same id replaces the rule in place.
        let by_artist = SortRule::new(
            "genre",
            "Genre",
            &[Level::Genre, Level::Artist],
            &[TrackKey::Title],
        );
        let settings = save_sort_rule(&conn, by_artist.clone()).unwrap();
        assert_eq!(settings.rules[1], by_artist);
        assert_eq!(settings.rules.len(), 5);

        let settings = set_ignored_articles(&conn, vec![" Die ".into(), "Les".into()]).unwrap();
        assert_eq!(settings.ignored_articles, ["Die", "Les"]);
        let settings = remove_sort_rule(&conn, "year").unwrap();
        assert_eq!(ids(&settings), ["album-artist", "genre", "folder", "songs"]);
        assert_eq!(sort_settings(&conn).unwrap(), settings);
        assert!(remove_sort_rule(&conn, "year").is_err());

        assert_eq!(reset_sort_settings(&conn).unwrap(), SortSettings::default());
        assert_eq!(sort_settings(&conn).unwrap(), SortSettings::default());
    }

    #[test]
    fn keeps_at_least_one_rule() {
        let conn = db::open_in_memory().unwrap();
        for id in ["album-artist", "genre", "year"] {
            remove_sort_rule(&conn, id).unwrap();
        }
        let error = remove_sort_rule(&conn, "folder").unwrap_err().to_string();
        assert!(error.contains("last sort rule"), "{error}");
        assert_eq!(ids(&sort_settings(&conn).unwrap()), ["folder"]);
    }

    #[test]
    fn refuses_invalid_rules_and_articles() {
        let conn = db::open_in_memory().unwrap();
        let rule = |id: &str, name: &str, levels: &[Level], order: &[TrackKey]| {
            save_sort_rule(&conn, SortRule::new(id, name, levels, order))
                .unwrap_err()
                .to_string()
        };
        assert!(rule(" ", "Name", &[], &[]).contains("id"));
        assert!(rule(&"x".repeat(65), "Name", &[], &[]).contains("id"));
        assert!(rule("x", " ", &[], &[]).contains("name"));
        assert!(rule("x", "X", &[Level::Folder, Level::Album], &[]).contains("folder level"));
        assert!(rule("x", "X", &[Level::Album, Level::Album], &[]).contains("level is repeated"));
        assert!(
            rule("x", "X", &[], &[TrackKey::Title, TrackKey::Title]).contains("key is repeated")
        );

        assert!(set_ignored_articles(&conn, vec!["".into()]).is_err());
        assert!(set_ignored_articles(&conn, vec!["Der Die".into()]).is_err());
        assert!(set_ignored_articles(&conn, vec!["a".into(); 21]).is_err());
        assert_eq!(
            sort_settings(&conn).unwrap(),
            SortSettings::default(),
            "nothing was stored"
        );
        // No articles turns the feature off.
        assert!(set_ignored_articles(&conn, vec![])
            .unwrap()
            .ignored_articles
            .is_empty());
    }

    #[test]
    fn falls_back_on_invalid_stored_json() {
        let conn = db::open_in_memory().unwrap();
        for json in [
            "not json",
            "[1, 2]",
            "{}",
            r#"{"rules": 3, "ignoredArticles": "The"}"#,
            r#"{"rules": []}"#,
        ] {
            store_json(&conn, json);
            assert_eq!(
                sort_settings(&conn).unwrap(),
                SortSettings::default(),
                "{json}"
            );
        }
    }

    #[test]
    fn keeps_the_usable_parts_of_stored_json() {
        let conn = db::open_in_memory().unwrap();
        store_json(
            &conn,
            r#"{
                "rules": [
                    {"id": "composer", "name": "Composer", "levels": ["composer"], "trackOrder": []},
                    {"id": "songs", "name": "Songs", "levels": [], "trackOrder": ["title"], "newField": 1},
                    {"id": "bad", "name": "Bad", "levels": ["folder", "album"], "trackOrder": []},
                    {"id": "songs", "name": "Songs again", "levels": [], "trackOrder": []},
                    {"id": "partial", "name": "Partial"},
                    "not a rule"
                ],
                "ignoredArticles": ["The", "Not one"],
                "futureSetting": true
            }"#,
        );
        let settings = sort_settings(&conn).unwrap();
        assert_eq!(
            settings.rules,
            [SortRule::new("songs", "Songs", &[], &[TrackKey::Title])]
        );
        assert_eq!(settings.ignored_articles, ["The", "A"]);

        // Saving writes back only what was kept.
        save_sort_rule(
            &conn,
            SortRule::new("folder", "Folder", &[Level::Folder], &[]),
        )
        .unwrap();
        assert_eq!(ids(&sort_settings(&conn).unwrap()), ["songs", "folder"]);
    }
}
