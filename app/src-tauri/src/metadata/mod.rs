//! Metadata from sources beyond the tags: pictures next to the files, and
//! online services such as MusicBrainz (PLAN.md Phase 4). Several sources
//! can supply each kind of data; `settings` holds which are enabled and in
//! what order, and the user can pin a source's result for an album.
//!
//! Online data is kept apart from the tags: the scanner owns the library's
//! tables, and the metadata code owns `album_links`, `artist_links` and
//! `album_art` (migration 003).

// The online parts have no caller in the app until the metadata worker
// lands (PLAN.md Phase 4.5); only tests use them so far.
#[allow(dead_code)]
pub mod albums;
#[allow(dead_code)]
pub mod cache;
pub mod commands;
pub mod folder_art;
#[allow(dead_code)]
pub mod http;
#[allow(dead_code)]
pub mod matcher;
#[allow(dead_code)]
pub mod musicbrainz;
pub mod settings;

use std::fmt;

#[derive(Debug)]
#[allow(dead_code)] // Offline and Status until Phase 4.5.
pub enum Error {
    Db(rusqlite::Error),
    /// The service can't be reached (no network, DNS, timeouts), or is
    /// backing off after that happened.
    Offline(String),
    /// The service answered with an HTTP error status.
    Status {
        url: String,
        status: u16,
    },
    /// A response that couldn't be used, or a problem to show the user.
    Invalid(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Db(error) => write!(f, "Library database error: {error}"),
            Error::Offline(message) => write!(f, "Offline: {message}"),
            Error::Status { url, status } => write!(f, "HTTP {status} from {url}"),
            Error::Invalid(message) => f.write_str(message),
        }
    }
}

impl std::error::Error for Error {}

impl From<rusqlite::Error> for Error {
    fn from(error: rusqlite::Error) -> Self {
        Error::Db(error)
    }
}

impl From<crate::library::Error> for Error {
    fn from(error: crate::library::Error) -> Self {
        match error {
            crate::library::Error::Db(error) => Error::Db(error),
            crate::library::Error::Invalid(message) => Error::Invalid(message),
        }
    }
}

impl From<Error> for crate::library::Error {
    fn from(error: Error) -> Self {
        match error {
            Error::Db(error) => crate::library::Error::Db(error),
            other => crate::library::Error::Invalid(other.to_string()),
        }
    }
}
