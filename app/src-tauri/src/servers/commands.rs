//! The music servers' commands and background work (PLAN.md Phases 7e and
//! 7f): finding a Jellyfin server, signing in (with a password or Quick
//! Connect) and out, syncing, keeping music offline, and saying whether
//! each server answers. Signing in to Plex is in `plex_commands`; the rest
//! works for every kind of server through `ServerApi`.
//!
//! Everything that goes online runs on a blocking thread, never the main
//! one, with a database connection of its own or the shared one held only
//! briefly. Each command refuses while its kind of server's feature is off
//! (`kind_on`). A server's state is a `FolderState` like a folder's
//! (`availability`): `Unreachable` when it doesn't answer, `SignedOut` when
//! it refuses the token, `Off` while the feature is off; its tracks stay
//! either way.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, Runtime};

use super::address::{self, System};
use super::api::{self, ServerApi};
use super::discovery::{self, Discovered};
use super::fetch::{self, RemoteFile};
use super::jellyfin::Jellyfin;
use super::offline::{self, OfflineStatus};
use super::plextv::PlexTv;
use super::sync::{self, SyncError, SyncOptions};
use super::{plex, Device, ServerInfo, Source};
use crate::coded::{coded, feature_off};
use crate::library::access::{FolderState, FolderStatus};
use crate::library::availability::{self, Changes, FolderStates};
use crate::library::commands::{LibraryState, LIBRARY_CHANGED_EVENT, SCAN_PROGRESS_EVENT};
use crate::library::scanner::ScanReport;
use crate::library::{db, FolderKind};
use crate::metadata::http::{Client, SystemClock, UreqTransport, IMAGE_LIMIT};
use crate::settings::{self, AppSettings};

/// Frontend event, without a payload: a server was added or removed, or a
/// sync or offline download started or finished.
pub const SERVERS_EVENT: &str = "servers-changed";

/// A server that answered within this long isn't asked again when the UI
/// lists folders.
const CHECK_AGAIN_AFTER: Duration = Duration::from_secs(60);

/// What the servers' work shares.
pub struct ServersState {
    /// The client every server request goes through: apart from the
    /// metadata worker's, so a server's backoff doesn't hold up MusicBrainz
    /// or the reverse.
    pub http: Client,
    /// Sources being synced now.
    syncing: Mutex<HashSet<i64>>,
    /// When each source last answered a check.
    checked: Mutex<HashMap<i64, Instant>>,
    /// Whether tracks kept offline are being downloaded.
    keeping: AtomicBool,
    /// Whether the servers are being checked (`check_sources`).
    checking: AtomicBool,
}

impl ServersState {
    fn syncing(&self) -> std::sync::MutexGuard<'_, HashSet<i64>> {
        self.syncing.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn checked(&self) -> std::sync::MutexGuard<'_, HashMap<i64, Instant>> {
        self.checked.lock().unwrap_or_else(|e| e.into_inner())
    }
}

/// Starts the servers' work: the client, the play reports, the settings'
/// policy, and, if the feature is on, a check and sync of each server.
/// Call after the library, the settings and the queue.
pub fn init<R: Runtime>(app: &AppHandle<R>) -> Result<(), String> {
    app.manage(ServersState {
        http: Client::new(Box::new(UreqTransport::new()), Box::new(SystemClock)),
        syncing: Mutex::new(HashSet::new()),
        checked: Mutex::new(HashMap::new()),
        keeping: AtomicBool::new(false),
        checking: AtomicBool::new(false),
    });
    super::playing::init(app)?;
    let current = settings::current(app);
    apply_policy(app, &current);
    let app = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        prepare_sources(&app);
        if super::any_on(&settings::current(&app).features) {
            sync_all(&app);
            keep_offline(&app);
        }
    });
    Ok(())
}

/// The settings changed: the cache's budget, the bit rate away from home,
/// Plex's relay, or a kind of server turned on or off.
pub fn configure<R: Runtime>(app: &AppHandle<R>, settings: &AppSettings, before: &AppSettings) {
    apply_policy(app, settings);
    let turned = settings.features.jellyfin != before.features.jellyfin
        || settings.features.plex != before.features.plex
        || settings.servers.plex_relay != before.servers.plex_relay;
    let app = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        evict(&app);
        if turned {
            if let Some(servers) = app.try_state::<ServersState>() {
                servers.checked().clear();
            }
            prepare_sources(&app);
            if super::any_on(&settings::current(&app).features) {
                sync_all(&app);
                keep_offline(&app);
            }
            let _ = app.emit(SERVERS_EVENT, ());
        }
    });
}

fn apply_policy<R: Runtime>(app: &AppHandle<R>, settings: &AppSettings) {
    if let Some(library) = app.try_state::<LibraryState>() {
        library
            .audio
            .cache
            .set_budget(settings.servers.cache_bytes());
        library.audio.set_away_kbps(settings.servers.away());
    }
}

