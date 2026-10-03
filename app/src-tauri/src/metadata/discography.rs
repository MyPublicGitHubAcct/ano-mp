//! An artist's discography on MusicBrainz, less what the library has: the
//! release groups of the MusicBrainz artist the library artist is matched
//! to, as MusicBrainz's own artist page lists them (no bootlegs), without
//! those the library holds.
//!
//! A release group is in the library if any album in it is matched to one
//! of its releases, whoever the album artist is (a collaboration may be
//! filed under another name). An album of the artist that isn't matched
//! (not looked up yet, awaiting review, or not found) counts by its title
//! instead, so it isn't listed as missing only because matching hasn't
//! found it.
//!
//! Nothing is stored beyond the response cache: the list is worked out
//! each time from the cached pages and the albums' current matches.

use std::collections::HashSet;
use std::time::Duration;

use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;

use super::artists;
use super::cache;
use super::http::Client;
use super::matcher;
use super::musicbrainz::{self, ReleaseGroupEntry, BROWSE_LIMIT};
use super::settings::SourceId;
use super::Error;

/// How long a fetched page is used before asking again: discographies grow,
/// like search results.
pub const MAX_AGE: Duration = Duration::from_secs(7 * 86400);

/// Pages read at most (1,000 release groups, 10 s at MusicBrainz's rate
/// limit). Few artists have more official release groups than that.
pub const MAX_PAGES: u32 = 10;

/// Title similarity (`matcher::title_similarity`) at which an album that
/// isn't matched counts as a release group of that title.
const SAME_TITLE: f64 = 0.9;

#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct Discography {
    /// The MusicBrainz artist listed, and their name there.
    pub musicbrainz_id: String,
    pub musicbrainz_name: String,
    /// The release groups the library doesn't have, oldest first, undated
    /// last.
    pub missing: Vec<ReleaseGroupEntry>,
    /// How many of those listed the library has.
    pub in_library: u32,
    /// How many release groups MusicBrainz has for the artist, and how many
    /// were listed: fewer when there are more than `MAX_PAGES` pages.
    pub total: u32,
    pub listed: u32,
}

/// Fetches through the client and the response cache, which is bypassed
/// with `refresh` (though still used offline).
pub fn online<'a>(
    client: &'a Client,
    conn: &'a Connection,
    refresh: bool,
) -> impl FnMut(&str) -> Result<String, Error> + 'a {
    let max_age = if refresh { Duration::ZERO } else { MAX_AGE };
    move |url| client.get_json(conn, url, max_age)
}

/// Reads only the response cache, however old, for when MusicBrainz may not
/// be contacted; `why` says why not.
pub fn cached<'a>(
    conn: &'a Connection,
    why: &'a str,
) -> impl FnMut(&str) -> Result<String, Error> + 'a {
    move |url| {
        cache::lookup(conn, url)?
            .map(|(body, _)| body)
            .ok_or_else(|| {
                Error::Invalid(format!(
                    "{why}, and this discography hasn't been fetched before"
                ))
            })
    }
}

