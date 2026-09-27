//! Smart playlists (PLAN.md F2): saved rules over the library, whose tracks
//! are whatever matches them now, so they follow every scan, heart, rating
//! and play. The rules are stored as JSON in `playlists.rules`.
//!
//! Queries are built as `library::browse` builds them: only fixed SQL
//! fragments, picked by the kind of each condition, with every value bound.

use rusqlite::types::Value;
use rusqlite::{Connection, Row};
use serde::{Deserialize, Serialize};

use super::marks::FAVOURITE_FILTER;
use super::{track_from_row, unix_now, Error, TrackSummary, TRACKS_FROM, TRACK_COLUMNS};

/// Most conditions a smart playlist can have.
pub const MAX_CONDITIONS: usize = 20;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SmartRules {
    /// Every condition must hold (true), or any one of them (false).
    pub match_all: bool,
    pub conditions: Vec<Condition>,
    pub order: SmartOrder,
    /// At most this many tracks, the first in `order`; `None` for all.
    pub limit: Option<u32>,
    /// Picks the order of `SmartOrder::Random`, so the list stays put
    /// between showing and playing it.
    #[serde(default)]
    pub seed: u32,
}

/// One condition on a track. Ranges are inclusive; a missing bound is open.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "field",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum Condition {
    /// One of the track's genres, ignoring case and accents.
    Genre { value: String },
    /// The track's year.
    Year { from: Option<u32>, to: Option<u32> },
    /// The file's type, by its extension ("flac", "mp3").
    Format { value: String },
    /// Came into the library in the last `days` days (O15).
    AddedWithin { days: u32 },
    /// Hearted, or on a hearted album or by a hearted artist (F3); or not.
    Favourite { value: bool },
    /// Rated at least this many stars (F3).
    Rating { at_least: u8 },
    /// Plays in the listening history (O8).
    PlayCount {
        at_least: Option<u32>,
        at_most: Option<u32>,
    },
    /// Not played in the last `days` days, or never.
    NotPlayedFor { days: u32 },
    /// Credits the artist, or is on their album (the name, ignoring case).
    Artist { value: String },
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SmartOrder {
    /// A shuffle that stays put until the rules change.
    #[default]
    Random,
    /// Newest first.
    DateAdded,
    MostPlayed,
    /// Most recently played first.
    LastPlayed,
    /// Highest rated first.
    Rating,
    /// By album artist, album, disc and track.
    Album,
    Title,
}

impl SmartRules {
    pub fn validate(&self) -> Result<(), Error> {
        let invalid = |message: &str| Err(Error::Invalid(format!("Smart playlist: {message}")));
        if self.conditions.len() > MAX_CONDITIONS {
            return invalid("it can have at most 20 conditions");
        }
        if self.limit == Some(0) {
            return invalid("the limit must be at least one track");
        }
        for condition in &self.conditions {
            match condition {
                Condition::Genre { value } | Condition::Artist { value }
                    if value.trim().is_empty() || value.len() > 200 =>
                {
                    return invalid("a name must be 1 to 200 bytes");
                }
                Condition::Format { value }
                    if value.is_empty()
                        || value.len() > 10
                        || !value.chars().all(|c| c.is_ascii_alphanumeric()) =>
                {
                    return invalid("a format is a file extension, such as flac");
                }
                Condition::Year {
                    from: Some(from),
                    to: Some(to),
                } if from > to => return invalid("a year range must start before it ends"),
                Condition::PlayCount {
                    at_least: Some(least),
                    at_most: Some(most),
                } if least > most => {
                    return invalid("a play count range must start before it ends")
                }
                Condition::Rating { at_least } if !(1..=5).contains(at_least) => {
                    return invalid("a rating is 1 to 5 stars");
                }
                Condition::AddedWithin { days } | Condition::NotPlayedFor { days }
                    if *days == 0 || *days > 36_500 =>
                {
                    return invalid("a number of days must be 1 to 36,500");
                }
                _ => {}
            }
        }
        Ok(())
    }
}

/// A statement's bound values.
struct Params(Vec<Value>);

impl Params {
    fn bind(&mut self, value: impl Into<Value>) -> String {
        self.0.push(value.into());
        format!("?{}", self.0.len())
    }
}

