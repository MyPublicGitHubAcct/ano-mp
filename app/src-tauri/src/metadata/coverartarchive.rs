//! The Cover Art Archive (https://coverartarchive.org): album art for
//! MusicBrainz releases and release groups. An album matched to a release
//! gets the release's front cover, else its release group's. Image URLs
//! redirect (307) to archive.org, which the transport follows. Pictures are
//! downloaded into the image cache (`images`), and the art handler serves
//! them from there; it never comes here except to work out the URLs.
//!
//! Each automatic fetch is recorded as the album's `album_links` row for
//! this source: 'matched' if the archive had a cover, 'none' if not, with
//! the MusicBrainz release it was for as `external_id`. So an album whose
//! cover the archive lacks isn't asked about on every run, and a record
//! for a release the album is no longer matched to doesn't count.

use std::collections::BTreeMap;
use std::time::Duration;

use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::albums::{self, AlbumLink, LinkStatus};
use super::cache::unix_now;
use super::http::{Client, IMAGE_LIMIT};
use super::images::ImageCache;
use super::musicbrainz::is_mbid;
use super::settings::SourceId;
use super::Error;
use crate::library::art::CoverCandidate;

pub const HOST: &str = "coverartarchive.org";

const BASE: &str = "https://coverartarchive.org";

/// The thumbnail fetched as an album's cover. The archive has 250, 500 and
/// 1200 px; 500 is sharp at the largest size the player shows art (about
/// 250 pt on a 2× screen) at around 100 KB.
pub const COVER_SIZE: u32 = 500;

/// How long a release's image listing is used before asking again. People
/// add art to releases now and then, so sooner than a MusicBrainz lookup.
const LISTING_MAX_AGE: Duration = Duration::from_secs(7 * 86400);

pub fn release_front_url(release_id: &str) -> String {
    format!("{BASE}/release/{release_id}/front-{COVER_SIZE}")
}

pub fn release_group_front_url(release_group_id: &str) -> String {
    format!("{BASE}/release-group/{release_group_id}/front-{COVER_SIZE}")
}

/// The thumbnail the "Choose cover" dialog previews pictures by.
const PREVIEW_SIZE: u32 = 250;

pub fn listing_url(release_id: &str) -> String {
    format!("{BASE}/release/{release_id}/")
}

/// The URLs that may hold album `album_id`'s front cover, best first: its
/// MusicBrainz release's (unless MusicBrainz says it has none), then its
/// release group's. Empty unless the album is matched to a release; one
/// that only awaits review has none.
pub fn cover_urls(conn: &Connection, album_id: i64) -> Result<Vec<String>, Error> {
    Ok(albums::album_link(conn, album_id, SourceId::MusicBrainz)?
        .map_or_else(Vec::new, |link| link_cover_urls(&link)))
}

/// The release a MusicBrainz link is matched to, if its covers can be
/// fetched.
pub fn matched_release(link: &AlbumLink) -> Option<&str> {
    match (&link.status, &link.external_id) {
        (LinkStatus::Matched, Some(id)) if is_mbid(id) => Some(id),
        _ => None,
    }
}

/// `cover_urls` for the album's MusicBrainz link.
pub fn link_cover_urls(link: &AlbumLink) -> Vec<String> {
    let Some(release_id) = matched_release(link) else {
        return Vec::new();
    };
    // Details stored by another version may not parse: then the release is
    // still tried.
    let release = link.release.as_ref();
    let mut urls = Vec::new();
    if release.and_then(|release| release.has_front_art) != Some(false) {
        urls.push(release_front_url(release_id));
    }
    if let Some(group) = release
        .and_then(|release| release.release_group_id.as_deref())
        .filter(|id| is_mbid(id))
    {
        urls.push(release_group_front_url(group));
    }
    urls
}

