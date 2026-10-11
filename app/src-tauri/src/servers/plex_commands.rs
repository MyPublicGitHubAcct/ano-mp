//! Signing in to Plex and adding its music libraries (PLAN.md Phase 7f).
//!
//! The user signs in at plex.tv in their browser, approving a pin the app
//! made (`plex_sign_in_start`, then `plex_sign_in_finish` until approved),
//! so no password is typed into the app. The account's token goes in the
//! keychain. The app then lists the servers the account reaches with their
//! music libraries (`plex_servers`), and adds those the user picks
//! (`plex_add`), each a source of its own, synced at once. Everything else,
//! syncing, playing and signing out, is `commands`' as for any server.
//!
//! Each command refuses while the `plex` feature is off, and runs on a
//! blocking thread.

use std::collections::HashSet;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Runtime};

use super::address::System;
use super::commands::{
    blocking, library, require_kind, servers, sync_later, ServersState, SERVERS_EVENT,
};
use super::discovery;
use super::plex::{self, Plex};
use super::plextv::{self, Connection, PlexTv, Resource};
use super::{Device, Section, ServerInfo, SignedIn, Source};
use crate::library::commands::LibraryState;
use crate::library::FolderKind;
use crate::metadata::keys::{self, Account};
use crate::settings;

/// A Plex sign-in under way: the pin to ask about, the code the browser
/// page shows, and that page, which the UI opens.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct PlexPin {
    pub id: i64,
    pub code: String,
    pub url: String,
}

/// A Plex account signed in to.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct PlexAccount {
    pub id: String,
    pub name: String,
}

/// A server a Plex account reaches, with its music libraries.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct PlexServer {
    pub id: String,
    pub name: String,
    pub version: Option<String>,
    /// Whether the account owns it, rather than has it shared.
    pub owned: bool,
    pub libraries: Vec<PlexLibrary>,
    /// Why its libraries couldn't be listed (it doesn't answer, say).
    pub error: Option<String>,
}

/// A music library on a Plex server.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct PlexLibrary {
    pub key: String,
    pub name: String,
    /// Whether it is a source already.
    pub added: bool,
}

fn device(library: &LibraryState) -> Result<Device, String> {
    Device::of(&library.conn()).map_err(|e| e.to_string())
}

/// The token of account `account_id`, failing as signed out without one.
fn account_token(account_id: &str) -> Result<String, String> {
    keys::get(Account::Server(super::plex_account_key(account_id)))
        .map_err(|e| e.to_string())?
        .ok_or_else(|| {
            plextv::error_text(&crate::metadata::Error::Status {
                url: String::new(),
                status: 401,
            })
        })
}

/// Starts signing in: a pin, and the plex.tv page where the user approves
/// it, which the UI opens in the browser.
#[tauri::command]
pub async fn plex_sign_in_start<R: Runtime>(app: AppHandle<R>) -> Result<PlexPin, String> {
    require_kind(&app, FolderKind::Plex)?;
    blocking(&app, |app| {
        let (library, servers) = (library(app)?, servers(app)?);
        let device = device(&library)?;
        let plex_tv = PlexTv {
            http: &servers.http,
            device: &device,
            token: None,
        };
        let pin = plex_tv.create_pin().map_err(|e| plextv::error_text(&e))?;
        log::info!("started a Plex sign-in");
        Ok(PlexPin {
            url: plex_tv.auth_url(&pin.code),
            id: pin.id,
            code: pin.code,
        })
    })
    .await
}

/// Finishes signing in once the user approved pin `pin_id`: the account,
/// whose token is now in the keychain, or `None` while the pin waits (the
/// UI asks again every few seconds).
#[tauri::command]
pub async fn plex_sign_in_finish<R: Runtime>(
    app: AppHandle<R>,
    pin_id: i64,
) -> Result<Option<PlexAccount>, String> {
    require_kind(&app, FolderKind::Plex)?;
    blocking(&app, move |app| {
        let (library, servers) = (library(app)?, servers(app)?);
        let device = device(&library)?;
        let anonymous = PlexTv {
            http: &servers.http,
            device: &device,
            token: None,
        };
        let Some(token) = anonymous
            .pin_token(pin_id)
            .map_err(|e| plextv::error_text(&e))?
        else {
            return Ok(None);
        };
        crate::logging::keep_secret(&token);
        let account = PlexTv {
            token: Some(&token),
            ..anonymous
        }
        .account()
        .map_err(|e| plextv::error_text(&e))?;
        super::store_token(&super::plex_account_key(&account.id), Some(&token))
            .map_err(|e| e.to_string())?;
        log::info!("signed in to a Plex account");
        log::debug!("signed in to Plex as {}", account.name);
        Ok(Some(PlexAccount {
            id: account.id,
            name: account.name,
        }))
    })
    .await
}