/// Evicts down to the budget, keeping what is kept offline. Blocking.
fn evict<R: Runtime>(app: &AppHandle<R>) {
    let Some(library) = app.try_state::<LibraryState>() else {
        return;
    };
    let keep = offline::keep(&library.conn(), &library.audio).unwrap_or_default();
    library.audio.cache.evict(&keep);
}

/// Each source's host gets requests as fast as they come, and its state is
/// `Off` while its kind's feature is off. Blocking.
fn prepare_sources<R: Runtime>(app: &AppHandle<R>) {
    let (Some(library), Some(servers)) = (
        app.try_state::<LibraryState>(),
        app.try_state::<ServersState>(),
    ) else {
        return;
    };
    let sources = super::sources(&library.conn()).unwrap_or_default();
    let features = settings::current(app).features;
    let mut changes = Changes::default();
    if let Some(states) = app.try_state::<FolderStates>() {
        for source in &sources {
            if let Some(host) = crate::metadata::http::host_of(&source.address) {
                servers.http.set_interval(host, Duration::ZERO);
            }
            if !super::kind_on(&features, source.kind) {
                states.set(
                    source.folder_id,
                    status(FolderState::Off, None),
                    &mut changes,
                );
            } else if states.get(source.folder_id).state == FolderState::Off {
                states.set(source.folder_id, FolderStatus::AVAILABLE, &mut changes);
            }
        }
    }
    availability::changed(app, changes, false);
}

fn status(state: FolderState, detail: Option<String>) -> FolderStatus {
    FolderStatus { state, detail }
}

/// Records what a server's answer (or silence) says about its state.
fn record<R: Runtime>(app: &AppHandle<R>, folder_id: i64, outcome: Result<(), &str>) {
    let Some(states) = app.try_state::<FolderStates>() else {
        return;
    };
    let new = match outcome {
        Ok(()) => {
            if let Some(servers) = app.try_state::<ServersState>() {
                servers.checked().insert(folder_id, Instant::now());
            }
            // A sync that held the tracks keeps saying so until one doesn't.
            if states.get(folder_id).state == FolderState::MostlyGone {
                return;
            }
            FolderStatus::AVAILABLE
        }
        Err(error)
            if crate::coded::is(&error, "serverSignedOut")
                || crate::coded::is(&error, "plexSignedOut") =>
        {
            status(FolderState::SignedOut, None)
        }
        Err(error)
            if crate::coded::is(&error, "serverUnreachable")
                || crate::coded::is(&error, "plexRelayOff") =>
        {
            status(FolderState::Unreachable, None)
        }
        // Another failure says nothing about whether it answers.
        Err(_) => return,
    };
    let mut changes = Changes::default();
    states.set(folder_id, new, &mut changes);
    availability::changed(app, changes, false);
}

/// Refuses unless servers of `kind` are turned on.
pub(super) fn require_kind<R: Runtime>(app: &AppHandle<R>, kind: FolderKind) -> Result<(), String> {
    if super::kind_on(&settings::current(app).features, kind) {
        Ok(())
    } else {
        Err(kind_off(kind))
    }
}

/// The error a command refuses with while servers of `kind` are turned off.
pub(super) fn kind_off(kind: FolderKind) -> String {
    if kind == FolderKind::Plex {
        feature_off("plex", "Plex servers")
    } else {
        feature_off("jellyfin", "Jellyfin servers")
    }
}

/// Refuses unless some kind of server is turned on.
fn require_any<R: Runtime>(app: &AppHandle<R>) -> Result<(), String> {
    if super::any_on(&settings::current(app).features) {
        Ok(())
    } else {
        require_kind(app, FolderKind::Jellyfin)
    }
}

pub(super) fn library<R: Runtime>(
    app: &AppHandle<R>,
) -> Result<tauri::State<'_, LibraryState>, String> {
    app.try_state::<LibraryState>()
        .ok_or_else(|| "The library is not available".to_owned())
}

pub(super) fn servers<R: Runtime>(
    app: &AppHandle<R>,
) -> Result<tauri::State<'_, ServersState>, String> {
    app.try_state::<ServersState>()
        .ok_or_else(|| "The servers are not available".to_owned())
}

/// Runs `f` on a blocking thread.
pub(super) async fn blocking<R: Runtime, T: Send + 'static>(
    app: &AppHandle<R>,
    f: impl FnOnce(&AppHandle<R>) -> Result<T, String> + Send + 'static,
) -> Result<T, String> {
    let app = app.clone();
    tauri::async_runtime::spawn_blocking(move || f(&app))
        .await
        .map_err(|e| e.to_string())?
}

