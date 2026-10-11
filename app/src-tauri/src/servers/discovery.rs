//! Finding servers on the local network. Jellyfin's (PLAN.md Phase 7e): a
//! UDP broadcast of "Who is JellyfinServer?" to port 7359, which each
//! server answers with its address, id and name; the user picks one instead
//! of typing its address. Plex's (Phase 7f): GDM, a broadcast of an HTTP
//! `M-SEARCH` to port 32414, which each server answers with its machine id
//! and port; a server's address found this way is added to those plex.tv
//! lists for it. On macOS this needs the Local Network permission
//! (`NSLocalNetworkUsageDescription`); without it nothing answers, and the
//! address can still be typed (Jellyfin) or plex.tv's used (Plex).

use std::collections::HashSet;
use std::net::{IpAddr, Ipv4Addr, SocketAddr, UdpSocket};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use super::address;

/// Jellyfin's discovery port.
const PORT: u16 = 7359;

const QUESTION: &[u8] = b"Who is JellyfinServer?";

/// Plex's GDM port for servers.
const GDM_PORT: u16 = 32414;

const GDM_QUESTION: &[u8] = b"M-SEARCH * HTTP/1.1\r\n\r\n";

/// How long to listen for answers.
pub const LISTEN: Duration = Duration::from_millis(1500);

/// Answers larger than this aren't a server's.
const ANSWER_LIMIT: usize = 4096;

