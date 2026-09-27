//! The LAN remote (PLAN.md O14): controls the app from a phone's browser on
//! the same network, with nothing to install. Off by default.
//!
//! Security (a listening socket is attack surface):
//! - It listens only while the feature is on, and answers only peers with
//!   local addresses (loopback, private, link-local, unique-local); others
//!   are dropped without a response.
//! - A phone pairs with a six-digit code shown in Settings, valid for five
//!   minutes and for one pairing. Pairing attempts are limited per address
//!   (five failures lock it out for five minutes). A paired phone gets a
//!   random 256-bit token; the library keeps only its SHA-256, and
//!   Settings can forget a phone.
//! - Every API call needs the token. The page itself, and cover art for
//!   albums, are the only things served besides the API: no file paths,
//!   no library files, nothing written but the queue and the volume.
//! - Requests are bounded (8 KB of headers, 16 KB of body, 10 s to arrive)
//!   and at most `MAX_CONNECTIONS` are served at once.
//!
//! Commands go through the same queue functions as the UI and the media
//! keys.

pub mod http;

use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr, TcpListener, TcpStream, UdpSocket};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use ring::digest::{digest, SHA256};
use ring::rand::{SecureRandom, SystemRandom};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tauri::{AppHandle, Manager, Runtime};

use crate::library::commands::LibraryState;
use crate::library::{unix_now, Error};
use crate::settings::FeatureSettings;
use http::{Request, Response};

/// The page phones load.
const PAGE: &str = include_str!("remote.html");

/// A pairing code lasts this long.
const CODE_LIFETIME: Duration = Duration::from_secs(300);
/// Failed pairings from one address before it's locked out, and for how long.
const MAX_FAILURES: u32 = 5;
const LOCKOUT: Duration = Duration::from_secs(300);
/// Connections served at once.
const MAX_CONNECTIONS: usize = 16;
const READ_TIMEOUT: Duration = Duration::from_secs(10);

// ---- Pairing and tokens --------------------------------------------------------

/// Pairing codes and failed attempts; apart from sockets, for tests.
#[derive(Default)]
pub struct Pairing {
    code: Option<(String, Instant)>,
    failures: HashMap<IpAddr, (u32, Instant)>,
}

#[derive(Debug, PartialEq)]
pub enum PairError {
    LockedOut,
    WrongCode,
}

impl Pairing {
    /// A new code, replacing any before it.
    pub fn new_code(&mut self, random: &dyn SecureRandom, now: Instant) -> String {
        let mut bytes = [0u8; 4];
        let _ = random.fill(&mut bytes);
        let code = format!("{:06}", u32::from_le_bytes(bytes) % 1_000_000);
        self.code = Some((code.clone(), now + CODE_LIFETIME));
        code
    }

    /// The code shown now, if one is valid.
    pub fn current(&self, now: Instant) -> Option<&str> {
        self.code
            .as_ref()
            .filter(|(_, until)| now < *until)
            .map(|(code, _)| code.as_str())
    }

    /// Checks a code from `peer`; a right one is used up.
    pub fn check(&mut self, peer: IpAddr, code: &str, now: Instant) -> Result<(), PairError> {
        if let Some((count, since)) = self.failures.get(&peer) {
            if *count >= MAX_FAILURES && now.duration_since(*since) < LOCKOUT {
                return Err(PairError::LockedOut);
            }
        }
        let right = self
            .current(now)
            .is_some_and(|expected| constant_eq(expected, code.trim()));
        if right {
            self.code = None;
            self.failures.remove(&peer);
            return Ok(());
        }
        let entry = self.failures.entry(peer).or_insert((0, now));
        if now.duration_since(entry.1) >= LOCKOUT {
            *entry = (0, now);
        }
        entry.0 += 1;
        Err(PairError::WrongCode)
    }
}

fn constant_eq(a: &str, b: &str) -> bool {
    a.len() == b.len()
        && a.bytes()
            .zip(b.bytes())
            .fold(0, |acc, (x, y)| acc | (x ^ y))
            == 0
}

fn hash(token: &str) -> Vec<u8> {
    digest(&SHA256, token.as_bytes()).as_ref().to_vec()
}