/// The source `folder_id`, and its token (failing as signed out without one).
fn source_and_token(
    library: &LibraryState,
    folder_id: i64,
) -> Result<(Source, String, Device), String> {
    let conn = library.conn();
    let source = super::source(&conn, folder_id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| crate::coded::no_folder(folder_id))?;
    let device = Device::of(&conn).map_err(|e| e.to_string())?;
    drop(conn);
    let token = source.token().map_err(|e| e.to_string())?.ok_or_else(|| {
        api::error_text(
            &crate::metadata::Error::Status {
                url: String::new(),
                status: 401,
            },
            &source.server_name,
        )
    })?;
    Ok((source, token, device))
}

/// The source's server at an address that answers now, and what it says of
/// itself: a Jellyfin server at its one address; a Plex server at the
/// address that answered last, else the next of its connections that does
/// (`plex::connect`), remembered for next time. Records whether the address
/// is on the local network. Blocking.
pub(super) fn reach(
    library: &LibraryState,
    servers: &ServersState,
    source: &Source,
    device: &Device,
    allow_relay: bool,
) -> Result<(Source, ServerInfo), String> {
    let mut source = source.clone();
    let info = match source.kind {
        FolderKind::Plex => {
            let (address, identity) = plex::connect(
                &servers.http,
                device,
                &source.server_id,
                &source.server_name,
                &source.address,
                &source.connections,
                allow_relay,
                &System,
            )
            .map_err(|error| api::error_text(&error, &source.server_name))?;
            if address != source.address {
                super::set_address(&library.conn(), source.folder_id, &address)
                    .map_err(|e| e.to_string())?;
                log::info!("server {} answers at another address", source.folder_id);
                log::debug!("server {} answers at {address}", source.folder_id);
                source.via_relay = source
                    .connections
                    .iter()
                    .any(|connection| connection.relay && connection.uri == address);
                source.address = address;
            }
            ServerInfo {
                id: identity.machine_id,
                name: source.server_name.clone(),
                version: identity.version.or_else(|| source.version.clone()),
            }
        }
        FolderKind::Jellyfin | FolderKind::Local => {
            address::check(&source.address, &System)?;
            ServerApi::new(&servers.http, device, &source, None)
                .info(&source)
                .map_err(|error| api::error_text(&error, &source.server_name))?
        }
    };
    if let Some(host) = crate::metadata::http::host_of(&source.address) {
        servers.http.set_interval(host, Duration::ZERO);
    }
    library.audio.set_local(
        source.folder_id,
        address::is_on_local_network(&source.address, &System),
    );
    Ok((source, info))
}

// ---- Listing and checking ----------------------------------------------------

/// A server source as the Settings panel and the sidebar show it.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct ServerSummary {
    #[serde(flatten)]
    pub source: Source,
    pub status: FolderStatus,
    pub syncing: bool,
}

/// Every server source with its state as last found.
#[tauri::command]
pub async fn servers_list<R: Runtime>(app: AppHandle<R>) -> Result<Vec<ServerSummary>, String> {
    blocking(&app, |app| {
        let library = library(app)?;
        let sources = super::sources(&library.conn()).map_err(|e| e.to_string())?;
        let syncing = app
            .try_state::<ServersState>()
            .map(|servers| servers.syncing().clone())
            .unwrap_or_default();
        let states = app.try_state::<FolderStates>();
        Ok(sources
            .into_iter()
            .map(|source| ServerSummary {
                status: states.as_ref().map_or(FolderStatus::AVAILABLE, |states| {
                    states.get(source.folder_id)
                }),
                syncing: syncing.contains(&source.folder_id),
                source,
            })
            .collect())
    })
    .await
}

/// Asks each server that hasn't answered lately whether it is there, on a
/// thread of its own, and records what it says, which tells the UI if a
/// state changed (`availability::refresh` calls this when the UI lists
/// folders, which doesn't wait for a server that is away).
pub fn check_sources<R: Runtime>(app: &AppHandle<R>) {
    if !super::any_on(&settings::current(app).features) {
        return;
    }
    let Ok(servers) = servers(app) else {
        return;
    };
    if servers.checking.swap(true, Ordering::SeqCst) {
        return;
    }
    let app = app.clone();
    let spawned = std::thread::Builder::new()
        .name("server checks".into())
        .spawn(move || {
            check_now(&app);
            if let Some(servers) = app.try_state::<ServersState>() {
                servers.checking.store(false, Ordering::SeqCst);
            }
        });
    if spawned.is_err() {
        servers.checking.store(false, Ordering::SeqCst);
    }
}

