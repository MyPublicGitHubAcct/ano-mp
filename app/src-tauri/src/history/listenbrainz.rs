//! Opt-in ListenBrainz submission (PLAN.md O8). Each play that counts is
//! queued in `listens_pending` as the JSON ListenBrainz takes, and sent in
//! batches through `http::Client` with the user's token from the keychain
//! (`metadata::keys`). While the service can't be reached, listens wait;
//! a rejected token stops sending until a new one is set.
//!
//! ListenBrainz is MetaBrainz's service, like MusicBrainz; the MusicBrainz
//! IDs in the tags make its matches exact. Its terms are checked before
//! release, like every source's (PLAN.md §8.1).

use std::time::{Duration, Instant};

use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;
use serde_json::{json, Value};

use crate::library::unix_now;
use crate::metadata::http::{Client, SystemClock, UreqTransport};
use crate::metadata::keys::{self, Account};
use crate::metadata::Error;

const SUBMIT_URL: &str = "https://api.listenbrainz.org/1/submit-listens";
const VALIDATE_URL: &str = "https://api.listenbrainz.org/1/validate-token";

/// Listens sent in one request at most (the service allows 1000).
const BATCH: usize = 100;

/// After a failure, wait this long before trying again.
const RETRY_WAIT: Duration = Duration::from_secs(300);

/// What the settings show about ListenBrainz.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ListenBrainzStatus {
    pub has_token: bool,
    /// Listens waiting to be sent.
    pub pending: u32,
    /// Why the last attempt failed, if it did.
    pub error: Option<String>,
}

/// The listen for a play of `track_id` started at `listened_at`, as
/// ListenBrainz's submit-listens payload takes it; `None` without a title
/// and artist, which it requires.
pub fn listen(conn: &Connection, track_id: i64, listened_at: i64) -> Result<Option<Value>, Error> {
    let row: Option<(
        Option<String>,
        Option<String>,
        Option<String>,
        f64,
        Option<String>,
        Option<String>,
        Option<String>,
    )> = conn
        .query_row(
            "SELECT t.title, IFNULL(t.artist_credit, artist.name), album.title, t.duration,
                    t.musicbrainz_recording_id,
                    album.musicbrainz_release_id, artist.musicbrainz_id
             FROM tracks t
             LEFT JOIN artists artist ON artist.id = t.artist_id
             LEFT JOIN albums album ON album.id = t.album_id
             WHERE t.id = ?1",
            [track_id],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                    row.get(5)?,
                    row.get(6)?,
                ))
            },
        )
        .optional()?;
    let Some((Some(title), Some(artist), album, duration, recording, release, artist_mbid)) = row
    else {
        return Ok(None);
    };
    let mut info = json!({
        "media_player": "ano-mp",
        "submission_client": "ano-mp",
        "submission_client_version": crate::anomp::version(),
        "duration_ms": (duration * 1000.0).round() as i64,
    });
    if let Some(recording) = recording {
        info["recording_mbid"] = json!(recording);
    }
    if let Some(release) = release {
        info["release_mbid"] = json!(release);
    }
    if let Some(artist_mbid) = artist_mbid {
        info["artist_mbids"] = json!([artist_mbid]);
    }
    let mut metadata = json!({
        "artist_name": artist,
        "track_name": title,
        "additional_info": info,
    });
    if let Some(album) = album {
        metadata["release_name"] = json!(album);
    }
    Ok(Some(json!({
        "listened_at": listened_at,
        "track_metadata": metadata,
    })))
}

/// Queues the listen for a play, to be sent.
pub fn queue(conn: &Connection, track_id: i64, listened_at: i64) -> Result<(), Error> {
    if let Some(listen) = listen(conn, track_id, listened_at)? {
        conn.execute(
            "INSERT INTO listens_pending (listen, queued_at) VALUES (?1, ?2)",
            params![listen.to_string(), unix_now()],
        )?;
    }
    Ok(())
}

pub fn pending_count(conn: &Connection) -> Result<u32, Error> {
    Ok(conn.query_row("SELECT count(*) FROM listens_pending", [], |row| row.get(0))?)
}

/// Sends up to `BATCH` pending listens with `client`; returns how many
/// went. Sent listens leave the queue.
pub fn send_batch(conn: &Connection, client: &Client, token: &str) -> Result<usize, Error> {
    let mut statement =
        conn.prepare_cached("SELECT id, listen FROM listens_pending ORDER BY id LIMIT ?1")?;
    let rows: Vec<(i64, String)> = statement
        .query_map([BATCH as i64], |row| Ok((row.get(0)?, row.get(1)?)))?
        .collect::<Result<_, _>>()?;
    if rows.is_empty() {
        return Ok(0);
    }
    let payload: Vec<Value> = rows
        .iter()
        .filter_map(|(_, listen)| serde_json::from_str(listen).ok())
        .collect();
    let body = json!({
        "listen_type": if payload.len() == 1 { "single" } else { "import" },
        "payload": payload,
    });
    client.post_json(
        SUBMIT_URL,
        &body.to_string(),
        Some(&format!("Token {token}")),
    )?;
    let last = rows.last().map_or(0, |(id, _)| *id);
    conn.execute("DELETE FROM listens_pending WHERE id <= ?1", [last])?;
    Ok(rows.len())
}

