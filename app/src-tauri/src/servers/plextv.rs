//! plex.tv (PLAN.md Phase 7f), as far as the app uses it: signing in with
//! a PIN the user approves in their browser (so no password is typed into
//! the app), the account's name, and the servers it can reach with their
//! own tokens and addresses. These are the only calls to plex.tv: the music
//! comes from the user's server (`plex`).
//!
//! The sign-in is Plex's PIN flow: `POST /api/v2/pins?strong=true` gives a
//! code, the user approves it at app.plex.tv (`auth_url`), and the pin then
//! holds the account's token. Plex now recommends a flow with a device key
//! and tokens that last a week (PLAN.md 7f.7); this older one still works.

use serde::{Deserialize, Serialize};

use super::plex::client_headers;
use super::Device;
use crate::coded::coded;
use crate::metadata::http::{percent_encode, Client, JSON_LIMIT};
use crate::metadata::Error;

/// plex.tv's API for accounts and pins.
const PLEX_TV: &str = "https://plex.tv";

/// plex.tv's API for the servers an account reaches.
const CLIENTS: &str = "https://clients.plex.tv";

/// Where the user approves a pin.
const AUTH_APP: &str = "https://app.plex.tv/auth";

/// A signed-in (or signing-in) conversation with plex.tv.
#[derive(Clone, Copy)]
pub struct PlexTv<'a> {
    pub http: &'a Client,
    pub device: &'a Device,
    /// The account's token, once signed in.
    pub token: Option<&'a str>,
}

/// A sign-in under way: the pin's id, to ask about it, and its code, which
/// the page at `auth_url` shows the user.
#[derive(Debug, Clone, PartialEq)]
pub struct Pin {
    pub id: i64,
    pub code: String,
}

/// A Plex account.
#[derive(Debug, Clone, PartialEq)]
pub struct Account {
    pub id: String,
    pub name: String,
}

/// One of the addresses a server answers at, as plex.tv lists it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Connection {
    /// "https://192-168-1-20.<hash>.plex.direct:32400", or plain http.
    pub uri: String,
    /// On the server's own network.
    #[serde(default)]
    pub local: bool,
    /// Plex's relay, which is slow and capped: a last resort.
    #[serde(default)]
    pub relay: bool,
}

/// A server the account can reach.
#[derive(Debug, Clone, PartialEq)]
pub struct Resource {
    /// The server's machine identifier.
    pub id: String,
    pub name: String,
    pub version: Option<String>,
    /// Whether the account owns it, rather than has it shared.
    pub owned: bool,
    /// The token for this server, which differs from the account's for a
    /// shared server.
    pub access_token: String,
    /// Whether it refuses plain http.
    pub https_required: bool,
    /// Its addresses, without plain http ones when it requires https.
    pub connections: Vec<Connection>,
}