/// Makes a token for a newly paired phone, keeping only its hash.
pub fn add_device(
    conn: &Connection,
    name: &str,
    random: &dyn SecureRandom,
) -> Result<String, Error> {
    let mut bytes = [0u8; 32];
    random
        .fill(&mut bytes)
        .map_err(|_| Error::Invalid("No randomness for a token".into()))?;
    let token: String = bytes.iter().map(|byte| format!("{byte:02x}")).collect();
    let name: String = name.trim().chars().take(64).collect();
    conn.execute(
        "INSERT INTO remote_devices (name, token_hash, paired_at) VALUES (?1, ?2, ?3)",
        params![
            if name.is_empty() { "Phone" } else { &name },
            hash(&token),
            unix_now()
        ],
    )?;
    Ok(token)
}

/// The paired phone a token belongs to, noting that it was seen.
pub fn check_token(conn: &Connection, token: &str) -> Result<Option<i64>, Error> {
    let id: Option<i64> = conn
        .query_row(
            "SELECT id FROM remote_devices WHERE token_hash = ?1",
            [hash(token)],
            |row| row.get(0),
        )
        .optional()?;
    if let Some(id) = id {
        conn.execute(
            "UPDATE remote_devices SET last_seen = ?1 WHERE id = ?2",
            params![unix_now(), id],
        )?;
    }
    Ok(id)
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteDevice {
    pub id: i64,
    pub name: String,
    pub paired_at: i64,
    pub last_seen: Option<i64>,
}

pub fn devices(conn: &Connection) -> Result<Vec<RemoteDevice>, Error> {
    let mut statement = conn.prepare(
        "SELECT id, name, paired_at, last_seen FROM remote_devices ORDER BY paired_at DESC",
    )?;
    let rows = statement.query_map([], |row| {
        Ok(RemoteDevice {
            id: row.get(0)?,
            name: row.get(1)?,
            paired_at: row.get(2)?,
            last_seen: row.get(3)?,
        })
    })?;
    Ok(rows.collect::<Result<_, _>>()?)
}

// ---- The server -----------------------------------------------------------------

struct Running {
    port: u16,
    stop: Arc<AtomicBool>,
}

/// The remote's state, managed by Tauri.
pub struct RemoteServer {
    running: Mutex<Option<Running>>,
    pairing: Mutex<Pairing>,
    error: Mutex<Option<String>>,
    random: SystemRandom,
}

impl Default for RemoteServer {
    fn default() -> Self {
        RemoteServer {
            running: Mutex::new(None),
            pairing: Mutex::new(Pairing::default()),
            error: Mutex::new(None),
            random: SystemRandom::new(),
        }
    }
}

impl RemoteServer {
    fn pairing(&self) -> MutexGuard<'_, Pairing> {
        self.pairing.lock().unwrap_or_else(|e| e.into_inner())
    }
}

/// What Settings shows about the remote.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteStatus {
    pub running: bool,
    /// Where phones open the page, e.g. "http://192.168.1.20:8765".
    pub url: Option<String>,
    /// Why it isn't running when it should be.
    pub error: Option<String>,
    /// The pairing code, while one is valid.
    pub code: Option<String>,
    pub devices: Vec<RemoteDevice>,
}

pub fn init<R: Runtime>(app: &AppHandle<R>) {
    app.manage(RemoteServer::default());
    configure(app, &crate::settings::current(app).features);
}

pub fn shutdown<R: Runtime>(app: &AppHandle<R>) {
    if let Some(server) = app.try_state::<RemoteServer>() {
        if let Some(running) = server
            .running
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .take()
        {
            running.stop.store(true, Ordering::SeqCst);
        }
    }
}

/// Starts, stops or moves the server to follow the settings.
pub fn configure<R: Runtime>(app: &AppHandle<R>, features: &FeatureSettings) {
    let Some(server) = app.try_state::<RemoteServer>() else {
        return;
    };
    let mut running = server.running.lock().unwrap_or_else(|e| e.into_inner());
    let wanted = features.remote_control.then_some(features.remote_port);
    if running.as_ref().map(|r| r.port) == wanted {
        return;
    }
    if let Some(old) = running.take() {
        old.stop.store(true, Ordering::SeqCst);
    }
    *server.error.lock().unwrap_or_else(|e| e.into_inner()) = None;
    let Some(port) = wanted else {
        return;
    };
    match TcpListener::bind(SocketAddr::from(([0, 0, 0, 0], port))) {
        Ok(listener) => {
            let stop = Arc::new(AtomicBool::new(false));
            let thread_stop = stop.clone();
            let thread_app = app.clone();
            let _ = listener.set_nonblocking(true);
            let spawned = std::thread::Builder::new()
                .name("remote".into())
                .spawn(move || serve(&thread_app, &listener, &thread_stop));
            match spawned {
                Ok(_) => *running = Some(Running { port, stop }),
                Err(error) => {
                    *server.error.lock().unwrap_or_else(|e| e.into_inner()) =
                        Some(error.to_string())
                }
            }
        }
        Err(error) => {
            eprintln!("[remote] cannot listen on port {port}: {error}");
            *server.error.lock().unwrap_or_else(|e| e.into_inner()) =
                Some(format!("Port {port} can't be used: {error}"));
        }
    }
}

