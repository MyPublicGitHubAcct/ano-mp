//! One interface to every kind of music server (PLAN.md Phase 7f), for the
//! work that doesn't care which it is: checking that the server answers,
//! reading its music (`sync`), fetching a track or a picture
//! (`fetch`, `commands`), reporting what plays (`playing`) and signing
//! out. Each kind's own calls stay in its module (`jellyfin`, `plex`).

use serde_json::json;

use super::cache::Form;
use super::fetch::RemoteFile;
use super::jellyfin::{self, Jellyfin};
use super::plex::{self, Plex};
use super::sync::ServerLibrary;
use super::{Device, ServerInfo, Source};
use crate::coded::coded;
use crate::library::FolderKind;
use crate::metadata::http::Client;
use crate::metadata::Error;

/// A request to a server: where, and the headers that say who asks.
#[derive(Debug, Clone, PartialEq)]
pub struct Request {
    pub url: String,
    pub headers: Vec<(&'static str, String)>,
}

impl Request {
    /// The headers as `http::Client` takes them.
    pub fn headers(&self) -> Vec<(&str, &str)> {
        self.headers
            .iter()
            .map(|(name, value)| (*name, value.as_str()))
            .collect()
    }
}

/// A conversation with a source's server, whatever its kind.
#[derive(Clone, Copy)]
pub enum ServerApi<'a> {
    Jellyfin(Jellyfin<'a>),
    Plex(Plex<'a>),
}

impl<'a> ServerApi<'a> {
    /// The server of `source`, at its address, with `token`.
    pub fn new(
        http: &'a Client,
        device: &'a Device,
        source: &'a Source,
        token: Option<&'a str>,
    ) -> ServerApi<'a> {
        match source.kind {
            FolderKind::Plex => ServerApi::Plex(Plex {
                http,
                address: &source.address,
                device,
                token,
            }),
            FolderKind::Jellyfin | FolderKind::Local => ServerApi::Jellyfin(Jellyfin {
                http,
                address: &source.address,
                device,
                token,
            }),
        }
    }

    pub fn http(&self) -> &'a Client {
        match self {
            ServerApi::Jellyfin(server) => server.http,
            ServerApi::Plex(server) => server.http,
        }
    }

    /// The headers every request to the server sends.
    pub fn headers(&self) -> Vec<(&'static str, String)> {
        match self {
            ServerApi::Jellyfin(server) => vec![("Authorization", server.authorization())],
            ServerApi::Plex(server) => server.headers(),
        }
    }

    /// Who answers at the source's address: the source's own server, or
    /// an error (`serverNotPlex` for another Plex server).
    pub fn info(&self, source: &Source) -> Result<ServerInfo, Error> {
        match self {
            ServerApi::Jellyfin(server) => server.public_info(),
            ServerApi::Plex(server) => {
                let identity = server.identity()?;
                if identity.machine_id != source.server_id {
                    return Err(Error::Invalid(coded(
                        "serverNotPlex",
                        &[],
                        "Another Plex server answers at that address",
                    )));
                }
                Ok(ServerInfo {
                    id: identity.machine_id,
                    name: source.server_name.clone(),
                    version: identity.version.or_else(|| source.version.clone()),
                })
            }
        }
    }