/// `check_sources`' work. Blocking.
fn check_now<R: Runtime>(app: &AppHandle<R>) {
    let (Ok(library), Ok(servers)) = (library(app), servers(app)) else {
        return;
    };
    let (sources, device) = {
        let conn = library.conn();
        (super::sources(&conn).unwrap_or_default(), Device::of(&conn))
    };
    let Ok(device) = device else {
        return;
    };
    let current = settings::current(app);
    for source in sources {
        let fresh = servers
            .checked()
            .get(&source.folder_id)
            .is_some_and(|at| at.elapsed() < CHECK_AGAIN_AFTER);
        if fresh || !super::kind_on(&current.features, source.kind) {
            continue;
        }
        match reach(
            &library,
            &servers,
            &source,
            &device,
            current.servers.plex_relay,
        ) {
            Ok((_, info)) => {
                if info.version != source.version || info.name != source.server_name {
                    let _ = super::update_info(&library.conn(), source.folder_id, &info);
                }
                record(app, source.folder_id, Ok(()));
            }
            Err(text) => record(app, source.folder_id, Err(&text)),
        }
    }
}

/// Servers on the local network that answered.
#[tauri::command]
pub async fn servers_discover<R: Runtime>(app: AppHandle<R>) -> Result<Vec<Discovered>, String> {
    require_kind(&app, FolderKind::Jellyfin)?;
    blocking(&app, |_| {
        discovery::discover().map_err(|error| {
            coded(
                "serverDiscovery",
                &[],
                format!("Couldn't look for servers on the network: {error}"),
            )
        })
    })
    .await
}

/// A server the user named, before signing in.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct ServerCheck {
    /// The address as it will be used.
    pub address: String,
    pub name: String,
    pub version: Option<String>,
    /// Whether it allows signing in with Quick Connect.
    pub quick_connect: bool,
}

/// Checks the address the user typed (or picked): that the app may use it
/// and a Jellyfin server answers there.
#[tauri::command]
pub async fn servers_check<R: Runtime>(
    app: AppHandle<R>,
    address: String,
) -> Result<ServerCheck, String> {
    require_kind(&app, FolderKind::Jellyfin)?;
    blocking(&app, move |app| {
        let address = address::parse(&address)?;
        address::check(&address, &System)?;
        let (library, servers) = (library(app)?, servers(app)?);
        let device = Device::of(&library.conn()).map_err(|e| e.to_string())?;
        let server = Jellyfin {
            http: &servers.http,
            address: &address,
            device: &device,
            token: None,
        };
        let info = server
            .public_info()
            .map_err(|error| api::error_text(&error, &address))?;
        let quick_connect = server.quick_connect_enabled().unwrap_or(false);
        Ok(ServerCheck {
            address,
            name: info.name,
            version: info.version,
            quick_connect,
        })
    })
    .await
}

// ---- Signing in and out --------------------------------------------------------

/// Signs in to the server at `address` with a user name and password; the
/// password goes to the server and is never stored. Adds the source (or
/// finds the one already there) and syncs it in the background.
#[tauri::command]
pub async fn servers_sign_in<R: Runtime>(
    app: AppHandle<R>,
    address: String,
    user: String,
    password: String,
) -> Result<Source, String> {
    require_kind(&app, FolderKind::Jellyfin)?;
    blocking(&app, move |app| {
        signed_in(app, &address, |server| {
            server.sign_in(user.trim(), &password)
        })
    })
    .await
}

/// A Quick Connect sign-in started: the code for the user to approve in
/// another Jellyfin app, and the secret to finish with.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct QuickConnectStart {
    pub code: String,
    pub secret: String,
}

#[tauri::command]
pub async fn servers_quick_connect_start<R: Runtime>(
    app: AppHandle<R>,
    address: String,
) -> Result<QuickConnectStart, String> {
    require_kind(&app, FolderKind::Jellyfin)?;
    blocking(&app, move |app| {
        let address = address::parse(&address)?;
        address::check(&address, &System)?;
        let (library, servers) = (library(app)?, servers(app)?);
        let device = Device::of(&library.conn()).map_err(|e| e.to_string())?;
        let started = Jellyfin {
            http: &servers.http,
            address: &address,
            device: &device,
            token: None,
        }
        .quick_connect_start()
        .map_err(|error| api::error_text(&error, &address))?;
        crate::logging::keep_secret(&started.secret);
        Ok(QuickConnectStart {
            code: started.code,
            secret: started.secret,
        })
    })
    .await
}