fn serve<R: Runtime>(app: &AppHandle<R>, listener: &TcpListener, stop: &AtomicBool) {
    let active = Arc::new(AtomicUsize::new(0));
    while !stop.load(Ordering::SeqCst) {
        match listener.accept() {
            Ok((stream, peer)) => {
                // Not local, or too busy: dropped without an answer.
                if !http::is_local(peer.ip()) || active.load(Ordering::SeqCst) >= MAX_CONNECTIONS {
                    continue;
                }
                active.fetch_add(1, Ordering::SeqCst);
                let (app, done) = (app.clone(), active.clone());
                let spawned = std::thread::Builder::new()
                    .name("remote-connection".into())
                    .spawn(move || {
                        handle(&app, stream, peer.ip());
                        done.fetch_sub(1, Ordering::SeqCst);
                    });
                if spawned.is_err() {
                    active.fetch_sub(1, Ordering::SeqCst);
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(100));
            }
            Err(error) => {
                eprintln!("[remote] {error}");
                std::thread::sleep(Duration::from_millis(500));
            }
        }
    }
}

fn handle<R: Runtime>(app: &AppHandle<R>, stream: TcpStream, peer: IpAddr) {
    let _ = stream.set_nonblocking(false);
    let _ = stream.set_read_timeout(Some(READ_TIMEOUT));
    let _ = stream.set_write_timeout(Some(READ_TIMEOUT));
    let response = match http::read_request(&stream) {
        Ok(request) => route(app, &request, peer),
        Err(status) => Response::error(status, "Bad request"),
    };
    let _ = response.write(&stream);
}

fn route<R: Runtime>(app: &AppHandle<R>, request: &Request, peer: IpAddr) -> Response {
    let Some(server) = app.try_state::<RemoteServer>() else {
        return Response::error(503, "Not running");
    };
    let Some(library) = app.try_state::<LibraryState>() else {
        return Response::error(503, "The library is not available");
    };
    match (request.method.as_str(), request.path.as_str()) {
        ("GET", "/") => Response {
            status: 200,
            content_type: "text/html; charset=utf-8",
            body: PAGE.as_bytes().to_vec(),
        },
        ("POST", "/api/pair") => {
            let body: Value = serde_json::from_slice(&request.body).unwrap_or_default();
            let code = body["code"].as_str().unwrap_or("");
            let name = body["name"].as_str().unwrap_or("Phone");
            match server.pairing().check(peer, code, Instant::now()) {
                Ok(()) => match add_device(&library.conn(), name, &server.random) {
                    Ok(token) => Response::json(&json!({ "token": token })),
                    Err(error) => Response::error(500, &error.to_string()),
                },
                Err(PairError::LockedOut) => {
                    Response::error(429, "Too many tries; wait five minutes")
                }
                Err(PairError::WrongCode) => Response::error(403, "That code isn't right"),
            }
        }
        (method, path) if path.starts_with("/api/") => {
            let authorised = request
                .bearer()
                .map(|token| check_token(&library.conn(), token))
                .transpose();
            match authorised {
                Ok(Some(Some(_))) => {}
                Ok(_) => return Response::error(401, "Pair this phone first"),
                Err(error) => return Response::error(500, &error.to_string()),
            }
            drop(library);
            api(app, method, path, request)
        }
        (_, path) if path.starts_with("/art/album/") => {
            // Only with a token, passed as a query (images can't send headers).
            let token = request.query.get("token").map(String::as_str).unwrap_or("");
            if !matches!(check_token(&library.conn(), token), Ok(Some(_))) {
                return Response::error(401, "Pair this phone first");
            }
            let Ok(id) = path.trim_start_matches("/art/album/").parse::<i64>() else {
                return Response::error(404, "No such album");
            };
            match crate::library::art::lookup(&library, crate::library::art::ArtKey::Album(id)) {
                Ok(Some(art)) => Response {
                    status: 200,
                    content_type: match art.mime_type.as_str() {
                        "image/png" => "image/png",
                        _ => "image/jpeg",
                    },
                    body: art.data.clone(),
                },
                _ => Response::error(404, "No cover"),
            }
        }
        _ => Response::error(404, "Not found"),
    }
}