/// Forgets account `account_id`'s token unless a source uses it: the user
/// signed in and then added nothing.
#[tauri::command]
pub async fn plex_sign_in_cancel<R: Runtime>(
    app: AppHandle<R>,
    account_id: String,
) -> Result<(), String> {
    blocking(&app, move |app| {
        let library = library(app)?;
        let used = super::sources(&library.conn())
            .map_err(|e| e.to_string())?
            .iter()
            .any(|source| source.kind == FolderKind::Plex && source.user_id == account_id);
        if !used {
            super::store_token(&super::plex_account_key(&account_id), None)
                .map_err(|e| e.to_string())?;
        }
        Ok(())
    })
    .await
}

/// The servers account `account_id` reaches, each with its music libraries
/// (or why they couldn't be listed).
#[tauri::command]
pub async fn plex_servers<R: Runtime>(
    app: AppHandle<R>,
    account_id: String,
) -> Result<Vec<PlexServer>, String> {
    require_kind(&app, FolderKind::Plex)?;
    blocking(&app, move |app| {
        let (library, servers) = (library(app)?, servers(app)?);
        let device = device(&library)?;
        let token = account_token(&account_id)?;
        let mut resources = PlexTv {
            http: &servers.http,
            device: &device,
            token: Some(&token),
        }
        .servers()
        .map_err(|e| plextv::error_text(&e))?;
        add_found_on_network(&mut resources);
        let allow_relay = settings::current(app).servers.plex_relay;
        let added: HashSet<String> = super::sources(&library.conn())
            .map_err(|e| e.to_string())?
            .into_iter()
            .map(|source| source.key())
            .collect();
        log::info!("the Plex account reaches {} servers", resources.len());
        Ok(resources
            .iter()
            .map(|resource| {
                crate::logging::keep_secret(&resource.access_token);
                let opened = open(&servers, &device, resource, allow_relay);
                let (libraries, error) = match opened {
                    Ok((_, _, sections)) => (
                        sections
                            .into_iter()
                            .map(|section| PlexLibrary {
                                added: added.contains(&super::folder_key(
                                    FolderKind::Plex,
                                    &resource.id,
                                    &account_id,
                                    &section.key,
                                )),
                                key: section.key,
                                name: section.name,
                            })
                            .collect(),
                        None,
                    ),
                    Err(error) => (Vec::new(), Some(error)),
                };
                PlexServer {
                    id: resource.id.clone(),
                    name: resource.name.clone(),
                    version: resource.version.clone(),
                    owned: resource.owned,
                    libraries,
                    error,
                }
            })
            .collect())
    })
    .await
}

/// Adds the music libraries `libraries` (their keys) of server `server_id`
/// that account `account_id` reaches, each a source synced at once; one
/// added already is brought up to date. Returns the sources.
#[tauri::command]
pub async fn plex_add<R: Runtime>(
    app: AppHandle<R>,
    account_id: String,
    server_id: String,
    libraries: Vec<String>,
) -> Result<Vec<Source>, String> {
    require_kind(&app, FolderKind::Plex)?;
    blocking(&app, move |app| {
        let (library, servers) = (library(app)?, servers(app)?);
        let device = device(&library)?;
        let token = account_token(&account_id)?;
        let plex_tv = PlexTv {
            http: &servers.http,
            device: &device,
            token: Some(&token),
        };
        let account = plex_tv.account().map_err(|e| plextv::error_text(&e))?;
        let mut resources = plex_tv.servers().map_err(|e| plextv::error_text(&e))?;
        add_found_on_network(&mut resources);
        let resource = resources
            .into_iter()
            .find(|resource| resource.id == server_id)
            .ok_or_else(|| {
                crate::coded::coded(
                    "plexServerGone",
                    &[],
                    "Your Plex account no longer reaches that server",
                )
            })?;
        crate::logging::keep_secret(&resource.access_token);
        let allow_relay = settings::current(app).servers.plex_relay;
        let (address, identity, sections) = open(&servers, &device, &resource, allow_relay)?;
        let info = ServerInfo {
            id: resource.id.clone(),
            name: resource.name.clone(),
            version: identity.version.or(resource.version.clone()),
        };
        let signed = SignedIn {
            token: resource.access_token.clone(),
            user_id: account.id.clone(),
            user_name: account.name.clone(),
            server_id: Some(resource.id.clone()),
        };
        let mut added = Vec::new();
        for section in sections
            .iter()
            .filter(|section| libraries.contains(&section.key))
        {
            let source = add(
                &library,
                &address,
                &info,
                &signed,
                section,
                &resource.connections,
            )?;
            added.push(source);
        }
        log::info!("added {} Plex music libraries", added.len());
        let _ = app.emit(SERVERS_EVENT, ());
        for source in &added {
            sync_later(app, source.folder_id);
        }
        Ok(added)
    })
    .await
}