/// What `fetch_album_art` or `fetch_image` did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fetched {
    /// Nothing to fetch: the album isn't matched to a MusicBrainz release.
    NotLinked,
    /// The picture was in the cache already; nothing was requested.
    Cached,
    Downloaded,
    /// The archive has no such picture.
    NotFound,
}

/// Downloads album `album_id`'s cover into `cache`: the archive picture the
/// user chose for it, else its release's front cover, else its release
/// group's. Does nothing if one is cached already. Offline, it fails with
/// `Error::Offline` and changes nothing. Whether the source is enabled is up
/// to the caller. Unless it fetched the user's choice, what it found is
/// recorded (see `cover_check`).
pub fn fetch_album_art(
    client: &Client,
    conn: &Connection,
    cache: &ImageCache,
    album_id: i64,
) -> Result<Fetched, Error> {
    if let Some(url) = chosen_image(conn, album_id)? {
        return fetch_image(client, cache, &url);
    }
    let Some(link) = albums::album_link(conn, album_id, SourceId::MusicBrainz)? else {
        return Ok(Fetched::NotLinked);
    };
    let (Some(release_id), urls) = (matched_release(&link), link_cover_urls(&link)) else {
        return Ok(Fetched::NotLinked);
    };
    let fetched = if urls.iter().any(|url| cache.contains(url)) {
        Fetched::Cached
    } else {
        let mut fetched = Fetched::NotFound;
        for url in &urls {
            fetched = fetch_image(client, cache, url)?;
            if fetched != Fetched::NotFound {
                break;
            }
        }
        fetched
    };
    record_check(conn, album_id, release_id, fetched != Fetched::NotFound)?;
    Ok(fetched)
}

/// What the archive said when album `album_id`'s cover was last fetched
/// automatically: status 'matched' (it had one) or 'none', and the release
/// asked about as `external_id`.
pub fn cover_check(conn: &Connection, album_id: i64) -> Result<Option<AlbumLink>, Error> {
    albums::album_link(conn, album_id, SourceId::CoverArtArchive)
}

fn record_check(
    conn: &Connection,
    album_id: i64,
    release_id: &str,
    found: bool,
) -> Result<(), Error> {
    let (status, score) = if found {
        (LinkStatus::Matched, 1.0)
    } else {
        (LinkStatus::None, 0.0)
    };
    conn.prepare_cached(
        "INSERT INTO album_links
             (album_id, source, status, external_id, score, chosen_by, details, checked_at)
         VALUES (?1, ?2, ?3, ?4, ?5, 'auto', NULL, ?6)
         ON CONFLICT (album_id, source) DO UPDATE SET
             status = excluded.status, external_id = excluded.external_id,
             score = excluded.score, checked_at = excluded.checked_at
         WHERE album_links.chosen_by = 'auto'",
    )?
    .execute(params![
        album_id,
        SourceId::CoverArtArchive.as_str(),
        status.as_str(),
        release_id,
        score,
        unix_now()
    ])?;
    Ok(())
}

/// Downloads the picture at `url` into `cache` unless it's there already.
pub fn fetch_image(client: &Client, cache: &ImageCache, url: &str) -> Result<Fetched, Error> {
    if cache.contains(url) {
        return Ok(Fetched::Cached);
    }
    match client.get(url, "image/*", IMAGE_LIMIT) {
        Ok(response) => {
            cache.store(url, &response.body)?;
            Ok(Fetched::Downloaded)
        }
        Err(Error::Status { status: 404, .. }) => Ok(Fetched::NotFound),
        Err(error) => Err(error),
    }
}

/// The archive picture the user chose for album `album_id`, if any (see
/// migration 003's `album_art`).
pub fn chosen_image(conn: &Connection, album_id: i64) -> Result<Option<String>, Error> {
    let reference: Option<Option<String>> = conn
        .prepare_cached("SELECT reference FROM album_art WHERE album_id = ?1 AND source = ?2")?
        .query_row(
            params![album_id, SourceId::CoverArtArchive.as_str()],
            |row| row.get(0),
        )
        .optional()?;
    Ok(reference.flatten().filter(|url| is_archive_url(url)))
}