/// The discography of artist `artist_id`, without what the library has.
/// `fetch` returns the body at a MusicBrainz URL (`online` or `cached`).
pub fn discography(
    conn: &Connection,
    artist_id: i64,
    fetch: &mut dyn FnMut(&str) -> Result<String, Error>,
) -> Result<Discography, Error> {
    let name: String = conn
        .query_row(
            "SELECT name FROM artists WHERE id = ?1",
            [artist_id],
            |row| row.get(0),
        )
        .optional()?
        .ok_or_else(|| Error::Invalid(crate::coded::gone(crate::coded::Gone::Artist)))?;
    let link = artists::artist_link(conn, artist_id, SourceId::MusicBrainz)?;
    let Some(mbid) = link
        .as_ref()
        .and_then(|link| link.matched_id())
        .map(str::to_owned)
    else {
        return Err(Error::Invalid(format!(
            "{name} isn't matched to a MusicBrainz artist"
        )));
    };
    let musicbrainz_name = link
        .and_then(|link| link.details::<musicbrainz::Artist>())
        .map_or(name, |artist| artist.name);

    let mut groups: Vec<ReleaseGroupEntry> = Vec::new();
    let mut seen = HashSet::new();
    let mut total = 0;
    for _ in 0..MAX_PAGES {
        // By what's been read, not page number: a page cached when the
        // count was different may overlap the next, or fall short of it.
        let offset = seen.len() as u32;
        let page = musicbrainz::parse_release_groups(&fetch(&musicbrainz::release_groups_url(
            &mbid, offset,
        ))?)?;
        total = page.count;
        let before = seen.len();
        for group in page.groups {
            if seen.insert(group.id.clone()) {
                groups.push(group);
            }
        }
        if seen.len() == before || seen.len() as u32 >= total || offset + BROWSE_LIMIT > total {
            break;
        }
    }

    let (owned, titles) = in_library(conn, artist_id)?;
    let listed = groups.len() as u32;
    let (have, mut missing): (Vec<_>, Vec<_>) = groups.into_iter().partition(|group| {
        owned.contains(&group.id)
            || titles
                .iter()
                .any(|title| matcher::title_similarity(title, &group.title) >= SAME_TITLE)
    });
    missing.sort_by(|a, b| {
        let date = |group: &ReleaseGroupEntry| {
            (
                group.first_release_date.is_none(),
                group.first_release_date.clone(),
            )
        };
        date(a)
            .cmp(&date(b))
            .then_with(|| a.title.to_lowercase().cmp(&b.title.to_lowercase()))
    });
    Ok(Discography {
        musicbrainz_id: mbid,
        musicbrainz_name,
        missing,
        in_library: have.len() as u32,
        total: total.max(listed),
        listed,
    })
}

