//! Where a music server is, and whether the app may talk to it there
//! (PLAN.md Phase 7e). An access token and the user's music cross the
//! network, so plain `http://` is accepted only for a server on the local
//! network (as the LAN remote decides what is local), and anything else
//! needs `https://` with a certificate the system trusts: self-signed
//! certificates are refused (the owner's decision, 2026-10-11).

use std::net::{IpAddr, ToSocketAddrs};

use serde_json::json;
use url::{Host, Url};

use crate::coded::coded;
use crate::remote::http::is_local;

/// Jellyfin's port for plain http, assumed for an address typed without a
/// scheme or port.
const DEFAULT_HTTP_PORT: u16 = 8096;

/// The address the user typed (or discovery found) as a base URL: scheme,
/// host, port if any, and path (a server behind a reverse proxy may live
/// under one), without a trailing '/'. An address without a scheme is
/// `http://` on Jellyfin's port when it names an IP address or a single
/// word or `.local` name, else `https://`. Fails with `serverAddress`.
pub fn parse(input: &str) -> Result<String, String> {
    let input = input.trim();
    let invalid = || {
        coded(
            "serverAddress",
            &[],
            "That isn't a server address, such as http://192.168.1.10:8096",
        )
    };
    if input.is_empty() || input.chars().any(char::is_whitespace) {
        return Err(invalid());
    }
    let with_scheme = if input.contains("://") {
        input.to_owned()
    } else {
        let host = input.split(['/', ':']).next().unwrap_or("");
        let homely = host.parse::<IpAddr>().is_ok()
            || input.starts_with('[')
            || !host.contains('.')
            || host.ends_with(".local");
        if homely {
            let has_port = input.rsplit_once(':').is_some_and(|(_, port)| {
                port.split('/')
                    .next()
                    .is_some_and(|p| p.parse::<u16>().is_ok())
            });
            if has_port {
                format!("http://{input}")
            } else {
                let (host, path) = input.split_once('/').unwrap_or((input, ""));
                let path = if path.is_empty() {
                    String::new()
                } else {
                    format!("/{path}")
                };
                format!("http://{host}:{DEFAULT_HTTP_PORT}{path}")
            }
        } else {
            format!("https://{input}")
        }
    };
    let url = Url::parse(&with_scheme).map_err(|_| invalid())?;
    if !matches!(url.scheme(), "http" | "https")
        || url.host().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err(invalid());
    }
    Ok(url.as_str().trim_end_matches('/').to_owned())
}

/// Looks up a host's addresses; the system's resolver, or a test's.
pub trait Resolver {
    fn resolve(&self, host: &str, port: u16) -> Vec<IpAddr>;
}

/// The system's resolver (which answers `.local` names through mDNS on
/// macOS).
pub struct System;

impl Resolver for System {
    fn resolve(&self, host: &str, port: u16) -> Vec<IpAddr> {
        (host, port)
            .to_socket_addrs()
            .map(|addresses| addresses.map(|address| address.ip()).collect())
            .unwrap_or_default()
    }
}

/// Whether the server at `address` (as `parse` gives it) is on the local
/// network: its host is a local IP address, `localhost`, or a name all of
/// whose addresses are local. A name that doesn't resolve isn't.
pub fn is_on_local_network(address: &str, resolver: &impl Resolver) -> bool {
    let Ok(url) = Url::parse(address) else {
        return false;
    };
    let port = url.port_or_known_default().unwrap_or(DEFAULT_HTTP_PORT);
    match url.host() {
        Some(Host::Ipv4(ip)) => is_local(IpAddr::V4(ip)),
        Some(Host::Ipv6(ip)) => is_local(IpAddr::V6(ip)),
        Some(Host::Domain(name)) if name.eq_ignore_ascii_case("localhost") => true,
        Some(Host::Domain(name)) => {
            let addresses = resolver.resolve(name, port);
            !addresses.is_empty() && addresses.into_iter().all(is_local)
        }
        None => false,
    }
}

/// Whether the app may send a token to `address`: always over https, and
/// over plain http only on the local network. Fails with `serverInsecure`.
pub fn check(address: &str, resolver: &impl Resolver) -> Result<(), String> {
    if address.starts_with("https://") || is_on_local_network(address, resolver) {
        return Ok(());
    }
    Err(coded(
        "serverInsecure",
        &[("address", json!(address))],
        "A server outside your network needs an https:// address",
    ))
}

#[cfg(test)]
pub mod testing {
    use std::collections::HashMap;
    use std::net::IpAddr;

    use super::Resolver;

    /// Names and their addresses, for tests.
    #[derive(Default)]
    pub struct FakeResolver(pub HashMap<String, Vec<IpAddr>>);

    impl Resolver for FakeResolver {
        fn resolve(&self, host: &str, _port: u16) -> Vec<IpAddr> {
            self.0.get(host).cloned().unwrap_or_default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::testing::FakeResolver;
    use super::*;

    #[test]
    fn parses_what_people_type() {
        for (typed, parsed) in [
            ("192.168.1.10", "http://192.168.1.10:8096"),
            ("192.168.1.10:8920", "http://192.168.1.10:8920"),
            ("nas", "http://nas:8096"),
            ("nas.local/jellyfin", "http://nas.local:8096/jellyfin"),
            ("music.example.org", "https://music.example.org"),
            (
                "https://music.example.org/jf/",
                "https://music.example.org/jf",
            ),
            ("http://[fd00::5]:8096", "http://[fd00::5]:8096"),
            ("  HTTP://Nas:8096  ", "http://nas:8096"),
        ] {
            assert_eq!(parse(typed).as_deref(), Ok(parsed), "{typed}");
        }
        for bad in [
            "",
            "two words",
            "ftp://nas",
            "https://user:pw@music.example.org",
            "https://music.example.org/?q=1",
            "https://music.example.org/#x",
            "http://",
        ] {
            let error = parse(bad).unwrap_err();
            assert!(crate::coded::is(&error, "serverAddress"), "{bad}: {error}");
        }
    }

    #[test]
    fn plain_http_only_on_the_local_network() {
        let resolver = FakeResolver(
            [
                (
                    "nas.local".to_owned(),
                    vec!["192.168.1.10".parse().unwrap()],
                ),
                (
                    "mixed.example".to_owned(),
                    vec!["10.0.0.2".parse().unwrap(), "8.8.8.8".parse().unwrap()],
                ),
                (
                    "music.example.org".to_owned(),
                    vec!["93.184.216.34".parse().unwrap()],
                ),
            ]
            .into(),
        );
        for allowed in [
            "http://192.168.1.10:8096",
            "http://127.0.0.1:8096",
            "http://localhost:8096",
            "http://[fd00::5]:8096",
            "http://nas.local:8096",
            "https://music.example.org",
            "https://203.0.113.9",
        ] {
            assert!(check(allowed, &resolver).is_ok(), "{allowed}");
        }
        for refused in [
            "http://music.example.org",
            "http://203.0.113.9:8096",
            "http://mixed.example:8096",
            "http://unknown.example:8096",
        ] {
            let error = check(refused, &resolver).unwrap_err();
            assert!(crate::coded::is(&error, "serverInsecure"), "{refused}");
        }
        assert!(!is_on_local_network("https://music.example.org", &resolver));
        assert!(is_on_local_network("https://nas.local", &resolver));
    }
}
