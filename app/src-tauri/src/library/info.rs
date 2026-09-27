//! "Get Info" for a track (PLAN.md F16): everything its file says (every
//! tag field TagLib reads, the kinds of tag, the embedded pictures), the
//! format as the decoder sees it (the facts O10's signal path shows too),
//! where the file is, and its MusicBrainz ids for links. Read only.

use std::path::Path;

use base64::Engine;
use rusqlite::OptionalExtension;
use serde::Serialize;

use super::access::open_folder;
use super::commands::LibraryState;
use super::{track_from_row, track_path, Error, TrackSummary, TRACKS_FROM, TRACK_COLUMNS};
use crate::anomp::{self, FileInfo};

/// Pictures larger than this are listed without their data.
const MAX_PICTURE: usize = 8 << 20;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TrackDetails {
    pub track: TrackSummary,
    /// Where the file is: its library folder, and the path in it.
    pub folder: String,
    pub relative_path: String,
    /// The part of the file the track is (a cue sheet's track, a chapter):
    /// where it ends, in seconds, if before the end.
    pub range_end: Option<f64>,
    pub file: FileInfo,
    pub pictures: Vec<PictureView>,
    pub musicbrainz: MusicBrainzIds,
    /// Why the file couldn't be read, e.g. its drive isn't there; the
    /// library's facts still show.
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PictureView {
    pub kind: String,
    pub mime_type: String,
    pub description: String,
    pub size: usize,
    /// A data: URL of the picture; `None` if too large to show.
    pub data_url: Option<String>,
}

/// MusicBrainz ids from the tags, as MusicBrainz's pages take them.
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MusicBrainzIds {
    pub recording: Option<String>,
    pub release: Option<String>,
    pub release_group: Option<String>,
    pub release_track: Option<String>,
    pub artists: Vec<String>,
    pub album_artists: Vec<String>,
    pub work: Option<String>,
}

impl MusicBrainzIds {
    fn from_fields(fields: &[(String, String)]) -> MusicBrainzIds {
        let values = |key: &str| -> Vec<String> {
            fields
                .iter()
                .filter(|(field, _)| field == key)
                .flat_map(|(_, value)| anomp::split_artists(value))
                .filter(|id| is_mbid(id))
                .collect()
        };
        let one = |key: &str| values(key).into_iter().next();
        MusicBrainzIds {
            recording: one("MUSICBRAINZ_TRACKID"),
            release: one("MUSICBRAINZ_ALBUMID"),
            release_group: one("MUSICBRAINZ_RELEASEGROUPID"),
            release_track: one("MUSICBRAINZ_RELEASETRACKID"),
            artists: values("MUSICBRAINZ_ARTISTID"),
            album_artists: values("MUSICBRAINZ_ALBUMARTISTID"),
            work: one("MUSICBRAINZ_WORKID"),
        }
    }
}

/// A MusicBrainz id: a UUID in its usual form, so it is safe in a URL.
fn is_mbid(text: &str) -> bool {
    text.len() == 36
        && text.char_indices().all(|(index, c)| match index {
            8 | 13 | 18 | 23 => c == '-',
            _ => c.is_ascii_hexdigit(),
        })
}

/// Track `track_id`'s details, reading its file with its folder open; `None`
/// if there is no such track.
pub fn track_details(library: &LibraryState, track_id: i64) -> Result<Option<TrackDetails>, Error> {
    let row = {
        let conn = library.conn();
        conn.query_row(
            &format!(
                "SELECT {TRACK_COLUMNS}, t.folder_id, t.range_end {TRACKS_FROM} WHERE t.id = ?1"
            ),
            [track_id],
            |row| {
                Ok((
                    track_from_row(row)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, i64>(27)?,
                    row.get::<_, Option<f64>>(28)?,
                ))
            },
        )
        .optional()?
    };
    let Some((track, folder, relative_path, folder_id, range_end)) = row else {
        return Ok(None);
    };
    let path = track_path(Path::new(&folder), &relative_path);
    let read = {
        let conn = library.conn();
        let open = open_folder(&conn, folder_id);
        drop(conn);
        match open {
            // Readable while `_open` lives.
            Ok(_open) => anomp::read_file_info(&path),
            Err(error) => Err(error.to_string()),
        }
    };
    let (file, error) = match read {
        Ok(file) => (file, None),
        Err(error) => (
            FileInfo {
                fields: Vec::new(),
                pictures: Vec::new(),
                tag_types: String::new(),
                codec: String::new(),
                lossless: false,
                bits_per_sample: None,
                bitrate_kbps: None,
                sample_rate: f64::from(track.sample_rate),
                channels: 0,
                duration: track.duration,
                file_size: 0,
            },
            Some(error),
        ),
    };
    let pictures = file
        .pictures
        .iter()
        .map(|picture| PictureView {
            kind: picture.kind.clone(),
            mime_type: picture.mime_type.clone(),
            description: picture.description.clone(),
            size: picture.data.len(),
            data_url: (picture.data.len() <= MAX_PICTURE
                && matches!(
                    picture.mime_type.as_str(),
                    "image/jpeg" | "image/png" | "image/gif" | "image/webp" | "image/bmp"
                ))
            .then(|| {
                format!(
                    "data:{};base64,{}",
                    picture.mime_type,
                    base64::engine::general_purpose::STANDARD.encode(&picture.data)
                )
            }),
        })
        .collect();
    Ok(Some(TrackDetails {
        musicbrainz: MusicBrainzIds::from_fields(&file.fields),
        track,
        folder,
        relative_path,
        range_end,
        pictures,
        file,
        error,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::library::{add_folder, db, scanner};

    #[test]
    fn details_of_a_scanned_file() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("Music");
        std::fs::create_dir(&root).unwrap();
        std::fs::copy(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../core/tests/fixtures/tagged-vorbis.flac"),
            root.join("a.flac"),
        )
        .unwrap();
        let mut conn = db::open_in_memory().unwrap();
        let folder = add_folder(&conn, &root).unwrap();
        scanner::scan_folder(
            &mut conn,
            folder.id,
            scanner::ScanOptions::default(),
            |_| {},
        )
        .unwrap();
        let id: i64 = conn
            .query_row("SELECT id FROM tracks", [], |row| row.get(0))
            .unwrap();
        let library = LibraryState::for_tests(conn);

        let details = track_details(&library, id).unwrap().unwrap();
        assert_eq!(details.relative_path, "a.flac");
        assert_eq!(details.file.codec, "flac");
        assert!(details.error.is_none());
        assert_eq!(
            details.musicbrainz.release.as_deref(),
            Some("a1b2c3d4-0000-4000-8000-000000000001")
        );
        assert!(!details.pictures.is_empty());
        assert!(details.pictures[0]
            .data_url
            .as_deref()
            .is_some_and(|url| url.starts_with("data:image/")));
        assert!(track_details(&library, id + 1).unwrap().is_none());

        // The file gone: the library's facts, and why the rest are missing.
        std::fs::remove_file(root.join("a.flac")).unwrap();
        let details = track_details(&library, id).unwrap().unwrap();
        assert!(details.error.is_some());
        assert_eq!(details.track.id, id);
    }

    #[test]
    fn only_real_mbids_become_links() {
        assert!(is_mbid("89ad4ac3-39f7-470e-963a-56509c546377"));
        assert!(!is_mbid("89ad4ac3-39f7-470e-963a-56509c54637"));
        assert!(!is_mbid("javascript:alert(1)//aaaaaaaaaaaaaaaaaaaaaaaaa"));
        let ids = MusicBrainzIds::from_fields(&[
            (
                "MUSICBRAINZ_ARTISTID".into(),
                "89ad4ac3-39f7-470e-963a-56509c546377; nonsense".into(),
            ),
            ("MUSICBRAINZ_TRACKID".into(), "not an id".into()),
        ]);
        assert_eq!(ids.artists.len(), 1);
        assert!(ids.recording.is_none());
    }
}