#[derive(Deserialize)]
#[serde(tag = "command", rename_all = "camelCase")]
enum Command {
    Toggle,
    Next,
    Previous,
    Seek { position: f64 },
    Volume { value: f64 },
    Jump { uid: u64 },
    Shuffle { on: bool },
    PlayAlbum { album_id: i64 },
    PlayTrack { track_id: i64 },
}

fn api<R: Runtime>(app: &AppHandle<R>, method: &str, path: &str, request: &Request) -> Response {
    let result: Result<Value, String> = match (method, path) {
        ("GET", "/api/state") => state(app),
        ("GET", "/api/search") => {
            let query = request.query.get("q").cloned().unwrap_or_default();
            app.try_state::<LibraryState>()
                .ok_or_else(|| "The library is not available".to_string())
                .and_then(|library| {
                    crate::library::search::search(
                        &library.conn(),
                        &query,
                        &[
                            crate::library::search::SearchKind::Albums,
                            crate::library::search::SearchKind::Tracks,
                        ],
                        0,
                        20,
                    )
                    .map_err(|e| e.to_string())
                })
                .map(|results| {
                    json!({
                        "albums": results.albums,
                        "tracks": results.tracks.iter().map(|track| json!({
                            "id": track.id,
                            "title": track.title,
                            "artist": track.artist,
                            "album": track.album,
                        })).collect::<Vec<_>>(),
                    })
                })
        }
        ("POST", "/api/command") => match serde_json::from_slice::<Command>(&request.body) {
            Ok(command) => run_command(app, command).map(|()| json!({ "ok": true })),
            Err(error) => return Response::error(400, &error.to_string()),
        },
        _ => return Response::error(404, "Not found"),
    };
    match result {
        Ok(value) => Response::json(&value),
        Err(error) => Response::error(500, &error),
    }
}

fn state<R: Runtime>(app: &AppHandle<R>) -> Result<Value, String> {
    let queue = crate::queue::queue_state(app.clone())?;
    let player = crate::audio::player_status(app.clone())?;
    Ok(json!({ "queue": queue, "player": player }))
}

fn run_command<R: Runtime>(app: &AppHandle<R>, command: Command) -> Result<(), String> {
    match command {
        Command::Toggle => crate::queue::toggle(app),
        Command::Next => crate::queue::next(app),
        Command::Previous => crate::queue::previous(app),
        Command::Seek { position } => crate::queue::seek(app, position),
        Command::Volume { value } => {
            crate::audio::player_set_volume(app.clone(), value.clamp(0.0, 1.0))
        }
        Command::Jump { uid } => crate::queue::queue_jump(app.clone(), uid),
        Command::Shuffle { on } => crate::queue::queue_set_shuffle(app.clone(), on),
        Command::PlayAlbum { album_id } => {
            let ids: Vec<i64> = {
                let library = app
                    .try_state::<LibraryState>()
                    .ok_or("The library is not available")?;
                let conn = library.conn();
                let mut statement = conn
                    .prepare(
                        "SELECT id FROM tracks WHERE album_id = ?1
                         ORDER BY IFNULL(disc_number, 1), track_number NULLS LAST, relative_path,
                                  range_start",
                    )
                    .map_err(|e| e.to_string())?;
                let ids = statement
                    .query_map([album_id], |row| row.get(0))
                    .map_err(|e| e.to_string())?
                    .collect::<Result<_, _>>()
                    .map_err(|e| e.to_string())?;
                ids
            };
            tauri::async_runtime::block_on(crate::queue::queue_play(app.clone(), ids, 0))
        }
        Command::PlayTrack { track_id } => {
            tauri::async_runtime::block_on(crate::queue::queue_add(
                app.clone(),
                vec![track_id],
                true,
            ))?;
            crate::queue::next(app)
        }
    }
}

/// The address phones reach this computer at: the one it would use to
/// reach the internet (nothing is sent to find it).
fn lan_address() -> Option<IpAddr> {
    let socket = UdpSocket::bind("0.0.0.0:0").ok()?;
    socket.connect("192.0.2.1:9").ok()?;
    let address = socket.local_addr().ok()?.ip();
    http::is_local(address).then_some(address)
}

// ---- Commands -------------------------------------------------------------------