/// Plays of track `t`, and when it was last played.
const PLAY_COUNT: &str = "(SELECT count(*) FROM plays p WHERE p.track_id = t.id)";
const LAST_PLAYED: &str = "(SELECT max(p.played_at) FROM plays p WHERE p.track_id = t.id)";
const RATING: &str = "(SELECT rating FROM track_ratings r WHERE r.track_id = t.id)";

/// The WHERE clause for `rules` over `TRACKS_FROM`.
fn filter(rules: &SmartRules, params: &mut Params) -> String {
    let now = unix_now();
    let terms: Vec<String> = rules
        .conditions
        .iter()
        .map(|condition| match condition {
            Condition::Genre { value } => {
                format!("anomp_has_genre(t.genre, {})", params.bind(value.clone()))
            }
            Condition::Year { from, to } => format!(
                "(t.year IS NOT NULL AND t.year >= {} AND t.year <= {})",
                params.bind(i64::from(from.unwrap_or(0))),
                params.bind(i64::from(to.unwrap_or(u32::MAX)))
            ),
            Condition::Format { value } => {
                format!("t.relative_path LIKE {}", params.bind(format!("%.{value}")))
            }
            Condition::AddedWithin { days } => format!(
                "t.added_at >= {}",
                params.bind(now - i64::from(*days) * 86_400)
            ),
            Condition::Favourite { value: true } => FAVOURITE_FILTER.to_owned(),
            Condition::Favourite { value: false } => format!("NOT {FAVOURITE_FILTER}"),
            Condition::Rating { at_least } => {
                format!(
                    "IFNULL({RATING}, 0) >= {}",
                    params.bind(i64::from(*at_least))
                )
            }
            Condition::PlayCount { at_least, at_most } => format!(
                "({PLAY_COUNT} >= {} AND {PLAY_COUNT} <= {})",
                params.bind(i64::from(at_least.unwrap_or(0))),
                params.bind(i64::from(at_most.unwrap_or(u32::MAX)))
            ),
            Condition::NotPlayedFor { days } => format!(
                "IFNULL({LAST_PLAYED}, 0) < {}",
                params.bind(now - i64::from(*days) * 86_400)
            ),
            Condition::Artist { value } => {
                let name = params.bind(value.trim().to_owned());
                format!(
                    "(EXISTS (SELECT 1 FROM track_artists ta JOIN artists a ON a.id = ta.artist_id
                              WHERE ta.track_id = t.id AND a.name = {name})
                      OR album_artist.name = {name})"
                )
            }
        })
        .collect();
    if terms.is_empty() {
        "1".into()
    } else {
        terms.join(if rules.match_all { " AND " } else { " OR " })
    }
}

fn order_by(rules: &SmartRules, params: &mut Params) -> String {
    match rules.order {
        // A multiplicative hash of the id, salted by the seed.
        SmartOrder::Random => format!(
            "((t.id + {}) * 2654435761) % 4294967296, t.id",
            params.bind(i64::from(rules.seed))
        ),
        SmartOrder::DateAdded => "t.added_at DESC, t.id".into(),
        SmartOrder::MostPlayed => format!("{PLAY_COUNT} DESC, {LAST_PLAYED} DESC, t.id"),
        SmartOrder::LastPlayed => format!("{LAST_PLAYED} DESC NULLS LAST, t.id"),
        SmartOrder::Rating => format!("{RATING} DESC NULLS LAST, t.id"),
        SmartOrder::Album => "anomp_sort_key(album_artist.name, NULL) NULLS LAST,
             anomp_sort_key(album.title, NULL) NULLS LAST, t.album_id,
             IFNULL(t.disc_number, 1), t.track_number NULLS LAST, t.relative_path, t.range_start"
            .into(),
        SmartOrder::Title => "anomp_sort_key(t.title, NULL) NULLS LAST, t.id".into(),
    }
}