/// Checks a token with ListenBrainz; returns the user it belongs to.
pub fn validate_token(client: &Client, token: &str) -> Result<String, Error> {
    let response = client.get_with_auth(
        VALIDATE_URL,
        "application/json",
        Some(&format!("Token {token}")),
        crate::metadata::http::JSON_LIMIT,
    )?;
    let body: Value = serde_json::from_slice(&response.body)
        .map_err(|_| Error::Invalid("ListenBrainz's answer wasn't JSON".into()))?;
    if body["valid"].as_bool() == Some(true) {
        Ok(body["user_name"].as_str().unwrap_or_default().to_owned())
    } else {
        Err(Error::Invalid(
            body["message"]
                .as_str()
                .unwrap_or("ListenBrainz doesn't know that token")
                .to_owned(),
        ))
    }
}

/// The history thread's sending state.
pub struct Sender {
    client: Option<Client>,
    retry_at: Option<Instant>,
    error: Option<String>,
}

impl Default for Sender {
    fn default() -> Self {
        Self::new()
    }
}

impl Sender {
    pub fn new() -> Sender {
        Sender {
            client: None,
            retry_at: None,
            error: None,
        }
    }

    /// How long the history thread may sleep.
    pub fn wait(&self) -> Duration {
        self.retry_at.map_or(super::IDLE_WAIT, |at| {
            at.saturating_duration_since(Instant::now())
                .max(Duration::from_secs(1))
        })
    }

    pub fn retry_now(&mut self) {
        self.retry_at = None;
        self.error = None;
    }

    /// Sends what's pending, unless waiting after a failure.
    pub fn send_pending(&mut self, conn: &Connection) {
        if self.retry_at.is_some_and(|at| Instant::now() < at) {
            return;
        }
        self.retry_at = None;
        let token = match keys::get(Account::ListenBrainz) {
            Ok(Some(token)) => token,
            Ok(None) => return,
            Err(error) => {
                self.fail(error.to_string());
                return;
            }
        };
        let client = self.client.get_or_insert_with(|| {
            Client::new(Box::new(UreqTransport::new()), Box::new(SystemClock))
        });
        loop {
            match send_batch(conn, client, &token) {
                Ok(0) => {
                    self.error = None;
                    return;
                }
                Ok(_) => continue,
                Err(error) => {
                    self.fail(error.to_string());
                    return;
                }
            }
        }
    }

    fn fail(&mut self, error: String) {
        eprintln!("[listenbrainz] {error}");
        self.error = Some(error);
        self.retry_at = Some(Instant::now() + RETRY_WAIT);
    }
}

/// Status for the settings: whether a token is stored and how many listens wait.
pub fn status(conn: &Connection) -> Result<ListenBrainzStatus, Error> {
    Ok(ListenBrainzStatus {
        has_token: keys::get(Account::ListenBrainz)?.is_some(),
        pending: pending_count(conn)?,
        error: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::library::test_library::{track, Library};
    use crate::metadata::http::testing::fake_client;

    #[test]
    fn builds_listens_with_their_ids() {
        let library = Library::new([track("a.flac").title("Song").artist("Band").album("Record")]);
        library
            .conn
            .execute_batch(
                "UPDATE tracks SET musicbrainz_recording_id = 'rec', duration = 61.25;
                 UPDATE artists SET musicbrainz_id = 'art';",
            )
            .unwrap();
        let listen = listen(&library.conn, 1, 1700000000).unwrap().unwrap();
        assert_eq!(listen["listened_at"], 1700000000);
        let metadata = &listen["track_metadata"];
        assert_eq!(metadata["track_name"], "Song");
        assert_eq!(metadata["artist_name"], "Band");
        assert_eq!(metadata["release_name"], "Record");
        assert_eq!(metadata["additional_info"]["recording_mbid"], "rec");
        assert_eq!(metadata["additional_info"]["artist_mbids"], json!(["art"]));
        assert_eq!(metadata["additional_info"]["duration_ms"], 61250);

        // Without an artist ListenBrainz can't take it.
        let untitled = Library::new([track("b.flac").title("Song")]);
        assert_eq!(super::listen(&untitled.conn, 1, 0).unwrap(), None);
    }

    #[test]
    fn sends_pending_listens_in_batches() {
        let library = Library::new([track("a.flac").title("Song").artist("Band")]);
        for at in 0..3 {
            queue(&library.conn, 1, 1000 + at).unwrap();
        }
        assert_eq!(pending_count(&library.conn).unwrap(), 3);
        let (client, transport, _) = fake_client();
        transport.push_status(SUBMIT_URL, 200, r#"{"status": "ok"}"#);
        assert_eq!(send_batch(&library.conn, &client, "secret").unwrap(), 3);
        assert_eq!(pending_count(&library.conn).unwrap(), 0);
        let body: Value = serde_json::from_str(&transport.bodies.lock().unwrap()[0]).unwrap();
        assert_eq!(body["listen_type"], "import");
        assert_eq!(body["payload"].as_array().unwrap().len(), 3);
        assert_eq!(
            transport.auths.lock().unwrap()[0].as_deref(),
            Some("Token secret")
        );

        // A refusal keeps them for later.
        queue(&library.conn, 1, 2000).unwrap();
        transport.push_status(SUBMIT_URL, 401, r#"{"message": "Invalid token"}"#);
        assert!(send_batch(&library.conn, &client, "bad").is_err());
        assert_eq!(pending_count(&library.conn).unwrap(), 1);
    }

    #[test]
    fn validates_tokens() {
        let (client, transport, _) = fake_client();
        transport.push_status(VALIDATE_URL, 200, r#"{"valid": true, "user_name": "me"}"#);
        assert_eq!(validate_token(&client, "t").unwrap(), "me");
        transport.push_status(VALIDATE_URL, 200, r#"{"valid": false, "message": "Nope"}"#);
        assert_eq!(
            validate_token(&client, "t").unwrap_err().to_string(),
            "Nope"
        );
    }
}
