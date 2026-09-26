//! Sort keys for names in the library, and the `anomp_sort_key` SQL function
//! that computes them, registered on every connection (`db::configure`).
//!
//! A key is a BLOB that sorts bytewise in the order people expect: case and
//! accents ignored ("élodie" with "Elodie"), numbers by value ("Track 2"
//! before "Track 10"), '/' before anything else (so a folder's files sort
//! before its neighbours'), and optionally without a leading article ("The
//! Beatles" under B). Keys are computed once per row and compared with
//! memcmp, which is cheaper than a collation that folds both strings on
//! every comparison. Names that fold alike get equal keys, so queries break
//! ties on the original text.
//!
//! The function is not used in the schema or any index: a connection without
//! it (e.g. the `sqlite3` shell) can still open the database.

use std::convert::Infallible;

use icu_normalizer::DecomposingNormalizerBorrowed;
use icu_properties::props::{GeneralCategory, GeneralCategoryGroup};
use icu_properties::CodePointMapData;
use rusqlite::functions::FunctionFlags;
use rusqlite::types::ValueRef;
use rusqlite::Connection;

/// `anomp_sort_key(text, articles)`: the key of `text`, or NULL if it is
/// NULL. `articles` is NULL, or leading words to skip separated by '\n'. It
/// should be a constant or bound parameter, since it is parsed once per
/// statement.
pub fn register(conn: &Connection) -> rusqlite::Result<()> {
    conn.create_scalar_function(
        "anomp_sort_key",
        2,
        FunctionFlags::SQLITE_UTF8 | FunctionFlags::SQLITE_DETERMINISTIC | FunctionFlags::SQLITE_INNOCUOUS,
        |ctx| {
            let articles = ctx.get_or_create_aux(1, |value| -> Result<Vec<String>, Infallible> {
                Ok(match value {
                    ValueRef::Text(text) => {
                        String::from_utf8_lossy(text).split('\n').map(fold).collect()
                    }
                    _ => Vec::new(),
                })
            })?;
            let text = match ctx.get_raw(0) {
                ValueRef::Null => return Ok(None),
                ValueRef::Text(text) | ValueRef::Blob(text) => String::from_utf8_lossy(text).into_owned(),
                ValueRef::Integer(number) => number.to_string(),
                ValueRef::Real(number) => number.to_string(),
            };
            Ok(Some(sort_key(&text, &articles)))
        },
    )
}

/// `text` for comparison: compatibility-decomposed (NFKD, so e.g. "ﬁ"
/// becomes "fi"), lowercase, and without accents or other combining marks.
/// Letters that don't decompose, such as "ø" or "ß", stay as they are.
pub fn fold(text: &str) -> String {
    let categories = CodePointMapData::<GeneralCategory>::new();
    DecomposingNormalizerBorrowed::new_nfkd()
        .normalize_iter(text.chars())
        .flat_map(char::to_lowercase)
        .filter(|&c| !GeneralCategoryGroup::Mark.contains(categories.get(c)))
        .collect()
}

/// A run of ASCII digits in a key: this byte, the number of digits after
/// leading zeros, then those digits. It is the byte of '0', so numbers sort
/// where digits would.
const NUMBER: u8 = b'0';