/// Finishes a Quick Connect sign-in once the user approved the code: the
/// source, or `None` while the code waits for approval (the UI asks again
/// every few seconds).
#[tauri::command]
pub async fn servers_quick_connect_finish<R: Runtime>(
    app: AppHandle<R>,
    address: String,
    secret: String,
) -> Result<Option<Source>, String> {
    require_kind(&app, FolderKind::Jellyfin)?;
    blocking(&app, move |app| {
        let address = address::parse(&address)?;
        {
            let (library, servers) = (library(app)?, servers(app)?);
            let device = Device::of(&library.conn()).map_err(|e| e.to_string())?;
            let approved = Jellyfin {
                http: &servers.http,
                address: &address,
                device: &device,
                token: None,
            }
            .quick_connect_approved(&secret)
            .map_err(|error| api::error_text(&error, &address))?;
            if !approved {
                return Ok(None);
            }
        }
        signed_in(app, &address, |server| {
            server.quick_connect_sign_in(&secret)
        })
        .map(Some)
    })
    .await
}

/// Signs in with `sign_in` at `address`, then adds the source, stores its
/// token and starts its first sync. Blocking.
fn signed_in<R: Runtime>(
    app: &AppHandle<R>,
    address: &str,
    sign_in: impl FnOnce(&Jellyfin<'_>) -> Result<super::SignedIn, crate::metadata::Error>,
) -> Result<Source, String> {
    let address = address::parse(address)?;
    address::check(&address, &System)?;
    let (library, servers) = (library(app)?, servers(app)?);
    let device = Device::of(&library.conn()).map_err(|e| e.to_string())?;
    let server = Jellyfin {
        http: &servers.http,
        address: &address,
        device: &device,
        token: None,
    };
    let info = server
        .public_info()
        .map_err(|error| api::error_text(&error, &address))?;
    let signed = sign_in(&server).map_err(|error| api::error_text(&error, &info.name))?;
    crate::logging::keep_secret(&signed.token);
    let folder_id = super::add_source(
        &mut library.conn(),
        FolderKind::Jellyfin,
        &address,
        &info,
        &signed,
        None,
    )
    .map_err(|e| e.to_string())?;
    let source = super::source(&library.conn(), folder_id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| crate::coded::no_folder(folder_id))?;
    super::store_token(&source.token_key(), Some(&signed.token)).map_err(|e| e.to_string())?;
    log::info!("signed in to server {folder_id}");
    log::debug!(
        "server {folder_id} is {} at {address}, as {}",
        source.server_name,
        source.user_name
    );
    if let Some(host) = crate::metadata::http::host_of(&address) {
        servers.http.set_interval(host, Duration::ZERO);
    }
    library
        .audio
        .set_local(folder_id, address::is_on_local_network(&address, &System));
    record(app, folder_id, Ok(()));
    let _ = app.emit(SERVERS_EVENT, ());
    sync_later(app, folder_id);
    Ok(source)
}

/// Syncs source `folder_id` on a thread of its own: a new source's first.
pub(super) fn sync_later<R: Runtime>(app: &AppHandle<R>, folder_id: i64) {
    let sync_app = app.clone();
    std::thread::spawn(move || {
        if let Err(error) = sync_now(&sync_app, folder_id, SyncOptions::default()) {
            log::warn!("cannot sync server {folder_id}: {error}");
        }
    });
}

/// Signs out of server source `folder_id`: ends the session on the server
/// (if it answers), deletes the token unless another source shares it (a
/// Plex server's other music libraries; a Plex account's token goes with
/// its last source), the source's tracks and playlists, and its tracks in
/// the audio cache.
#[tauri::command]
pub async fn servers_sign_out<R: Runtime>(app: AppHandle<R>, folder_id: i64) -> Result<(), String> {
    blocking(&app, move |app| {
        let (library, servers) = (library(app)?, servers(app)?);
        if servers.syncing().contains(&folder_id) {
            return Err(coded(
                "serverSyncRunning",
                &[],
                "Wait for the server's sync to finish",
            ));
        }
        let source = super::source(&library.conn(), folder_id)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| crate::coded::no_folder(folder_id))?;
        let (token_shared, account_shared) = {
            let conn = library.conn();
            let others: Vec<Source> = super::sources(&conn)
                .map_err(|e| e.to_string())?
                .into_iter()
                .filter(|other| other.folder_id != folder_id)
                .collect();
            (
                others
                    .iter()
                    .any(|other| other.token_key() == source.token_key()),
                others
                    .iter()
                    .any(|other| other.kind == FolderKind::Plex && other.user_id == source.user_id),
            )
        };
        if !token_shared {
            if let (Ok(Some(token)), Ok(device)) = (source.token(), Device::of(&library.conn())) {
                let ended =
                    ServerApi::new(&servers.http, &device, &source, Some(&token)).sign_out();
                if let Err(error) = ended {
                    log::warn!("cannot end the session on server {folder_id}: {error}");
                }
            }
        }
        // The cached files' names, before their rows go.
        let names: HashSet<String> = {
            let conn = library.conn();
            let ids: Vec<i64> = conn
                .prepare("SELECT id FROM tracks WHERE folder_id = ?1")
                .and_then(|mut statement| {
                    statement
                        .query_map([folder_id], |row| row.get(0))?
                        .collect::<Result<_, _>>()
                })
                .map_err(|e| e.to_string())?;
            ids.into_iter()
                .filter_map(|id| fetch::remote_file(&conn, id).ok().flatten())
                .flat_map(|file| library.audio.names(&file))
                .collect()
        };
        crate::library::remove_folder(&mut library.conn(), folder_id).map_err(|e| e.to_string())?;
        library.audio.cache.remove(&names);
        if !token_shared {
            super::store_token(&source.token_key(), None).map_err(|e| e.to_string())?;
        }
        if source.kind == FolderKind::Plex && !account_shared {
            super::store_token(&super::plex_account_key(&source.user_id), None)
                .map_err(|e| e.to_string())?;
        }
        if let Some(states) = app.try_state::<FolderStates>() {
            states.forget(folder_id);
        }
        servers.checked().remove(&folder_id);
        log::info!("signed out of server {folder_id}");
        library.art.clear();
        let _ = app.emit(crate::library::availability::FOLDERS_EVENT, ());
        let _ = app.emit(LIBRARY_CHANGED_EVENT, Vec::<ScanReport>::new());
        let _ = app.emit(SERVERS_EVENT, ());
        let refresh_app = app.clone();
        tauri::async_runtime::spawn(async move {
            crate::queue::refresh_tracks(&refresh_app).await;
        });
        Ok(())
    })
    .await
}