/// Whether `url` is one of the archive's, so a stored reference can be
/// requested.
pub fn is_archive_url(url: &str) -> bool {
    url.starts_with(&format!("{BASE}/"))
}

/// A picture in a release's listing.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Image {
    pub id: String,
    /// "Front", "Back", "Booklet", "Medium"…
    pub types: Vec<String>,
    /// Whether it's the release's chosen front cover.
    pub front: bool,
    pub comment: Option<String>,
    /// The original upload, which can be large.
    pub image: String,
    /// Thumbnail URLs by width in px (250, 500, 1200): those it has.
    pub thumbnails: BTreeMap<u32, String>,
}

/// The archive's pictures the user can choose for album `album_id`: those
/// of the release it's matched to, in the archive's order, then its release
/// group's front cover. Empty if the album isn't matched. A picture is
/// chosen by its `COVER_SIZE` thumbnail (else the nearest it has) and
/// previewed by its `PREVIEW_SIZE` one.
pub fn cover_candidates(
    client: &Client,
    conn: &Connection,
    album_id: i64,
) -> Result<Vec<CoverCandidate>, Error> {
    let Some(link) = albums::album_link(conn, album_id, SourceId::MusicBrainz)? else {
        return Ok(Vec::new());
    };
    let Some(release_id) = matched_release(&link) else {
        return Ok(Vec::new());
    };
    let mut candidates: Vec<CoverCandidate> = release_images(client, conn, release_id)?
        .into_iter()
        .map(|image| {
            let thumbnail = |sizes: &[u32]| {
                sizes
                    .iter()
                    .find_map(|size| image.thumbnails.get(size))
                    .cloned()
                    .unwrap_or_else(|| image.image.clone())
            };
            CoverCandidate {
                source: SourceId::CoverArtArchive,
                reference: Some(thumbnail(&[COVER_SIZE, 1200, PREVIEW_SIZE])),
                label: match image.types.is_empty() {
                    true => "Picture".into(),
                    false => image.types.join(", "),
                },
                detail: image.comment.clone(),
                preview: Some(thumbnail(&[PREVIEW_SIZE, COVER_SIZE, 1200])),
            }
        })
        .collect();
    if let Some(group) = link
        .release
        .as_ref()
        .and_then(|release| release.release_group_id.as_deref())
        .filter(|id| is_mbid(id))
    {
        candidates.push(CoverCandidate {
            source: SourceId::CoverArtArchive,
            reference: Some(release_group_front_url(group)),
            label: "Front".into(),
            detail: Some("The album's cover, from any of its releases".into()),
            preview: Some(format!("{BASE}/release-group/{group}/front-{PREVIEW_SIZE}")),
        });
    }
    Ok(candidates)
}

/// The pictures of release `release_id`, in the archive's order (empty if it
/// has none). The listing is cached like other JSON.
pub fn release_images(
    client: &Client,
    conn: &Connection,
    release_id: &str,
) -> Result<Vec<Image>, Error> {
    if !is_mbid(release_id) {
        return Err(Error::Invalid(format!(
            "Not a MusicBrainz id: {release_id}"
        )));
    }
    match client.get_json(conn, &listing_url(release_id), LISTING_MAX_AGE) {
        Ok(json) => parse_listing(&json),
        Err(Error::Status { status: 404, .. }) => Ok(Vec::new()),
        Err(error) => Err(error),
    }
}