/// A server that answered.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS), ts(rename = "DiscoveredServer"))]
#[serde(rename_all = "camelCase")]
pub struct Discovered {
    /// As `address::parse` gives it.
    pub address: String,
    pub id: String,
    pub name: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct Answer {
    address: Option<String>,
    id: Option<String>,
    name: Option<String>,
}

/// A server's answer, if `bytes` is one with a usable address.
fn parse_answer(bytes: &[u8]) -> Option<Discovered> {
    let answer: Answer = serde_json::from_slice(bytes).ok()?;
    let address = address::parse(answer.address.as_deref()?).ok()?;
    let id = answer.id.filter(|id| !id.is_empty() && id.len() <= 64)?;
    Some(Discovered {
        address,
        id,
        name: answer
            .name
            .map(|name| name.chars().take(100).collect())
            .unwrap_or_else(|| "Jellyfin".into()),
    })
}

/// A Plex server that answered GDM.
#[derive(Debug, Clone, PartialEq)]
pub struct PlexAnswer {
    pub machine_id: String,
    /// Plain http at the address it answered from: "http://192.168.1.20:32400".
    pub address: String,
}

/// A Plex server's GDM answer from `from`, if `bytes` is one: HTTP-like
/// lines with its `Resource-Identifier` and `Port`.
fn parse_gdm(bytes: &[u8], from: IpAddr) -> Option<PlexAnswer> {
    let text = std::str::from_utf8(bytes).ok()?;
    let mut lines = text.lines();
    if !lines.next()?.contains("200") {
        return None;
    }
    let mut machine_id = None;
    let mut port = None;
    for line in lines {
        let Some((name, value)) = line.split_once(':') else {
            continue;
        };
        let value = value.trim();
        match name.trim().to_ascii_lowercase().as_str() {
            "resource-identifier" => machine_id = Some(value.to_owned()),
            "port" => port = value.parse::<u16>().ok(),
            "content-type" if !value.contains("plex/media-server") => return None,
            _ => {}
        }
    }
    let machine_id = machine_id.filter(|id| {
        !id.is_empty() && id.len() <= 64 && id.bytes().all(|b| b.is_ascii_alphanumeric())
    })?;
    let host = match from {
        IpAddr::V4(ip) => ip.to_string(),
        IpAddr::V6(ip) => format!("[{ip}]"),
    };
    Some(PlexAnswer {
        machine_id,
        address: format!("http://{host}:{}", port?),
    })
}

/// Asks the local network for Jellyfin servers and collects the answers
/// that come within `LISTEN`, each server once. Blocking.
pub fn discover() -> std::io::Result<Vec<Discovered>> {
    let found = ask(QUESTION, PORT, |bytes, _| {
        parse_answer(bytes).map(|server| (server.id.clone(), server))
    })?;
    log::info!("{} servers answered on the local network", found.len());
    Ok(found)
}

/// Asks the local network for Plex servers (GDM) and collects the answers
/// that come within `LISTEN`, each server once. Blocking.
pub fn discover_plex() -> std::io::Result<Vec<PlexAnswer>> {
    let found = ask(GDM_QUESTION, GDM_PORT, |bytes, from| {
        parse_gdm(bytes, from).map(|server| (server.machine_id.clone(), server))
    })?;
    log::info!("{} Plex servers answered on the local network", found.len());
    Ok(found)
}

/// Broadcasts `question` to `port` and collects what `parse` makes of the
/// answers from the local network within `LISTEN`, each id once.
fn ask<T>(
    question: &[u8],
    port: u16,
    parse: impl Fn(&[u8], IpAddr) -> Option<(String, T)>,
) -> std::io::Result<Vec<T>> {
    let socket = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0))?;
    socket.set_broadcast(true)?;
    socket.send_to(question, SocketAddr::from((Ipv4Addr::BROADCAST, port)))?;
    let deadline = Instant::now() + LISTEN;
    let mut found = Vec::new();
    let mut ids = HashSet::new();
    let mut buffer = [0u8; ANSWER_LIMIT];
    loop {
        let left = deadline.saturating_duration_since(Instant::now());
        if left.is_zero() {
            break;
        }
        socket.set_read_timeout(Some(left))?;
        match socket.recv_from(&mut buffer) {
            Ok((read, from)) => {
                // Only answers from the local network count.
                if !crate::remote::http::is_local(from.ip()) {
                    continue;
                }
                if let Some((id, server)) = parse(&buffer[..read], from.ip()) {
                    if ids.insert(id) {
                        found.push(server);
                    }
                }
            }
            Err(error)
                if matches!(
                    error.kind(),
                    std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                ) =>
            {
                break
            }
            Err(error) => return Err(error),
        }
    }
    Ok(found)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_answers() {
        let answer = br#"{"Address":"http://192.168.1.20:8096","Id":"4c5e9a1b","Name":"Living Room","EndpointAddress":null}"#;
        assert_eq!(
            parse_answer(answer),
            Some(Discovered {
                address: "http://192.168.1.20:8096".into(),
                id: "4c5e9a1b".into(),
                name: "Living Room".into(),
            })
        );
        for bad in [
            &b"Who is JellyfinServer?"[..],
            br#"{"Address":"not an address","Id":"x"}"#,
            br#"{"Address":"http://192.168.1.20:8096"}"#,
            br#"{"Address":"http://192.168.1.20:8096","Id":""}"#,
        ] {
            assert_eq!(parse_answer(bad), None);
        }
    }

    #[test]
    fn reads_plex_answers() {
        let from: IpAddr = "192.168.1.20".parse().unwrap();
        let answer = b"HTTP/1.0 200 OK\r\nContent-Type: plex/media-server\r\n\
            Resource-Identifier: 9d3f1c7a5b2e4f6081a3c5e7f9b1d3e5a7c9e1f3\r\n\
            Name: Den\r\nPort: 32400\r\nVersion: 1.41.9.9961-46083195d\r\n\r\n";
        assert_eq!(
            parse_gdm(answer, from),
            Some(PlexAnswer {
                machine_id: "9d3f1c7a5b2e4f6081a3c5e7f9b1d3e5a7c9e1f3".into(),
                address: "http://192.168.1.20:32400".into(),
            })
        );
        for bad in [
            &b"M-SEARCH * HTTP/1.1\r\n\r\n"[..],
            b"HTTP/1.0 200 OK\r\nContent-Type: plex/media-player\r\nResource-Identifier: x\r\nPort: 1\r\n",
            b"HTTP/1.0 200 OK\r\nResource-Identifier: ../x\r\nPort: 32400\r\n",
            b"HTTP/1.0 200 OK\r\nResource-Identifier: abc\r\n",
        ] {
            assert_eq!(parse_gdm(bad, from), None);
        }
    }
}