// ---- Syncing -------------------------------------------------------------------

/// Syncs server source `folder_id` now: its changes, or everything if
/// `full`.
#[tauri::command]
pub async fn servers_sync<R: Runtime>(
    app: AppHandle<R>,
    folder_id: i64,
    full: bool,
) -> Result<ScanReport, String> {
    blocking(&app, move |app| {
        sync_now(
            app,
            folder_id,
            SyncOptions {
                full,
                ..SyncOptions::default()
            },
        )
    })
    .await
}

/// Syncs every source, one after another. Blocking.
fn sync_all<R: Runtime>(app: &AppHandle<R>) {
    let Ok(library) = library(app) else {
        return;
    };
    let sources = super::sources(&library.conn()).unwrap_or_default();
    for source in sources {
        if let Err(error) = sync_now(app, source.folder_id, SyncOptions::default()) {
            log::warn!("cannot sync server {}: {error}", source.folder_id);
        }
    }
}

/// Syncs source `folder_id` as `options` say, then downloads its new album
/// pictures and tells the UI and the queue. Blocking.
pub fn sync_now<R: Runtime>(
    app: &AppHandle<R>,
    folder_id: i64,
    options: SyncOptions,
) -> Result<ScanReport, String> {
    let (library, servers) = (library(app)?, servers(app)?);
    let kind = super::source(&library.conn(), folder_id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| crate::coded::no_folder(folder_id))?
        .kind;
    require_kind(app, kind)?;
    if !servers.syncing().insert(folder_id) {
        return Err(coded(
            "serverSyncRunning",
            &[],
            "The server is already being synced",
        ));
    }
    let _ = app.emit(SERVERS_EVENT, ());
    let result: Result<ScanReport, String> = (|| {
        if kind == FolderKind::Plex {
            refresh_plex(&library, &servers, folder_id);
        }
        let (source, token, device) = source_and_token(&library, folder_id)?;
        let allow_relay = settings::current(app).servers.plex_relay;
        let (source, _) = reach(&library, &servers, &source, &device, allow_relay)?;
        let server = ServerApi::new(&servers.http, &device, &source, Some(&token));
        let mut conn = db::open(library.db_path()).map_err(|e| e.to_string())?;
        let progress_app = app.clone();
        let synced = sync::sync(
            &mut conn,
            server.library(&source).as_ref(),
            folder_id,
            options,
            |progress| {
                let _ = progress_app.emit(SCAN_PROGRESS_EVENT, progress);
            },
        )
        .map_err(|error| match error {
            SyncError::Server(error) => api::error_text(&error, &source.server_name),
            SyncError::Library(error) => error.to_string(),
        })?;
        download_art(&library, &server, &source, &conn);
        if !synced.art_changed.is_empty() {
            for album in &synced.art_changed {
                let _ = crate::library::thumbs::forget(
                    &conn,
                    crate::library::art::ArtKey::Album(*album),
                );
            }
        }
        Ok(synced.report)
    })();
    servers.syncing().remove(&folder_id);
    match &result {
        Ok(report) => {
            if let Some(states) = app.try_state::<FolderStates>() {
                let mut changes = Changes::default();
                let status = report
                    .unavailable
                    .clone()
                    .unwrap_or(FolderStatus::AVAILABLE);
                states.set(folder_id, status, &mut changes);
                availability::changed(app, changes, false);
            }
            if let Some(servers) = app.try_state::<ServersState>() {
                servers.checked().insert(folder_id, Instant::now());
            }
            library.art.clear();
            let _ = app.emit(LIBRARY_CHANGED_EVENT, vec![report.clone()]);
            let refresh_app = app.clone();
            tauri::async_runtime::spawn(async move {
                crate::queue::refresh_tracks(&refresh_app).await;
                crate::metadata::worker::enrich_library(&refresh_app);
            });
            keep_offline(app);
        }
        Err(error) => record(app, folder_id, Err(error.as_str())),
    }
    let _ = app.emit(SERVERS_EVENT, ());
    result
}