pub fn parse_listing(json: &str) -> Result<Vec<Image>, Error> {
    #[derive(Deserialize)]
    struct RawListing {
        #[serde(default)]
        images: Vec<RawImage>,
    }
    #[derive(Deserialize)]
    struct RawImage {
        // A number in some listings, a string in others.
        id: Value,
        #[serde(default)]
        types: Vec<String>,
        #[serde(default)]
        front: bool,
        comment: Option<String>,
        image: String,
        #[serde(default)]
        thumbnails: BTreeMap<String, String>,
    }
    let raw: RawListing = serde_json::from_str(json).map_err(|error| {
        Error::Invalid(format!("Unexpected Cover Art Archive listing: {error}"))
    })?;
    Ok(raw
        .images
        .into_iter()
        .map(|image| {
            let mut thumbnails = BTreeMap::new();
            for (key, url) in image.thumbnails {
                // Older pictures only have "small" (250) and "large" (500).
                let width = match key.as_str() {
                    "small" => 250,
                    "large" => 500,
                    width => match width.parse() {
                        Ok(width) => width,
                        Err(_) => continue,
                    },
                };
                thumbnails.entry(width).or_insert_with(|| https(&url));
            }
            Image {
                id: match image.id {
                    Value::String(id) => id,
                    other => other.to_string(),
                },
                types: image.types,
                front: image.front,
                comment: image.comment.filter(|comment| !comment.trim().is_empty()),
                image: https(&image.image),
                thumbnails,
            }
        })
        .collect())
}

/// Listings give `http://` URLs; the archive and archive.org serve https.
fn https(url: &str) -> String {
    match url.strip_prefix("http://") {
        Some(rest) => format!("https://{rest}"),
        None => url.to_owned(),
    }
}

#[cfg(test)]
mod fixtures {
    //! Recorded listings (trimmed) for two "In Rainbows" releases from the
    //! MusicBrainz fixtures: the 2016 digital release, whose one picture
    //! predates the numbered thumbnail sizes, and the CD with several.

    pub const LISTING_2016: (&str, &str) = (
        "1a33443c-3fff-450f-8298-efbc65659d32",
        include_str!("fixtures/coverartarchive/release-1a33443c-3fff-450f-8298-efbc65659d32.json"),
    );

