//! Genre tags can hold several genres: the core joins a tag's values with
//! "; ", and some taggers write "Rock;Pop" into one value. The SQL functions
//! here, registered on every connection (`db::configure`), split them so a
//! track is listed under each of its genres. Splitting when querying keeps
//! the schema and the scanner as they are (PLAN.md Phase 2 has timings).

use rusqlite::functions::FunctionFlags;
use rusqlite::types::ValueRef;
use rusqlite::Connection;

use super::sort_key::fold;

/// The genres in a genre tag: split at ';' and trimmed, without empty ones
/// or repeats (ignoring case and accents; the first spelling is kept).
pub fn split(tag: &str) -> Vec<&str> {
    let pieces: Vec<&str> = tag
        .split(';')
        .map(str::trim)
        .filter(|genre| !genre.is_empty())
        .collect();
    if pieces.len() < 2 {
        return pieces;
    }
    let mut genres = Vec::new();
    let mut folded: Vec<String> = Vec::new();
    for genre in pieces {
        let key = fold(genre);
        if !folded.contains(&key) {
            folded.push(key);
            genres.push(genre);
        }
    }
    genres
}

/// Registers:
/// - `anomp_genres(tag)`: the tag's genres as a JSON array for `json_each`,
///   or `[null]` if it has none, so that such a track is listed once, under
///   a null genre.
/// - `anomp_has_genre(tag, genre)`: whether `genre` is one of the tag's
///   genres, ignoring case and accents. A null `genre` matches tags with
///   none.
pub fn register(conn: &Connection) -> rusqlite::Result<()> {
    let flags = FunctionFlags::SQLITE_UTF8 | FunctionFlags::SQLITE_DETERMINISTIC | FunctionFlags::SQLITE_INNOCUOUS;
    conn.create_scalar_function("anomp_genres", 1, flags, |ctx| {
        let tag = text(ctx.get_raw(0));
        let genres = split(&tag);
        Ok(if genres.is_empty() {
            "[null]".to_owned()
        } else {
            serde_json::to_string(&genres).expect("strings serialize to JSON")
        })
    })?;
    conn.create_scalar_function("anomp_has_genre", 2, flags, |ctx| {
        let tag = text(ctx.get_raw(0));
        let genres = split(&tag);
        Ok(match ctx.get_raw(1) {
            ValueRef::Null => genres.is_empty(),
            genre => {
                let genre = fold(text(genre).trim());
                genres.iter().any(|candidate| fold(candidate) == genre)
            }
        })
    })
}

/// A value as text; null is empty.
fn text(value: ValueRef<'_>) -> String {
    match value {
        ValueRef::Null => String::new(),
        ValueRef::Text(text) | ValueRef::Blob(text) => String::from_utf8_lossy(text).into_owned(),
        ValueRef::Integer(number) => number.to_string(),
        ValueRef::Real(number) => number.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_trims_and_removes_repeats() {
        assert_eq!(split("Rock; Pop"), ["Rock", "Pop"]);
        assert_eq!(split("Rock;Pop ; ;Jazz"), ["Rock", "Pop", "Jazz"]);
        assert_eq!(split("Pop; pop; PÖP; Hip-Hop/Rap"), ["Pop", "Hip-Hop/Rap"]);
        assert!(split("; ;").is_empty());
        assert!(split("").is_empty());
    }

    #[test]
    fn sql_functions() {
        let conn = Connection::open_in_memory().unwrap();
        register(&conn).unwrap();
        let genres = |tag: &str| -> Vec<Option<String>> {
            conn.prepare("SELECT value FROM json_each(anomp_genres(?1))")
                .unwrap()
                .query_map([tag], |row| row.get(0))
                .unwrap()
                .collect::<Result<_, _>>()
                .unwrap()
        };
        assert_eq!(genres("Rock; \"Pop\"\n"), [Some("Rock".into()), Some("\"Pop\"".into())]);
        assert_eq!(genres(" ; "), [None]);
        let genres_of_null: String = conn.query_row("SELECT anomp_genres(NULL)", [], |row| row.get(0)).unwrap();
        assert_eq!(genres_of_null, "[null]");

        let has = |tag: Option<&str>, genre: Option<&str>| -> bool {
            conn.query_row("SELECT anomp_has_genre(?1, ?2)", [tag, genre], |row| row.get(0))
                .unwrap()
        };
        assert!(has(Some("Rock; Électro"), Some("electro")));
        assert!(has(Some("Rock; Électro"), Some(" ROCK ")));
        assert!(!has(Some("Rock; Électro"), Some("Pop")));
        assert!(!has(Some("Rock"), None));
        assert!(has(Some("; "), None));
        assert!(has(None, None));
        assert!(!has(None, Some("Rock")));
    }
}