impl PlexTv<'_> {
    fn headers(&self) -> Vec<(&'static str, String)> {
        client_headers(self.device, self.token)
    }

    fn get<T: for<'de> Deserialize<'de>>(&self, url: &str) -> Result<T, Error> {
        let headers = self.headers();
        let headers: Vec<(&str, &str)> = headers.iter().map(|(n, v)| (*n, v.as_str())).collect();
        let response = self
            .http
            .get_with_headers(url, "application/json", &headers, JSON_LIMIT)?;
        serde_json::from_slice(&response.body).map_err(|error| Error::Invalid(format!("{error}")))
    }

    /// Starts signing in: a new pin.
    pub fn create_pin(&self) -> Result<Pin, Error> {
        let headers = self.headers();
        let headers: Vec<(&str, &str)> = headers.iter().map(|(n, v)| (*n, v.as_str())).collect();
        let response = self.http.post_with_headers(
            &format!("{PLEX_TV}/api/v2/pins?strong=true"),
            "application/x-www-form-urlencoded",
            "",
            &headers,
        )?;
        let pin: PinDto = serde_json::from_slice(&response.body)
            .map_err(|error| Error::Invalid(format!("{error}")))?;
        if pin.id == 0 || pin.code.is_empty() {
            return Err(Error::Invalid("plex.tv's pin had no id or code".into()));
        }
        Ok(Pin {
            id: pin.id,
            code: pin.code,
        })
    }

    /// The page where the user approves pin `code`, signing in to Plex
    /// there if they need to.
    pub fn auth_url(&self, code: &str) -> String {
        format!(
            "{AUTH_APP}#?clientID={}&code={}&context%5Bdevice%5D%5Bproduct%5D=ano-mp",
            percent_encode(&self.device.id),
            percent_encode(code)
        )
    }

    /// The account's token once the user approved pin `id`, `None` until
    /// then. Fails with `plexPinExpired` once the pin has gone.
    pub fn pin_token(&self, id: i64) -> Result<Option<String>, Error> {
        let pin: PinDto = self
            .get(&format!("{PLEX_TV}/api/v2/pins/{id}"))
            .map_err(|error| match error {
                Error::Status {
                    status: 404 | 410, ..
                } => Error::Invalid(pin_expired()),
                other => other,
            })?;
        Ok(pin.auth_token.filter(|token| !token.is_empty()))
    }

    /// The signed-in account.
    pub fn account(&self) -> Result<Account, Error> {
        let user: UserDto = self.get(&format!("{PLEX_TV}/api/v2/user"))?;
        if user.id == 0 {
            return Err(Error::Invalid("plex.tv's account had no id".into()));
        }
        Ok(Account {
            id: user.id.to_string(),
            name: user
                .username
                .or(user.title)
                .filter(|name| !name.is_empty())
                .unwrap_or_else(|| "Plex".into()),
        })
    }

    /// The servers the account reaches, each with its token and addresses.
    pub fn servers(&self) -> Result<Vec<Resource>, Error> {
        let resources: Vec<ResourceDto> = self.get(&format!(
            "{CLIENTS}/api/v2/resources?includeHttps=1&includeRelay=1&includeIPv6=1"
        ))?;
        Ok(resources
            .into_iter()
            .filter(|resource| {
                resource
                    .provides
                    .split(',')
                    .any(|role| role.trim() == "server")
            })
            .filter_map(|resource| {
                let token = resource.access_token.filter(|token| !token.is_empty())?;
                if resource.client_identifier.is_empty() {
                    return None;
                }
                let https_required = resource.https_required;
                let connections = resource
                    .connections
                    .into_iter()
                    .filter(|connection| {
                        connection.uri.starts_with("https://")
                            || (connection.uri.starts_with("http://") && !https_required)
                    })
                    .collect();
                Some(Resource {
                    id: resource.client_identifier,
                    name: resource.name.unwrap_or_else(|| "Plex".into()),
                    version: resource.product_version,
                    owned: resource.owned,
                    access_token: token,
                    https_required,
                    connections,
                })
            })
            .collect())
    }
}

/// The error of a pin that has expired or gone.
fn pin_expired() -> String {
    coded(
        "plexPinExpired",
        &[],
        "The Plex sign-in took too long; start again",
    )
}

/// plex.tv's error as one the UI can show: a token it no longer accepts is
/// `plexSignedOut`; the rest as `api::error_text` words them.
pub fn error_text(error: &Error) -> String {
    match error {
        Error::Status { status: 401, .. } => coded(
            "plexSignedOut",
            &[],
            "plex.tv no longer accepts this sign-in; sign in with Plex again",
        ),
        other => super::api::error_text(other, "plex.tv"),
    }
}