    pub const LISTING_CD: (&str, &str) = (
        "219e7d7c-806c-44b3-9972-cdb3614b3411",
        include_str!("fixtures/coverartarchive/release-219e7d7c-806c-44b3-9972-cdb3614b3411.json"),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::library::test_library::{track, Library};
    use crate::metadata::http::testing::{fake_client, response, FakeTransport};
    use crate::metadata::http::{Response, TransportError};
    use crate::metadata::images::testing::JPEG;
    use crate::metadata::musicbrainz::{self, fixtures::RELEASES, fixtures::RELEASE_GROUP};

    /// A library with one album (id 1), linked to MusicBrainz release
    /// `RELEASES[0]` with `status`, and a cache in a temporary folder.
    fn album(status: &str) -> (Library, tempfile::TempDir, ImageCache) {
        let library = Library::new([track("Radiohead/In Rainbows/01.flac")
            .artist("Radiohead")
            .album("In Rainbows")]);
        let (id, json) = RELEASES[0];
        link(&library, status, id, json);
        let dir = tempfile::tempdir().unwrap();
        let cache = ImageCache::new(dir.path().to_path_buf());
        (library, dir, cache)
    }

    fn link(library: &Library, status: &str, id: &str, details: &str) {
        let details = serde_json::to_string(&musicbrainz::parse_release(details).unwrap()).unwrap();
        library
            .conn
            .execute(
                "INSERT OR REPLACE INTO album_links
                     (album_id, source, status, external_id, score, chosen_by, details, checked_at)
                 VALUES (1, 'musicbrainz', ?1, ?2, 1.0, 'auto', ?3, 0)",
                params![status, id, details],
            )
            .unwrap();
    }

    fn serve_jpeg(transport: &FakeTransport, url: &str) {
        transport.push(
            url,
            Ok(Response {
                content_type: Some("image/jpeg".into()),
                body: JPEG.to_vec(),
                ..response(200, "")
            }),
        );
    }

    #[test]
    fn covers_come_from_a_matched_release_then_its_group() {
        let (library, _dir, _cache) = album("matched");
        let (id, json) = RELEASES[0];
        assert_eq!(
            cover_urls(&library.conn, 1).unwrap(),
            [
                format!("{BASE}/release/{id}/front-500"),
                format!("{BASE}/release-group/{RELEASE_GROUP}/front-500"),
            ]
        );
        // MusicBrainz says the release has no front cover: only the group's.
        let mut no_front: serde_json::Value = serde_json::from_str(json).unwrap();
        no_front["cover-art-archive"]["front"] = false.into();
        link(&library, "matched", id, &no_front.to_string());
        assert_eq!(
            cover_urls(&library.conn, 1).unwrap(),
            [release_group_front_url(RELEASE_GROUP)]
        );
        // Awaiting review, or no link at all: none.
        link(&library, "review", id, json);
        assert!(cover_urls(&library.conn, 1).unwrap().is_empty());
        assert!(cover_urls(&library.conn, 2).unwrap().is_empty());
    }

    #[test]
    fn downloads_the_release_cover_once() {
        let (library, _dir, cache) = album("matched");
        let (client, transport, _clock) = fake_client();
        let url = release_front_url(RELEASES[0].0);
        serve_jpeg(&transport, &url);
        assert_eq!(
            fetch_album_art(&client, &library.conn, &cache, 1).unwrap(),
            Fetched::Downloaded
        );
        assert_eq!(cache.get(&url), Some(("image/jpeg", JPEG.to_vec())));
        assert_eq!(
            fetch_album_art(&client, &library.conn, &cache, 1).unwrap(),
            Fetched::Cached
        );
        assert_eq!(transport.urls(), [url], "requested once");
        let check = cover_check(&library.conn, 1).unwrap().unwrap();
        assert_eq!(check.status, LinkStatus::Matched);
        assert_eq!(check.external_id.as_deref(), Some(RELEASES[0].0));
    }

    #[test]
    fn falls_back_to_the_release_group_cover() {
        let (library, _dir, cache) = album("matched");
        let (client, transport, _clock) = fake_client();
        // The release's cover is a 404 (the fake's answer to anything not
        // scripted).
        let group = release_group_front_url(RELEASE_GROUP);
        serve_jpeg(&transport, &group);
        assert_eq!(
            fetch_album_art(&client, &library.conn, &cache, 1).unwrap(),
            Fetched::Downloaded
        );
        assert_eq!(transport.urls()[1], group);
        assert!(cache.contains(&group));
        assert!(!cache.contains(&release_front_url(RELEASES[0].0)));

        // Neither has one: recorded, for the release asked about.
        let (library, _dir, cache) = album("matched");
        assert_eq!(cover_check(&library.conn, 1).unwrap(), None);
        assert_eq!(
            fetch_album_art(&client, &library.conn, &cache, 1).unwrap(),
            Fetched::NotFound
        );
        let check = cover_check(&library.conn, 1).unwrap().unwrap();
        assert_eq!(
            (check.status, check.external_id.as_deref(), check.score),
            (LinkStatus::None, Some(RELEASES[0].0), 0.0)
        );
        assert!(check.checked_at > 0);
    }

    #[test]
    fn does_nothing_for_albums_not_matched() {
        let (client, transport, _clock) = fake_client();
        let (library, _dir, cache) = album("review");
        assert_eq!(
            fetch_album_art(&client, &library.conn, &cache, 1).unwrap(),
            Fetched::NotLinked
        );
        library.conn.execute("DELETE FROM album_links", []).unwrap();
        assert_eq!(
            fetch_album_art(&client, &library.conn, &cache, 1).unwrap(),
            Fetched::NotLinked
        );
        assert!(transport.urls().is_empty());
    }

    #[test]
    fn fetches_the_users_choice() {
        let (library, _dir, cache) = album("matched");
        let (client, transport, _clock) = fake_client();
        let chosen = format!(
            "{BASE}/release/{}/1931675364-500.jpg",
            fixtures::LISTING_CD.0
        );
        library
            .conn
            .execute(
                "INSERT INTO album_art (album_id, source, reference)
                 VALUES (1, 'cover-art-archive', ?1)",
                [&chosen],
            )
            .unwrap();
        serve_jpeg(&transport, &chosen);
        assert_eq!(
            fetch_album_art(&client, &library.conn, &cache, 1).unwrap(),
            Fetched::Downloaded
        );
        assert_eq!(transport.urls(), std::slice::from_ref(&chosen));
        assert!(cache.contains(&chosen));
        assert_eq!(cover_check(&library.conn, 1).unwrap(), None, "not recorded");

        // A reference that isn't the archive's is never requested.
        library
            .conn
            .execute(
                "UPDATE album_art SET reference = 'https://example.com/x.jpg'",
                [],
            )
            .unwrap();
        let _ = fetch_album_art(&client, &library.conn, &cache, 1);
        assert!(!transport.urls().iter().any(|url| url.contains("example")));
    }

    #[test]
    fn offline_changes_nothing() {
        let (library, dir, cache) = album("matched");
        let (client, transport, _clock) = fake_client();
        transport.push(
            &release_front_url(RELEASES[0].0),
            Err(TransportError::Unreachable("down".into())),
        );
        let error = fetch_album_art(&client, &library.conn, &cache, 1).unwrap_err();
        assert!(matches!(error, Error::Offline(_)), "{error}");
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0);
        // The host is backing off: refused at once, still nothing stored.
        let error = fetch_album_art(&client, &library.conn, &cache, 1).unwrap_err();
        assert!(matches!(error, Error::Offline(_)), "{error}");
        assert_eq!(transport.urls().len(), 1);
        assert_eq!(cover_check(&library.conn, 1).unwrap(), None);
    }