    /// The source's music, as the sync reads it.
    pub fn library(&self, source: &'a Source) -> Box<dyn ServerLibrary + 'a> {
        match *self {
            ServerApi::Jellyfin(server) => Box::new(jellyfin::Library {
                server,
                user_id: &source.user_id,
            }),
            ServerApi::Plex(server) => Box::new(plex::Library {
                server,
                machine_id: &source.server_id,
                section: &source.section,
            }),
        }
    }

    /// Where and how to fetch `file` in `form`; `None` when the server
    /// can't give it in that form (a Plex track whose part isn't known).
    pub fn fetch(&self, file: &RemoteFile, form: Form) -> Option<Request> {
        let headers = self.headers();
        match (self, form) {
            (ServerApi::Jellyfin(server), Form::Original) => Some(Request {
                url: server.original_url(&file.item_id),
                headers,
            }),
            (ServerApi::Jellyfin(server), Form::Mp3(kbps)) => Some(Request {
                url: server.mp3_url(&file.item_id, kbps),
                headers,
            }),
            (ServerApi::Plex(server), Form::Original) => Some(Request {
                url: server.original_url(file.media_path.as_deref()?)?,
                headers,
            }),
            (ServerApi::Plex(server), Form::Mp3(kbps)) => {
                let mut headers = headers;
                headers.push(("X-Plex-Client-Profile-Extra", plex::MP3_PROFILE.to_owned()));
                Some(Request {
                    url: server.mp3_url(&file.item_id, kbps),
                    headers,
                })
            }
        }
    }

    /// Where and how to fetch the picture the sync recorded as `art`
    /// (`ServerTrack::art_url`).
    pub fn art(&self, art: &str) -> Option<Request> {
        let url = match self {
            ServerApi::Jellyfin(_) => art.to_owned(),
            ServerApi::Plex(server) => server.art_url(art)?,
        };
        Some(Request {
            url,
            headers: self.headers(),
        })
    }

    /// Tells the server `file` started playing here.
    pub fn playing(&self, file: &RemoteFile) -> Result<(), Error> {
        match self {
            ServerApi::Jellyfin(server) => server.playing(&file.item_id),
            ServerApi::Plex(server) => {
                server.timeline(&file.item_id, "playing", 0.0, file.duration)
            }
        }
    }

    /// Tells the server `file` stopped `seconds` in.
    pub fn stopped(&self, file: &RemoteFile, seconds: f64) -> Result<(), Error> {
        match self {
            ServerApi::Jellyfin(server) => server.stopped(&file.item_id, seconds),
            ServerApi::Plex(server) => {
                server.timeline(&file.item_id, "stopped", seconds, file.duration)
            }
        }
    }

    /// Ends this device's session on the server, where it has one to end:
    /// a Plex server's token is the account's, ended at plex.tv.
    pub fn sign_out(&self) -> Result<(), Error> {
        match self {
            ServerApi::Jellyfin(server) => server.sign_out(),
            ServerApi::Plex(_) => Ok(()),
        }
    }
}

/// A server's error as one the UI can show: one it no longer accepts the
/// token for (401) is `serverSignedOut`, one that can't be reached
/// `serverUnreachable`, a certificate the system doesn't trust
/// `serverCertificate`, any other status `serverError`. Coded errors pass
/// through.
pub fn error_text(error: &Error, server: &str) -> String {
    match error {
        Error::Status { status: 401, .. } => coded(
            "serverSignedOut",
            &[("server", json!(server))],
            format!("{server} no longer accepts this sign-in; sign in again"),
        ),
        Error::Status { status, .. } => coded(
            "serverError",
            &[("server", json!(server)), ("status", json!(status))],
            format!("{server} answered with an error (HTTP {status})"),
        ),
        Error::Offline(_) => coded(
            "serverUnreachable",
            &[("server", json!(server))],
            format!("{server} can't be reached"),
        ),
        Error::Invalid(message) if serde_json::from_str::<serde_json::Value>(message).is_ok() => {
            message.clone()
        }
        Error::Invalid(message) if is_certificate_error(message) => coded(
            "serverCertificate",
            &[("server", json!(server))],
            format!(
                "{server}'s https certificate isn't one the system trusts; \
                 self-signed certificates aren't supported"
            ),
        ),
        other => other.to_string(),
    }
}

/// Whether a transport error is a TLS certificate the system refused.
fn is_certificate_error(message: &str) -> bool {
    let lower = message.to_ascii_lowercase();
    lower.contains("certificate") || lower.contains("unknownissuer") || lower.contains("x509")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::library::db;
    use crate::metadata::http::testing::fake_client;
    use crate::servers::jellyfin::testing::device;
    use crate::servers::plex::testing as plex_server;
    use crate::servers::testing::{add_plex_source, add_test_source};

    fn file(kind: FolderKind, media_path: Option<&str>) -> RemoteFile {
        RemoteFile {
            folder_id: 1,
            kind,
            source_key: String::new(),
            item_id: "1001".into(),
            size: 0,
            container: Some("flac".into()),
            media_path: media_path.map(str::to_owned),
            duration: 180.0,
        }
    }

    #[test]
    fn each_kind_fetches_in_its_own_way() {
        let mut conn = db::open_in_memory().unwrap();
        let jellyfin = add_test_source(&mut conn);
        let plex = add_plex_source(&mut conn, "1", "Music");
        let jellyfin = crate::servers::source(&conn, jellyfin).unwrap().unwrap();
        let plex = crate::servers::source(&conn, plex).unwrap().unwrap();
        let (http, _, _) = fake_client();
        let device = device();

        let api = ServerApi::new(&http, &device, &jellyfin, Some("t"));
        let request = api
            .fetch(&file(FolderKind::Jellyfin, None), Form::Original)
            .unwrap();
        assert!(request.url.ends_with("/Audio/1001/stream?static=true"));
        assert_eq!(request.headers[0].0, "Authorization");

        let api = ServerApi::new(&http, &device, &plex, Some(plex_server::SERVER_TOKEN));
        let path = "/library/parts/827/1712000000/file.flac";
        let request = api
            .fetch(&file(FolderKind::Plex, Some(path)), Form::Original)
            .unwrap();
        assert_eq!(request.url, plex_server::url(path));
        assert!(request
            .headers()
            .contains(&("X-Plex-Token", plex_server::SERVER_TOKEN)));
        // Without its part, only a transcode.
        assert_eq!(
            api.fetch(&file(FolderKind::Plex, None), Form::Original),
            None
        );
        let request = api
            .fetch(&file(FolderKind::Plex, None), Form::Mp3(320))
            .unwrap();
        assert!(request
            .headers()
            .contains(&("X-Plex-Client-Profile-Extra", plex::MP3_PROFILE)));
        // Pictures: Jellyfin's URL as it is, Plex's key at the address in use.
        let key = plex::art_key(plex_server::MACHINE, "/library/metadata/1000/thumb/1");
        assert!(api
            .art(&key)
            .unwrap()
            .url
            .starts_with(&plex_server::url("/photo/:/transcode")));
    }

    #[test]
    fn a_plex_server_must_be_the_sources_own() {
        let mut conn = db::open_in_memory().unwrap();
        let plex = add_plex_source(&mut conn, "1", "Music");
        let source = crate::servers::source(&conn, plex).unwrap().unwrap();
        let (http, transport, _) = fake_client();
        let device = device();
        let api = ServerApi::new(&http, &device, &source, None);
        transport.push_status(&plex_server::url("/identity"), 200, plex_server::IDENTITY);
        let info = api.info(&source).unwrap();
        assert_eq!(info.name, "Den");
        assert_eq!(info.version.as_deref(), Some("1.41.9.9961-46083195d"));
        transport.push_status(
            &plex_server::url("/identity"),
            200,
            r#"{"MediaContainer":{"machineIdentifier":"someone-else"}}"#,
        );
        let error = api.info(&source).unwrap_err();
        assert!(crate::coded::is(&error.to_string(), "serverNotPlex"));
    }

    #[test]
    fn errors_the_ui_can_show() {
        let unauthorised = Error::Status {
            url: "x".into(),
            status: 401,
        };
        assert!(crate::coded::is(
            &error_text(&unauthorised, "Home"),
            "serverSignedOut"
        ));
        let offline = Error::Offline("down".into());
        assert!(crate::coded::is(
            &error_text(&offline, "Home"),
            "serverUnreachable"
        ));
        let tls = Error::Invalid("https://x: invalid peer certificate: UnknownIssuer".into());
        assert!(crate::coded::is(
            &error_text(&tls, "Home"),
            "serverCertificate"
        ));
        let coded = Error::Invalid(crate::coded::scan_running());
        assert!(crate::coded::is(&error_text(&coded, "Home"), "scanRunning"));
    }
}