// What plex.tv sends, as far as it's read.

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
struct PinDto {
    id: i64,
    code: String,
    auth_token: Option<String>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct UserDto {
    id: i64,
    username: Option<String>,
    title: Option<String>,
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
struct ResourceDto {
    name: Option<String>,
    product_version: Option<String>,
    client_identifier: String,
    provides: String,
    owned: bool,
    access_token: Option<String>,
    https_required: bool,
    connections: Vec<Connection>,
}

#[cfg(test)]
pub mod testing {
    //! plex.tv for tests, answering from the hand-made responses in
    //! `metadata/fixtures/plex/` (made as `manifest.json` says).

    pub const PIN: &str = include_str!("../metadata/fixtures/plex/pin.json");
    pub const PIN_CLAIMED: &str = include_str!("../metadata/fixtures/plex/pin-claimed.json");
    pub const USER: &str = include_str!("../metadata/fixtures/plex/user.json");
    pub const RESOURCES: &str = include_str!("../metadata/fixtures/plex/resources.json");

    pub const PINS_URL: &str = "https://plex.tv/api/v2/pins?strong=true";
    pub const PIN_URL: &str = "https://plex.tv/api/v2/pins/48213977";
    pub const USER_URL: &str = "https://plex.tv/api/v2/user";
    pub const RESOURCES_URL: &str =
        "https://clients.plex.tv/api/v2/resources?includeHttps=1&includeRelay=1&includeIPv6=1";

    /// The account's own token in the fixtures.
    pub const ACCOUNT_TOKEN: &str = "pQ7xYz3kLm9nB2vC4dF6";
}

#[cfg(test)]
mod tests {
    use super::testing::*;
    use super::*;
    use crate::metadata::http::testing::fake_client;
    use crate::servers::jellyfin::testing::device;

    fn header<'a>(headers: &'a [(String, String)], name: &str) -> Option<&'a str> {
        headers
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, v)| v.as_str())
    }

    #[test]
    fn signs_in_with_a_pin_approved_in_the_browser() {
        let (http, transport, _) = fake_client();
        let device = device();
        let plex_tv = PlexTv {
            http: &http,
            device: &device,
            token: None,
        };
        transport.push_status(PINS_URL, 201, PIN);
        let pin = plex_tv.create_pin().unwrap();
        assert_eq!(
            pin,
            Pin {
                id: 48213977,
                code: "7tq2bxkd8m3c5wq9rjf4p6zhn".into()
            }
        );
        assert_eq!(
            plex_tv.auth_url(&pin.code),
            "https://app.plex.tv/auth#?clientID=0123456789abcdef0123456789abcdef\
             &code=7tq2bxkd8m3c5wq9rjf4p6zhn&context%5Bdevice%5D%5Bproduct%5D=ano-mp"
        );
        // Who the app is, on every request; no token before signing in.
        let sent = transport.headers.lock().unwrap()[0].clone();
        assert_eq!(
            header(&sent, "X-Plex-Client-Identifier"),
            Some("0123456789abcdef0123456789abcdef")
        );
        assert_eq!(header(&sent, "X-Plex-Product"), Some("ano-mp"));
        assert_eq!(header(&sent, "X-Plex-Token"), None);

        transport.push_status(PIN_URL, 200, PIN);
        transport.push_status(PIN_URL, 200, PIN_CLAIMED);
        assert_eq!(plex_tv.pin_token(pin.id).unwrap(), None);
        assert_eq!(
            plex_tv.pin_token(pin.id).unwrap().as_deref(),
            Some(ACCOUNT_TOKEN)
        );
        transport.push_status(PIN_URL, 404, "");
        let error = plex_tv.pin_token(pin.id).unwrap_err();
        assert!(crate::coded::is(&error, "plexPinExpired"), "{error}");
    }

    #[test]
    fn names_the_account_and_lists_its_servers() {
        let (http, transport, _) = fake_client();
        let device = device();
        let plex_tv = PlexTv {
            http: &http,
            device: &device,
            token: Some(ACCOUNT_TOKEN),
        };
        transport.push_status(USER_URL, 200, USER);
        assert_eq!(
            plex_tv.account().unwrap(),
            Account {
                id: "31415926".into(),
                name: "mark".into()
            }
        );
        transport.push_status(RESOURCES_URL, 200, RESOURCES);
        let servers = plex_tv.servers().unwrap();
        assert_eq!(servers.len(), 2, "players aren't servers");
        let den = &servers[0];
        assert_eq!(den.id, "9d3f1c7a5b2e4f6081a3c5e7f9b1d3e5a7c9e1f3");
        assert_eq!(den.name, "Den");
        assert!(den.owned);
        assert_eq!(den.access_token, "srvTok3n8Kq2Lm5Np7Rs");
        assert_eq!(den.connections.len(), 3);
        assert!(den.connections[0].local && !den.connections[0].relay);
        assert!(den.connections[2].relay);
        // A server that requires https loses its plain http address.
        let shared = &servers[1];
        assert!(!shared.owned);
        assert!(shared
            .connections
            .iter()
            .all(|connection| connection.uri.starts_with("https://")));
        // The token goes in a header, never a URL.
        let sent = transport.headers.lock().unwrap()[1].clone();
        assert_eq!(header(&sent, "X-Plex-Token"), Some(ACCOUNT_TOKEN));
        assert!(transport.urls().iter().all(|url| !url.contains("Token")));

        transport.push_status(USER_URL, 401, "");
        let error = error_text(&plex_tv.account().unwrap_err());
        assert!(crate::coded::is(&error, "plexSignedOut"), "{error}");
    }

    #[test]
    fn connections_round_trip_through_the_database_json() {
        let connection = Connection {
            uri: "https://10-0-0-2.abc.plex.direct:32400".into(),
            local: true,
            relay: false,
        };
        let json = serde_json::to_string(std::slice::from_ref(&connection)).unwrap();
        assert_eq!(
            serde_json::from_str::<Vec<Connection>>(&json).unwrap(),
            [connection]
        );
    }

    /// Needs a Plex account's token in `PLEX_TOKEN` (7f.1): names the
    /// account and lists its servers.
    #[test]
    #[ignore = "contacts plex.tv"]
    fn live_plex_tv() {
        use crate::metadata::http::{SystemClock, UreqTransport};
        let token = std::env::var("PLEX_TOKEN").expect("PLEX_TOKEN is set");
        let http = Client::new(Box::new(UreqTransport::new()), Box::new(SystemClock));
        let device = device();
        let plex_tv = PlexTv {
            http: &http,
            device: &device,
            token: Some(&token),
        };
        assert!(!plex_tv.account().unwrap().id.is_empty());
        plex_tv.servers().unwrap();
    }
}