fn status_of<R: Runtime>(app: &AppHandle<R>) -> Result<RemoteStatus, String> {
    let server = app
        .try_state::<RemoteServer>()
        .ok_or("The remote is not available")?;
    let library = app
        .try_state::<LibraryState>()
        .ok_or("The library is not available")?;
    let port = server
        .running
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .as_ref()
        .map(|running| running.port);
    let code = server.pairing().current(Instant::now()).map(str::to_owned);
    let devices = devices(&library.conn()).map_err(|e| e.to_string())?;
    let url = port.and_then(|port| {
        let address = lan_address()?;
        Some(match address {
            IpAddr::V6(v6) => format!("http://[{v6}]:{port}"),
            IpAddr::V4(v4) => format!("http://{v4}:{port}"),
        })
    });
    let error = server
        .error
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .clone();
    Ok(RemoteStatus {
        running: port.is_some(),
        url,
        error,
        code,
        devices,
    })
}

#[tauri::command]
pub fn remote_status<R: Runtime>(app: AppHandle<R>) -> Result<RemoteStatus, String> {
    status_of(&app)
}

/// Shows a new pairing code, valid for five minutes.
#[tauri::command]
pub fn remote_new_code<R: Runtime>(app: AppHandle<R>) -> Result<RemoteStatus, String> {
    {
        let server = app
            .try_state::<RemoteServer>()
            .ok_or("The remote is not available")?;
        let random = &server.random;
        server.pairing().new_code(random, Instant::now());
    }
    status_of(&app)
}

#[tauri::command]
pub fn remote_forget<R: Runtime>(
    app: AppHandle<R>,
    device_id: i64,
) -> Result<RemoteStatus, String> {
    {
        let library = app
            .try_state::<LibraryState>()
            .ok_or("The library is not available")?;
        library
            .conn()
            .execute("DELETE FROM remote_devices WHERE id = ?1", [device_id])
            .map_err(|e| e.to_string())?;
    }
    status_of(&app)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::library::db;

    #[test]
    fn pairing_codes_are_used_once_and_expire() {
        let random = SystemRandom::new();
        let mut pairing = Pairing::default();
        let now = Instant::now();
        let peer: IpAddr = "192.168.1.5".parse().unwrap();
        assert_eq!(
            pairing.check(peer, "123456", now),
            Err(PairError::WrongCode)
        );
        let code = pairing.new_code(&random, now);
        assert_eq!(code.len(), 6);
        assert_eq!(pairing.current(now), Some(code.as_str()));
        assert_eq!(pairing.check(peer, &code, now), Ok(()));
        assert_eq!(
            pairing.check(peer, &code, now),
            Err(PairError::WrongCode),
            "used up"
        );

        let code = pairing.new_code(&random, now);
        let later = now + CODE_LIFETIME;
        assert_eq!(pairing.current(later), None);
        assert_eq!(pairing.check(peer, &code, later), Err(PairError::WrongCode));
    }

    #[test]
    fn repeated_failures_lock_an_address_out() {
        let random = SystemRandom::new();
        let mut pairing = Pairing::default();
        let now = Instant::now();
        let peer: IpAddr = "10.0.0.2".parse().unwrap();
        let code = pairing.new_code(&random, now);
        let wrong = if code == "000000" { "000001" } else { "000000" };
        for _ in 0..MAX_FAILURES {
            assert_eq!(pairing.check(peer, wrong, now), Err(PairError::WrongCode));
        }
        assert_eq!(pairing.check(peer, &code, now), Err(PairError::LockedOut));
        // Another phone isn't.
        assert_eq!(
            pairing.check("10.0.0.3".parse().unwrap(), &code, now),
            Ok(())
        );
        // After the lockout, the address may try again.
        let later = now + LOCKOUT;
        let code = pairing.new_code(&random, later);
        assert_eq!(pairing.check(peer, &code, later), Ok(()));
    }

    #[test]
    fn keeps_only_hashes_of_tokens() {
        let conn = db::open_in_memory().unwrap();
        let random = SystemRandom::new();
        let token = add_device(&conn, "  My phone  ", &random).unwrap();
        assert_eq!(token.len(), 64);
        let stored: Vec<u8> = conn
            .query_row("SELECT token_hash FROM remote_devices", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_ne!(stored, token.as_bytes());
        let id = check_token(&conn, &token).unwrap().unwrap();
        assert_eq!(check_token(&conn, "nope").unwrap(), None);
        let listed = devices(&conn).unwrap();
        assert_eq!(listed[0].id, id);
        assert_eq!(listed[0].name, "My phone");
        assert!(listed[0].last_seen.is_some());
    }
}