/// The release groups the library's matched albums belong to, and the
/// titles of artist `artist_id`'s albums that aren't matched.
fn in_library(conn: &Connection, artist_id: i64) -> Result<(HashSet<String>, Vec<String>), Error> {
    let source = SourceId::MusicBrainz.as_str();
    let mut statement = conn.prepare_cached(
        "SELECT DISTINCT json_extract(details, '$.releaseGroupId') FROM album_links
         WHERE source = ?1 AND status = 'matched' AND json_valid(details)",
    )?;
    let owned = statement
        .query_map([source], |row| row.get::<_, Option<String>>(0))?
        .filter_map(Result::transpose)
        .collect::<Result<HashSet<String>, _>>()?;
    let mut statement = conn.prepare_cached(
        "SELECT title FROM albums al
         WHERE artist_id = ?1 AND NOT EXISTS
             (SELECT 1 FROM album_links
              WHERE album_id = al.id AND source = ?2 AND status = 'matched')",
    )?;
    let titles = statement
        .query_map(params![artist_id, source], |row| row.get(0))?
        .collect::<Result<Vec<String>, _>>()?;
    Ok((owned, titles))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::library::test_library::{track, Library};
    use crate::metadata::albums::LinkStatus;
    use crate::metadata::http::testing::fake_client;
    use crate::metadata::musicbrainz::fixtures;

    const AMNESIAC: &str = "bca9280e-28b4-327f-8fe0-fd918579e486";

    /// Radiohead's "In Rainbows" (to be matched), "OK Computer (Collector's
    /// Edition)" and "Kid A" (not matched), "Amnesiac" filed under another
    /// album artist, and an album by someone else.
    fn library() -> Library {
        Library::new([
            track("R/In Rainbows/01.flac")
                .artist("Radiohead")
                .album("In Rainbows"),
            track("R/OK Computer/01.flac")
                .artist("Radiohead")
                .album("OK Computer (Collector's Edition)"),
            track("R/Kid A/01.flac").artist("Radiohead").album("Kid A"),
            track("X/Amnesiac/01.flac")
                .artist("Radiohead")
                .album_artist("Radiohead & Friends")
                .album("Amnesiac"),
            track("O/Pablo Honey/01.flac")
                .artist("Someone Else")
                .album("Pablo Honey"),
        ])
    }

    fn album(library: &Library, title: &str) -> i64 {
        library
            .conn
            .query_row("SELECT id FROM albums WHERE title = ?1", [title], |row| {
                row.get(0)
            })
            .unwrap()
    }

    fn link_album(library: &Library, title: &str, status: &str, group: &str) {
        library
            .conn
            .execute(
                "INSERT OR REPLACE INTO album_links
                     (album_id, source, status, external_id, score, chosen_by, details,
                      checked_at)
                 VALUES (?1, 'musicbrainz', ?2, 'release', 1.0, 'auto', ?3, 0)",
                params![
                    album(library, title),
                    status,
                    format!(r#"{{"releaseGroupId": "{group}"}}"#)
                ],
            )
            .unwrap();
    }

    fn match_radiohead(library: &Library) -> i64 {
        let radiohead = library.artist(Some("Radiohead")).unwrap();
        let artist = musicbrainz::parse_artist(fixtures::ARTIST_JSON).unwrap();
        artists::store_link(
            &library.conn,
            radiohead,
            SourceId::MusicBrainz,
            LinkStatus::Matched,
            Some(fixtures::ARTIST),
            1.0,
            Some(&artist),
        )
        .unwrap();
        radiohead
    }

    #[test]
    fn lists_what_the_library_lacks() {
        let library = library();
        let radiohead = match_radiohead(&library);
        link_album(&library, "In Rainbows", "matched", fixtures::RELEASE_GROUP);
        link_album(&library, "Amnesiac", "matched", AMNESIAC);
        // A candidate awaiting review doesn't count, but the title does.
        link_album(&library, "Kid A", "review", "not-kid-a");
        let (client, transport, _clock) = fake_client();
        transport.push_status(
            &musicbrainz::release_groups_url(fixtures::ARTIST, 0),
            200,
            fixtures::RELEASE_GROUPS_JSON,
        );
        let found = discography(
            &library.conn,
            radiohead,
            &mut online(&client, &library.conn, false),
        )
        .unwrap();
        assert_eq!(found.musicbrainz_id, fixtures::ARTIST);
        assert_eq!(found.musicbrainz_name, "Radiohead");
        assert_eq!((found.total, found.listed), (14, 14));
        // In Rainbows and Amnesiac by their match, OK Computer and Kid A by
        // title. Someone else's "Pablo Honey" isn't Radiohead's.
        assert_eq!(found.in_library, 4);
        let titles: Vec<&str> = found.missing.iter().map(|g| g.title.as_str()).collect();
        assert_eq!(
            titles,
            [
                "Drill",
                "Creep",
                "Pablo Honey",
                "The Bends",
                "I Might Be Wrong: Live Recordings",
                "The Gloaming (DJ Shadow remix)",
                "The Best Of",
                "5 Album Set",
                "A Moon Shaped Pool",
                "KCRW Sessions 2003‐06‐26",
            ],
            "oldest first, undated last"
        );
        assert_eq!(transport.urls().len(), 1, "one page");

        // Cached now: read without the network, and with it off.
        let (client, transport, _clock) = fake_client();
        discography(
            &library.conn,
            radiohead,
            &mut online(&client, &library.conn, false),
        )
        .unwrap();
        assert!(transport.urls().is_empty());
        let offline =
            discography(&library.conn, radiohead, &mut cached(&library.conn, "Off")).unwrap();
        assert_eq!(offline.missing.len(), 10);
        // "Check again" asks anyway.
        let (client, transport, _clock) = fake_client();
        transport.push_status(
            &musicbrainz::release_groups_url(fixtures::ARTIST, 0),
            200,
            r#"{"release-group-count": 0, "release-groups": []}"#,
        );
        let refreshed = discography(
            &library.conn,
            radiohead,
            &mut online(&client, &library.conn, true),
        )
        .unwrap();
        assert!(refreshed.missing.is_empty());
        assert_eq!(transport.urls().len(), 1);
    }

    /// A page of `count` release groups numbered from `first`, of `total`.
    fn page(first: u32, count: u32, total: u32) -> String {
        let groups: Vec<String> = (first..first + count)
            .map(|n| format!(r#"{{"id": "g{n}", "title": "Single {n}"}}"#))
            .collect();
        format!(
            r#"{{"release-group-count": {total}, "release-groups": [{}]}}"#,
            groups.join(",")
        )
    }

    #[test]
    fn reads_every_page_up_to_the_limit() {
        let library = library();
        let radiohead = match_radiohead(&library);
        let mut urls = Vec::new();
        let found = discography(&library.conn, radiohead, &mut |url: &str| {
            urls.push(url.to_owned());
            let offset = urls.len() as u32 - 1;
            Ok(page(offset * 100, if offset == 0 { 100 } else { 50 }, 150))
        })
        .unwrap();
        assert_eq!((found.total, found.listed), (150, 150));
        assert_eq!(
            urls,
            [
                musicbrainz::release_groups_url(fixtures::ARTIST, 0),
                musicbrainz::release_groups_url(fixtures::ARTIST, 100)
            ]
        );
        assert_eq!(found.missing[0].title, "Single 0");

        let mut requests = 0;
        let found = discography(&library.conn, radiohead, &mut |_: &str| {
            requests += 1;
            Ok(page((requests - 1) * 100, 100, 5000))
        })
        .unwrap();
        assert_eq!(requests, MAX_PAGES);
        assert_eq!((found.total, found.listed), (5000, 1000));

        // A short page that isn't the last, or one repeating what was read,
        // doesn't loop or leave gaps.
        let mut offsets = Vec::new();
        let found = discography(&library.conn, radiohead, &mut |url: &str| {
            let offset: u32 = url
                .split("offset=")
                .nth(1)
                .and_then(|rest| rest.split('&').next())
                .unwrap()
                .parse()
                .unwrap();
            offsets.push(offset);
            Ok(match offset {
                0 => page(0, 60, 300),
                _ => page(50, 20, 300),
            })
        })
        .unwrap();
        assert_eq!(offsets, [0, 60, 70]);
        assert_eq!(found.listed, 70);
    }

    /// Against the real service: `cargo test live_ -- --ignored`.
    #[test]
    #[ignore]
    fn live_discography() {
        use crate::metadata::http::{Client, SystemClock, UreqTransport};
        let library = library();
        let radiohead = match_radiohead(&library);
        let client = Client::new(Box::new(UreqTransport::new()), Box::new(SystemClock));
        let found = discography(
            &library.conn,
            radiohead,
            &mut online(&client, &library.conn, false),
        )
        .unwrap();
        assert!(found.listed > BROWSE_LIMIT, "more than one page");
        assert_eq!(found.listed, found.total);
        // In Rainbows, OK Computer and Kid A by title; Amnesiac is filed
        // under another artist and not matched.
        let missing = |id: &str| found.missing.iter().any(|g| g.id == id);
        assert!(!missing(fixtures::RELEASE_GROUP));
        assert!(missing(AMNESIAC));
        assert_eq!(found.in_library, 3);
    }

    #[test]
    fn needs_a_matched_artist() {
        let library = library();
        let radiohead = library.artist(Some("Radiohead")).unwrap();
        let mut fetch = |_: &str| -> Result<String, Error> { panic!("nothing is fetched") };
        let error = discography(&library.conn, radiohead, &mut fetch).unwrap_err();
        assert_eq!(
            error.to_string(),
            "Radiohead isn't matched to a MusicBrainz artist"
        );
        artists::store_link(
            &library.conn,
            radiohead,
            SourceId::MusicBrainz,
            LinkStatus::Review,
            Some(fixtures::ARTIST),
            0.8,
            None::<&musicbrainz::Artist>,
        )
        .unwrap();
        assert!(discography(&library.conn, radiohead, &mut fetch).is_err());
        assert!(discography(&library.conn, 999, &mut fetch).is_err());

        let radiohead = match_radiohead(&library);
        let error = discography(
            &library.conn,
            radiohead,
            &mut cached(&library.conn, "Online services are turned off"),
        )
        .unwrap_err();
        assert_eq!(
            error.to_string(),
            "Online services are turned off, and this discography hasn't been fetched before"
        );
    }
}
