//! Errors the UI shows, as codes with parameters it can translate (PLAN.md
//! F19). Such an error's text is the JSON `{"code", "params", "message"}`:
//! the UI shows its catalogue's `error.<code>` with the params filled in,
//! or `message`, the English text, when the catalogue lacks the code
//! (`errorText` in `app/src/lib/i18n`). Errors the user can't act on (a
//! database or I/O failure) stay plain English text.

use serde_json::{json, Map, Value};

/// The error `code` with `params`, and `message` for a UI without the code.
pub fn coded(code: &str, params: &[(&str, Value)], message: impl Into<String>) -> String {
    let params: Map<String, Value> = params
        .iter()
        .map(|(name, value)| ((*name).to_owned(), value.clone()))
        .collect();
    json!({ "code": code, "params": params, "message": message.into() }).to_string()
}

/// What went missing from the library while the UI still showed it.
#[derive(Clone, Copy, Debug)]
pub enum Gone {
    Track,
    Album,
    Artist,
    Playlist,
}

/// A track, album, artist or playlist that is no longer in the library.
pub fn gone(what: Gone) -> String {
    match what {
        Gone::Track => coded("trackGone", &[], "The track is no longer in the library"),
        Gone::Album => coded("albumGone", &[], "The album is no longer in the library"),
        Gone::Artist => coded("artistGone", &[], "The artist is no longer in the library"),
        Gone::Playlist => coded("playlistGone", &[], "The playlist no longer exists"),
    }
}

/// An optional feature (PLAN.md §4.6) that is turned off; `name` is its
/// English name, and `feature` its key in the feature settings.
pub fn feature_off(feature: &str, name: &str) -> String {
    coded(
        "featureOff",
        &[("feature", json!(feature)), ("name", json!(name))],
        format!("{name} is turned off in Settings › Features"),
    )
}

/// A library folder that can't be read now (PLAN.md H22); `reason` is a
/// `FolderState` code ("missing", "empty", "mostlyGone", "inTrash",
/// "noPermission"), and `detail` the system's words, when there are any.
pub fn folder_unavailable(path: &str, reason: &str, detail: Option<&str>) -> String {
    let message = match detail {
        Some(detail) => format!("Folder not available ({reason}): {path} ({detail})"),
        None => format!("Folder not available ({reason}): {path}"),
    };
    let mut params = vec![("path", json!(path)), ("reason", json!(reason))];
    if let Some(detail) = detail {
        params.push(("detail", json!(detail)));
    }
    coded("folderUnavailable", &params, message)
}

/// Whether `error` is the error code `code`.
#[cfg(test)]
pub fn is(error: &impl std::fmt::Display, code: &str) -> bool {
    serde_json::from_str::<Value>(&error.to_string()).is_ok_and(|value| value["code"] == code)
}

/// A scan is running, so this has to wait for it.
pub fn scan_running() -> String {
    coded("scanRunning", &[], "A library scan is already running")
}

/// A library folder id the library doesn't have.
pub fn no_folder(id: impl std::fmt::Debug) -> String {
    coded(
        "noFolder",
        &[("id", json!(format!("{id:?}")))],
        format!("No library folder with id {id:?}"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codes_are_json_with_their_english_text() {
        let text = coded(
            "playlistName",
            &[("max", json!(200))],
            "A playlist's name must be 1 to 200 characters",
        );
        let value: Value = serde_json::from_str(&text).unwrap();
        assert_eq!(value["code"], "playlistName");
        assert_eq!(value["params"]["max"], 200);
        assert_eq!(
            value["message"],
            "A playlist's name must be 1 to 200 characters"
        );

        let off: Value = serde_json::from_str(&feature_off("lyrics", "Lyrics")).unwrap();
        assert_eq!(off["code"], "featureOff");
        assert_eq!(off["params"]["feature"], "lyrics");
        assert_eq!(
            off["message"],
            "Lyrics is turned off in Settings › Features"
        );

        let gone: Value = serde_json::from_str(&gone(Gone::Album)).unwrap();
        assert_eq!(gone["code"], "albumGone");
        assert_eq!(gone["params"], json!({}));
    }
}