fn query<T>(
    conn: &Connection,
    sql: &str,
    params: &Params,
    mut map: impl FnMut(&Row) -> rusqlite::Result<T>,
) -> Result<Vec<T>, Error> {
    let mut statement = conn.prepare(sql)?;
    for (index, value) in params.0.iter().enumerate() {
        statement.raw_bind_parameter(index + 1, value)?;
    }
    let mut rows = statement.raw_query();
    let mut result = Vec::new();
    while let Some(row) = rows.next()? {
        result.push(map(row)?);
    }
    Ok(result)
}

/// The ids of the tracks matching `rules`, in its order, up to its limit.
pub fn track_ids(conn: &Connection, rules: &SmartRules) -> Result<Vec<i64>, Error> {
    rules.validate()?;
    let mut params = Params(Vec::new());
    let filter = filter(rules, &mut params);
    let order = order_by(rules, &mut params);
    let limit = params.bind(i64::from(rules.limit.unwrap_or(u32::MAX)));
    query(
        conn,
        &format!("SELECT t.id {TRACKS_FROM} WHERE {filter} ORDER BY {order} LIMIT {limit}"),
        &params,
        |row| row.get(0),
    )
}

/// How many tracks match `rules` (up to its limit), and their total length.
pub fn summary(conn: &Connection, rules: &SmartRules) -> Result<(u32, f64), Error> {
    rules.validate()?;
    let mut params = Params(Vec::new());
    let filter = filter(rules, &mut params);
    let order = order_by(rules, &mut params);
    let limit = params.bind(i64::from(rules.limit.unwrap_or(u32::MAX)));
    let rows = query(
        conn,
        &format!(
            "SELECT count(*), IFNULL(sum(duration), 0) FROM
                 (SELECT t.duration {TRACKS_FROM} WHERE {filter} ORDER BY {order} LIMIT {limit})"
        ),
        &params,
        |row| Ok((row.get(0)?, row.get(1)?)),
    )?;
    Ok(rows.into_iter().next().unwrap_or((0, 0.0)))
}