/// Asks plex.tv where the Plex server of source `folder_id` answers now,
/// with its name, version and token, and records them; a plex.tv that
/// can't be asked leaves them as they were (the stored addresses are
/// tried). Blocking.
fn refresh_plex(library: &LibraryState, servers: &ServersState, folder_id: i64) {
    let Ok(Some(source)) = super::source(&library.conn(), folder_id) else {
        return;
    };
    let Ok(Some(account_token)) = crate::metadata::keys::get(
        crate::metadata::keys::Account::Server(super::plex_account_key(&source.user_id)),
    ) else {
        return;
    };
    let Ok(device) = Device::of(&library.conn()) else {
        return;
    };
    let listed = PlexTv {
        http: &servers.http,
        device: &device,
        token: Some(&account_token),
    }
    .servers();
    let resource = match listed {
        Ok(resources) => resources
            .into_iter()
            .find(|resource| resource.id == source.server_id),
        Err(error) => {
            log::warn!("cannot ask plex.tv where server {folder_id} is: {error}");
            return;
        }
    };
    let Some(resource) = resource else {
        log::warn!("plex.tv no longer lists server {folder_id}");
        return;
    };
    crate::logging::keep_secret(&resource.access_token);
    let conn = library.conn();
    let _ = super::set_connections(&conn, folder_id, &resource.connections);
    let _ = super::update_info(
        &conn,
        folder_id,
        &ServerInfo {
            id: resource.id.clone(),
            name: resource.name.clone(),
            version: resource.version.clone(),
        },
    );
    drop(conn);
    if source.token().ok().flatten().as_deref() != Some(resource.access_token.as_str()) {
        let _ = super::store_token(&source.token_key(), Some(&resource.access_token));
    }
}

/// Downloads the source's album pictures the image cache lacks; one that
/// fails is tried again by the next sync. Blocking.
fn download_art(
    library: &LibraryState,
    server: &ServerApi,
    source: &Source,
    conn: &rusqlite::Connection,
) {
    let Ok(keys) = sync::missing_art(conn, source.folder_id, &library.images) else {
        return;
    };
    let mut fetched = 0;
    for key in keys {
        let Some(request) = server.art(&key) else {
            continue;
        };
        let headers = request.headers();
        match server
            .http()
            .get_with_headers(&request.url, "image/*", &headers, IMAGE_LIMIT)
        {
            Ok(response) => {
                if library.images.store(&key, &response.body).is_ok() {
                    fetched += 1;
                }
            }
            Err(crate::metadata::Error::Offline(_)) => break,
            Err(error) => log::debug!(
                "cannot fetch a picture from server {}: {error}",
                source.folder_id
            ),
        }
    }
    if fetched > 0 {
        log::info!(
            "fetched {fetched} pictures from server {}",
            source.folder_id
        );
    }
}

// ---- Playing ------------------------------------------------------------------

/// The file to play server track `file` from: the cached copy, or one
/// downloaded now (calling `downloading` first, so the queue waits
/// longer), unless `cancelled` says the queue gave up. Records the
/// server's state from the outcome. Blocking (`queue::opening`).
pub fn fetch_to_play<R: Runtime>(
    app: &AppHandle<R>,
    file: &RemoteFile,
    cancelled: &dyn Fn() -> bool,
    downloading: impl FnOnce(),
) -> Result<PathBuf, String> {
    let library = library(app)?;
    let choice = library.audio.choose(file);
    if library.audio.cache.contains(&choice.name) {
        library.audio.cache.opened(&choice.name);
        return Ok(library.audio.cache.path(&choice.name));
    }
    require_kind(app, file.kind)?;
    let servers = servers(app)?;
    let (source, token, device) = source_and_token(&library, file.folder_id)?;
    address::check(&source.address, &System)?;
    let keep = offline::keep(&library.conn(), &library.audio).unwrap_or_default();
    downloading();
    let fetch_from = |source: &Source| {
        fetch::download(
            &ServerApi::new(&servers.http, &device, source, Some(&token)),
            source,
            file,
            &choice,
            &library.audio.cache,
            &keep,
            cancelled,
        )
    };
    let mut result = fetch_from(&source);
    // A Plex server may answer at another of its addresses.
    let unreachable = matches!(&result, Err(error) if crate::coded::is(error, "serverUnreachable"));
    if source.kind == FolderKind::Plex && unreachable && !cancelled() {
        let allow_relay = settings::current(app).servers.plex_relay;
        if let Ok((moved, _)) = reach(&library, &servers, &source, &device, allow_relay) {
            if moved.address != source.address {
                result = fetch_from(&moved);
            }
        }
    }
    match &result {
        Ok(_) => record(app, file.folder_id, Ok(())),
        Err(error) if !fetch::is_cancelled(error) => {
            record(app, file.folder_id, Err(error.as_str()));
        }
        Err(_) => {}
    }
    result
}