/// The key `text` sorts by. `articles` must already be folded; the first one
/// that starts `text` as a word, with more text after it, is skipped.
pub fn sort_key(text: &str, articles: &[String]) -> Vec<u8> {
    let folded = fold(text);
    let mut name = folded.trim();
    for article in articles.iter().filter(|article| !article.is_empty()) {
        let rest = name
            .strip_prefix(article.as_str())
            .and_then(|rest| rest.strip_prefix(' '))
            .map(str::trim_start);
        if let Some(rest) = rest.filter(|rest| !rest.is_empty()) {
            name = rest;
            break;
        }
    }

    // Digits and '/' are ASCII, and UTF-8 never uses ASCII bytes inside a
    // multibyte character, so working on bytes keeps the rest in code point
    // order.
    let bytes = name.as_bytes();
    let mut key = Vec::with_capacity(bytes.len() + 2);
    let mut index = 0;
    while index < bytes.len() {
        let byte = bytes[index];
        if byte.is_ascii_digit() {
            let start = index;
            while index < bytes.len() && bytes[index].is_ascii_digit() {
                index += 1;
            }
            let digits = &bytes[start..index];
            let significant = &digits[digits.iter().take_while(|&&digit| digit == b'0').count()..];
            key.push(NUMBER);
            // Numbers of over 255 digits compare by their digits alone.
            key.push(u8::try_from(significant.len()).unwrap_or(u8::MAX));
            key.extend_from_slice(significant);
        } else {
            key.push(if byte == b'/' { 0 } else { byte });
            index += 1;
        }
    }
    key
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `names` sorted by key, then by the text itself, as the queries do.
    fn sorted(names: &[&str], articles: &[&str]) -> Vec<String> {
        let articles: Vec<String> = articles.iter().map(|article| fold(article)).collect();
        let mut names: Vec<String> = names.iter().map(|name| name.to_string()).collect();
        names.sort_by(|a, b| {
            sort_key(a, &articles)
                .cmp(&sort_key(b, &articles))
                .then_with(|| a.cmp(b))
        });
        names
    }

    #[test]
    fn folds_case_accents_and_compatibility_forms() {
        assert_eq!(fold("Élodie"), "elodie");
        assert_eq!(fold("CAFÉ Déjà Vu"), "cafe deja vu");
        assert_eq!(fold("İstanbul"), "istanbul");
        assert_eq!(fold("ﬁve ＡＢＣ ²"), "five abc 2");
        assert_eq!(fold("Ørjan Straße 東京"), "ørjan straße 東京");
    }

    #[test]
    fn ignores_case_and_accents() {
        assert_eq!(
            sorted(&["Zoë", "élodie", "Eve", "Émile", "abba", "Elodie"], &[]),
            ["abba", "Elodie", "élodie", "Émile", "Eve", "Zoë"]
        );
        assert_eq!(sort_key("Élodie", &[]), sort_key("elodie", &[]));
    }

    #[test]
    fn orders_numbers_by_value() {
        assert_eq!(
            sorted(&["Track 10", "track 2", "Track 1", "Track 02", "Track 2b", "Track", "Track 100"], &[]),
            ["Track", "Track 1", "Track 02", "track 2", "Track 2b", "Track 10", "Track 100"]
        );
        assert_eq!(sorted(&["a10b2", "a10b10", "a9"], &[]), ["a9", "a10b2", "a10b10"]);
        // Numbers sort among other text as digits do: after spaces, before
        // letters.
        assert_eq!(sorted(&["ab", "a1", "a b"], &[]), ["a b", "a1", "ab"]);
    }

    #[test]
    fn sorts_a_folder_before_its_neighbours() {
        assert_eq!(
            sorted(&["Disc 1 extra/a.flac", "Disc 1/b.flac", "Disc 10/a.flac", "Disc 2/a.flac"], &[]),
            ["Disc 1/b.flac", "Disc 1 extra/a.flac", "Disc 2/a.flac", "Disc 10/a.flac"]
        );
    }

    #[test]
    fn skips_leading_articles_as_words() {
        let names = [
            "The Beatles",
            "Beach Boys",
            "A Tribe Called Quest",
            "The The",
            "Abba",
            "Theatre of Tragedy",
            "The",
            "a-ha",
        ];
        assert_eq!(
            sorted(&names, &["The", "A"]),
            [
                "a-ha",
                "Abba",
                "Beach Boys",
                "The Beatles",
                "The",
                "The The",
                "Theatre of Tragedy",
                "A Tribe Called Quest"
            ]
        );
        assert_eq!(sort_key("the  beatles", &[fold("The")]), sort_key("Beatles", &[]));
        assert_eq!(
            sorted(&names, &[]),
            [
                "A Tribe Called Quest",
                "a-ha",
                "Abba",
                "Beach Boys",
                "The",
                "The Beatles",
                "The The",
                "Theatre of Tragedy"
            ]
        );
    }

    #[test]
    fn sql_function_handles_null_and_articles() {
        let conn = Connection::open_in_memory().unwrap();
        register(&conn).unwrap();
        let key = |sql: &str| -> Option<Vec<u8>> { conn.query_row(sql, [], |row| row.get(0)).unwrap() };
        assert_eq!(key("SELECT anomp_sort_key(NULL, NULL)"), None);
        assert_eq!(key("SELECT anomp_sort_key('Élodie', NULL)"), Some(b"elodie".to_vec()));
        assert_eq!(
            key("SELECT anomp_sort_key('The Beatles', 'A' || char(10) || 'The')"),
            Some(b"beatles".to_vec())
        );
        assert_eq!(key("SELECT anomp_sort_key(7, NULL)"), Some(vec![NUMBER, 1, b'7']));
    }
}