/// Adds one music library as a source, with its connections and token.
fn add(
    library: &LibraryState,
    address: &str,
    info: &ServerInfo,
    signed: &SignedIn,
    section: &Section,
    connections: &[Connection],
) -> Result<Source, String> {
    let folder_id = super::add_source(
        &mut library.conn(),
        FolderKind::Plex,
        address,
        info,
        signed,
        Some(section),
    )
    .map_err(|e| e.to_string())?;
    super::set_connections(&library.conn(), folder_id, connections).map_err(|e| e.to_string())?;
    let source = super::source(&library.conn(), folder_id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| crate::coded::no_folder(folder_id))?;
    super::store_token(&source.token_key(), Some(&signed.token)).map_err(|e| e.to_string())?;
    library.audio.set_local(
        folder_id,
        super::address::is_on_local_network(address, &System),
    );
    log::info!("added Plex music library as server {folder_id}");
    log::debug!(
        "server {folder_id} is {} › {} at {address}",
        source.server_name,
        section.name
    );
    Ok(source)
}

/// A server reached at one of its addresses: the address, what it says of
/// itself, and its music libraries. Fails as the server's error words it.
fn open(
    servers: &ServersState,
    device: &Device,
    resource: &Resource,
    allow_relay: bool,
) -> Result<(String, plex::Identity, Vec<Section>), String> {
    let (address, identity) = plex::connect(
        &servers.http,
        device,
        &resource.id,
        &resource.name,
        "",
        &resource.connections,
        allow_relay,
        &System,
    )
    .map_err(|error| super::api::error_text(&error, &resource.name))?;
    if let Some(host) = crate::metadata::http::host_of(&address) {
        servers.http.set_interval(host, std::time::Duration::ZERO);
    }
    let sections = Plex {
        http: &servers.http,
        address: &address,
        device,
        token: Some(&resource.access_token),
    }
    .music_sections()
    .map_err(|error| super::api::error_text(&error, &resource.name))?;
    Ok((address, identity, sections))
}

/// Adds to each server plex.tv listed without an address on its own
/// network the one it answered GDM from, if it allows plain http.
fn add_found_on_network(resources: &mut [Resource]) {
    let wanting: Vec<&Resource> = resources
        .iter()
        .filter(|resource| {
            !resource.https_required
                && !resource
                    .connections
                    .iter()
                    .any(|connection| connection.local && !connection.relay)
        })
        .collect();
    if wanting.is_empty() {
        return;
    }
    let found = match discovery::discover_plex() {
        Ok(found) => found,
        Err(error) => {
            log::warn!("cannot look for Plex servers on the network: {error}");
            return;
        }
    };
    add_answers(resources, &found);
}

/// `add_found_on_network`'s work, given GDM's answers.
fn add_answers(resources: &mut [Resource], found: &[discovery::PlexAnswer]) {
    for resource in resources.iter_mut() {
        let has_local = resource
            .connections
            .iter()
            .any(|connection| connection.local && !connection.relay);
        if resource.https_required || has_local {
            continue;
        }
        for answer in found
            .iter()
            .filter(|answer| answer.machine_id == resource.id)
        {
            resource.connections.insert(
                0,
                Connection {
                    uri: answer.address.clone(),
                    local: true,
                    relay: false,
                },
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::servers::plex::testing::MACHINE;

    fn resource(https_required: bool, connections: Vec<Connection>) -> Resource {
        Resource {
            id: MACHINE.into(),
            name: "Den".into(),
            version: None,
            owned: true,
            access_token: "t".into(),
            https_required,
            connections,
        }
    }

    #[test]
    fn a_server_found_on_the_network_gains_a_local_address() {
        let answer = discovery::PlexAnswer {
            machine_id: MACHINE.into(),
            address: "http://192.168.1.20:32400".into(),
        };
        let remote = Connection {
            uri: "https://203-0-113-9.a1b2c3.plex.direct:32400".into(),
            local: false,
            relay: false,
        };
        let mut resources = vec![
            resource(false, vec![remote.clone()]),
            resource(true, vec![remote.clone()]),
        ];
        add_answers(&mut resources, std::slice::from_ref(&answer));
        assert_eq!(resources[0].connections[0].uri, answer.address);
        assert!(resources[0].connections[0].local);
        assert_eq!(resources[1].connections, [remote], "it requires https");
        // A server with a local address of its own keeps to it.
        let mut known = vec![resource(
            false,
            crate::servers::plex::testing::connections(),
        )];
        add_answers(&mut known, &[answer]);
        assert_eq!(known[0].connections.len(), 3);
    }
}