// ---- Keeping music offline ----------------------------------------------------

#[tauri::command]
pub async fn servers_offline_status<R: Runtime>(
    app: AppHandle<R>,
) -> Result<OfflineStatus, String> {
    blocking(&app, |app| {
        let library = library(app)?;
        let status = offline::status(&library.conn(), &library.audio).map_err(|e| e.to_string());
        status
    })
    .await
}

/// Keeps an album or a playlist offline, or stops, then downloads what is
/// missing in the background.
#[tauri::command]
pub async fn servers_set_offline<R: Runtime>(
    app: AppHandle<R>,
    album_id: Option<i64>,
    playlist_id: Option<i64>,
    on: bool,
) -> Result<OfflineStatus, String> {
    require_any(&app)?;
    blocking(&app, move |app| {
        let library = library(app)?;
        {
            let conn = library.conn();
            if let Some(album) = album_id {
                offline::set_album(&conn, album, on).map_err(|e| e.to_string())?;
            }
            if let Some(playlist) = playlist_id {
                offline::set_playlist(&conn, playlist, on).map_err(|e| e.to_string())?;
            }
        }
        if on {
            keep_offline(app);
        } else {
            evict(app);
        }
        let _ = app.emit(SERVERS_EVENT, ());
        let status = offline::status(&library.conn(), &library.audio).map_err(|e| e.to_string());
        status
    })
    .await
}

/// Empties the audio cache, except what is kept offline and what plays.
#[tauri::command]
pub async fn servers_clear_cache<R: Runtime>(app: AppHandle<R>) -> Result<OfflineStatus, String> {
    blocking(&app, |app| {
        let library = library(app)?;
        let keep = offline::keep(&library.conn(), &library.audio).map_err(|e| e.to_string())?;
        library.audio.cache.clear(&keep);
        log::info!("emptied the audio cache");
        let status = offline::status(&library.conn(), &library.audio).map_err(|e| e.to_string());
        status
    })
    .await
}

/// Downloads the tracks kept offline that aren't cached, one at a time, on
/// a thread of its own; stops when a server can't be reached. Does nothing
/// while it runs already.
fn keep_offline<R: Runtime>(app: &AppHandle<R>) {
    let Ok(servers) = servers(app) else {
        return;
    };
    if !super::any_on(&settings::current(app).features)
        || servers.keeping.swap(true, Ordering::SeqCst)
    {
        return;
    }
    let app = app.clone();
    let spawned = std::thread::Builder::new()
        .name("offline downloads".into())
        .spawn(move || {
            let _ = app.emit(SERVERS_EVENT, ());
            let mut downloaded = 0;
            let mut skip: HashSet<i64> = HashSet::new();
            let mut tried: HashSet<i64> = HashSet::new();
            while let Ok(library) = library(&app) {
                let missing = offline::missing(&library.conn(), &library.audio).unwrap_or_default();
                let Some((track_id, file, _)) = missing
                    .into_iter()
                    .find(|(_, file, _)| !skip.contains(&file.folder_id))
                else {
                    break;
                };
                // Each once: one that downloads but still isn't cached
                // (a full disk) mustn't be fetched for ever.
                if !tried.insert(track_id) {
                    break;
                }
                if !super::any_on(&settings::current(&app).features) {
                    break;
                }
                match fetch_to_play(&app, &file, &|| false, || {}) {
                    Ok(_) => downloaded += 1,
                    Err(error) => {
                        log::warn!("cannot download a track to keep offline: {error}");
                        // The rest of this server's wait for the next sync.
                        skip.insert(file.folder_id);
                    }
                }
            }
            if downloaded > 0 {
                log::info!("downloaded {downloaded} tracks to keep offline");
            }
            if let Some(servers) = app.try_state::<ServersState>() {
                servers.keeping.store(false, Ordering::SeqCst);
            }
            let _ = app.emit(SERVERS_EVENT, ());
        });
    if let Err(error) = spawned {
        log::warn!("cannot start the offline downloads: {error}");
        servers.keeping.store(false, Ordering::SeqCst);
    }
}