    #[test]
    fn hosts_are_those_of_the_urls() {
        assert!(release_front_url("x").starts_with(&format!("https://{HOST}/")));
        assert!(
            musicbrainz::release_url("x").starts_with(&format!("https://{}/", musicbrainz::HOST))
        );
    }

    #[test]
    fn refuses_a_response_that_is_not_an_image() {
        let (library, dir, cache) = album("matched");
        let (client, transport, _clock) = fake_client();
        transport.push_status(&release_front_url(RELEASES[0].0), 200, "<html>Oops</html>");
        assert!(fetch_album_art(&client, &library.conn, &cache, 1).is_err());
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0);
    }

    #[test]
    fn offers_the_matched_releases_pictures_and_its_groups_front() {
        let (library, _dir, _cache) = album("review");
        let (client, transport, _clock) = fake_client();
        assert!(cover_candidates(&client, &library.conn, 1)
            .unwrap()
            .is_empty());
        assert!(transport.urls().is_empty(), "only for a match");

        let (id, json) = RELEASES[3];
        assert_eq!(id, fixtures::LISTING_CD.0);
        link(&library, "matched", id, json);
        transport.push_status(&listing_url(id), 200, fixtures::LISTING_CD.1);
        let candidates = cover_candidates(&client, &library.conn, 1).unwrap();
        let labels: Vec<&str> = candidates.iter().map(|c| c.label.as_str()).collect();
        assert_eq!(labels, ["Front", "Back", "Medium", "Front"]);
        let front = &candidates[0];
        let release = format!("{BASE}/release/{id}");
        assert_eq!(
            front.reference.as_deref(),
            Some(&*format!("{release}/1931675364-500.jpg"))
        );
        assert_eq!(
            front.preview.as_deref(),
            Some(&*format!("{release}/1931675364-250.jpg"))
        );
        assert_eq!(front.detail.as_deref(), Some("digital media image"));
        let group = &candidates[3];
        assert_eq!(
            group.reference.as_deref(),
            Some(&*release_group_front_url(RELEASE_GROUP))
        );
        assert!(candidates
            .iter()
            .all(|c| c.source == SourceId::CoverArtArchive
                && c.reference.as_deref().is_some_and(is_archive_url)
                && c.preview.as_deref().is_some_and(is_archive_url)));
    }

    #[test]
    fn parses_listings() {
        let images = parse_listing(fixtures::LISTING_CD.1).unwrap();
        let types: Vec<&[String]> = images.iter().map(|image| &image.types[..]).collect();
        assert_eq!(types, [["Front"], ["Back"], ["Medium"]]);
        let front = &images[0];
        assert!(front.front && !images[1].front);
        assert_eq!(front.id, "1931675364");
        assert_eq!(front.comment.as_deref(), Some("digital media image"));
        assert_eq!(images[1].comment, None);
        assert_eq!(
            front.thumbnails.keys().copied().collect::<Vec<_>>(),
            [250, 500, 1200]
        );
        let release = format!(
            "https://coverartarchive.org/release/{}",
            fixtures::LISTING_CD.0
        );
        assert_eq!(
            front.thumbnails[&500],
            format!("{release}/1931675364-500.jpg")
        );
        assert_eq!(front.image, format!("{release}/1931675364.jpg"));

        // An older picture: an id as a string, and the named sizes.
        let images = parse_listing(fixtures::LISTING_2016.1).unwrap();
        assert_eq!(images[0].id, "14926982777");
        assert_eq!(
            images[0].thumbnails.keys().copied().collect::<Vec<_>>(),
            [250, 500]
        );
        assert!(images[0].thumbnails[&500].starts_with("https://"));

        assert!(parse_listing("not json").is_err());
        assert!(parse_listing(r#"{"images": [{"id": 1}]}"#).is_err());
        assert!(parse_listing("{}").unwrap().is_empty());
    }

    #[test]
    fn lists_a_releases_images_through_the_client_and_cache() {
        let (library, _dir, _cache) = album("matched");
        let (client, transport, _clock) = fake_client();
        let (id, json) = fixtures::LISTING_CD;
        transport.push_status(&listing_url(id), 200, json);
        assert_eq!(release_images(&client, &library.conn, id).unwrap().len(), 3);
        assert_eq!(release_images(&client, &library.conn, id).unwrap().len(), 3);
        assert_eq!(transport.urls().len(), 1);
        // No pictures at all is a 404.
        let (other, _) = fixtures::LISTING_2016;
        assert!(release_images(&client, &library.conn, other)
            .unwrap()
            .is_empty());
        assert!(release_images(&client, &library.conn, "../x").is_err());
        assert_eq!(transport.urls().len(), 2);
    }

    /// Against the real service, following its redirect to archive.org:
    /// `cargo test live_ -- --ignored`.
    #[test]
    #[ignore]
    fn live_cover_and_listing() {
        use crate::metadata::http::{SystemClock, UreqTransport};
        let conn = crate::library::db::open_in_memory().unwrap();
        let client = Client::new(Box::new(UreqTransport::new()), Box::new(SystemClock));
        let dir = tempfile::tempdir().unwrap();
        let cache = ImageCache::new(dir.path().to_path_buf());
        let (id, _) = fixtures::LISTING_CD;
        let url = release_front_url(id);
        assert_eq!(
            fetch_image(&client, &cache, &url).unwrap(),
            Fetched::Downloaded
        );
        assert_eq!(cache.get(&url).unwrap().0, "image/jpeg");
        let images = release_images(&client, &conn, id).unwrap();
        assert!(images.iter().any(|image| image.front));
        assert!(images.iter().all(|image| image
            .thumbnails
            .values()
            .all(|url| url.starts_with("https://"))));
    }
}