/// `limit` of the tracks matching `rules` from `offset`, in its order.
pub fn tracks(
    conn: &Connection,
    rules: &SmartRules,
    offset: u32,
    limit: u32,
) -> Result<Vec<TrackSummary>, Error> {
    rules.validate()?;
    let mut params = Params(Vec::new());
    let filter = filter(rules, &mut params);
    let order = order_by(rules, &mut params);
    let cap = params.bind(i64::from(rules.limit.unwrap_or(u32::MAX)));
    let limit = params.bind(i64::from(limit));
    let offset = params.bind(i64::from(offset));
    query(
        conn,
        &format!(
            "SELECT * FROM (SELECT {TRACK_COLUMNS} {TRACKS_FROM} WHERE {filter}
                            ORDER BY {order} LIMIT {cap})
             LIMIT {limit} OFFSET {offset}"
        ),
        &params,
        track_from_row,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::library::marks::{self, MarkKind};
    use crate::library::test_library::{titles, track, Library};

    fn rules(conditions: Vec<Condition>, order: SmartOrder) -> SmartRules {
        SmartRules {
            match_all: true,
            conditions,
            order,
            limit: None,
            seed: 7,
        }
    }

    fn library() -> Library {
        Library::new([
            track("a/1.flac")
                .title("Rock 94")
                .artist("Band")
                .album("X")
                .genre("Rock")
                .year(1994),
            track("a/2.mp3")
                .title("Pop 99")
                .artist("Band; Guest")
                .album("Y")
                .genre("Pop; Rock")
                .year(1999),
            track("a/3.flac")
                .title("Jazz 70")
                .artist("Trio")
                .genre("Jazz")
                .year(1970),
            track("a/4.ogg").title("Untagged"),
        ])
    }

    fn matching(library: &Library, rules: &SmartRules) -> Vec<String> {
        titles(&tracks(&library.conn, rules, 0, 100).unwrap())
    }

    #[test]
    fn matches_each_kind_of_condition() {
        let library = library();
        let conn = &library.conn;
        let by_title = |conditions| rules(conditions, SmartOrder::Title);
        assert_eq!(
            matching(
                &library,
                &by_title(vec![Condition::Genre {
                    value: "rock".into()
                }])
            ),
            ["Pop 99", "Rock 94"]
        );
        assert_eq!(
            matching(
                &library,
                &by_title(vec![Condition::Year {
                    from: Some(1990),
                    to: None
                }])
            ),
            ["Pop 99", "Rock 94"]
        );
        assert_eq!(
            matching(
                &library,
                &by_title(vec![Condition::Format {
                    value: "FLAC".into()
                }])
            ),
            ["Jazz 70", "Rock 94"]
        );
        assert_eq!(
            matching(
                &library,
                &by_title(vec![Condition::Artist {
                    value: "guest".into()
                }])
            ),
            ["Pop 99"]
        );
        assert_eq!(
            matching(
                &library,
                &by_title(vec![Condition::AddedWithin { days: 30 }])
            )
            .len(),
            0,
            "the test library's tracks arrived in 1970"
        );

        let ids: Vec<i64> = crate::library::tracks(conn)
            .unwrap()
            .iter()
            .map(|t| t.id)
            .collect();
        marks::set_favourite(conn, MarkKind::Track, &[ids[2]], true).unwrap();
        marks::set_rating(conn, &[ids[0]], Some(5)).unwrap();
        conn.execute(
            "INSERT INTO plays (track_id, played_at, seconds) VALUES (?1, 100, 60), (?1, 200, 60)",
            [ids[1]],
        )
        .unwrap();
        assert_eq!(
            matching(
                &library,
                &by_title(vec![Condition::Favourite { value: true }])
            ),
            ["Jazz 70"]
        );
        assert_eq!(
            matching(&library, &by_title(vec![Condition::Rating { at_least: 4 }])),
            ["Rock 94"]
        );
        assert_eq!(
            matching(
                &library,
                &by_title(vec![Condition::PlayCount {
                    at_least: Some(1),
                    at_most: None
                }])
            ),
            ["Pop 99"]
        );
        assert_eq!(
            matching(
                &library,
                &by_title(vec![Condition::NotPlayedFor { days: 1 }])
            )
            .len(),
            4,
            "played in 1970"
        );

        // Any of them, and the orders.
        let mut any = by_title(vec![
            Condition::Genre {
                value: "Jazz".into(),
            },
            Condition::Rating { at_least: 5 },
        ]);
        any.match_all = false;
        assert_eq!(matching(&library, &any), ["Jazz 70", "Rock 94"]);
        any.order = SmartOrder::MostPlayed;
        any.conditions.clear();
        assert_eq!(matching(&library, &any)[0], "Pop 99");
        any.order = SmartOrder::Rating;
        assert_eq!(matching(&library, &any)[0], "Rock 94");
        any.limit = Some(2);
        assert_eq!(summary(conn, &any).unwrap().0, 2);
        assert_eq!(track_ids(conn, &any).unwrap().len(), 2);
    }

    #[test]
    fn a_random_order_stays_put_for_its_seed() {
        let library = library();
        let mut shuffled = rules(Vec::new(), SmartOrder::Random);
        let first = matching(&library, &shuffled);
        assert_eq!(matching(&library, &shuffled), first);
        assert_eq!(first.len(), 4);
        let orders: std::collections::HashSet<Vec<String>> = (0..20)
            .map(|seed| {
                shuffled.seed = seed;
                matching(&library, &shuffled)
            })
            .collect();
        assert!(orders.len() > 1);
    }

    #[test]
    fn refuses_bad_rules() {
        let bad = |condition| {
            rules(vec![condition], SmartOrder::Title)
                .validate()
                .is_err()
        };
        assert!(bad(Condition::Format {
            value: "fl%c".into()
        }));
        assert!(bad(Condition::Genre { value: " ".into() }));
        assert!(bad(Condition::Year {
            from: Some(2000),
            to: Some(1990)
        }));
        assert!(bad(Condition::Rating { at_least: 0 }));
        assert!(bad(Condition::AddedWithin { days: 0 }));
        let json = r#"{"matchAll":true,"conditions":[{"field":"playCount","atLeast":3,"atMost":null}],
                       "order":"mostPlayed","limit":25}"#;
        let parsed: SmartRules = serde_json::from_str(json).unwrap();
        assert_eq!(
            parsed.conditions,
            [Condition::PlayCount {
                at_least: Some(3),
                at_most: None
            }]
        );
        assert_eq!(parsed.seed, 0);
    }
}
