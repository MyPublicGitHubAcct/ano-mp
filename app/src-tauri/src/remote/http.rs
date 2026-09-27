//! Just enough HTTP/1.1 for the remote page: one request per connection
//! (`Connection: close`), bounded sizes, and nothing the page doesn't use.
//! Kept apart from the server so it can be tested without sockets.

use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::IpAddr;

/// The request line and headers together may be at most this long.
const HEADER_LIMIT: usize = 8 << 10;
/// Request bodies (JSON commands) may be at most this long.
pub const BODY_LIMIT: usize = 16 << 10;

#[derive(Debug, Clone, PartialEq)]
pub struct Request {
    pub method: String,
    /// The path without the query.
    pub path: String,
    pub query: HashMap<String, String>,
    /// Lower-case names.
    pub headers: HashMap<String, String>,
    pub body: Vec<u8>,
}

impl Request {
    /// The bearer token, if the request carries one.
    pub fn bearer(&self) -> Option<&str> {
        self.headers
            .get("authorization")?
            .strip_prefix("Bearer ")
            .map(str::trim)
            .filter(|token| !token.is_empty())
    }
}

/// Reads one request; `Err` with a status for anything malformed or too big.
pub fn read_request(stream: impl Read) -> Result<Request, u16> {
    let mut reader = BufReader::new(stream.take((HEADER_LIMIT + BODY_LIMIT) as u64));
    let mut head = Vec::new();
    loop {
        let mut line = Vec::new();
        let read = reader.read_until(b'\n', &mut line).map_err(|_| 400u16)?;
        if read == 0 {
            return Err(400);
        }
        head.extend_from_slice(&line);
        if head.len() > HEADER_LIMIT {
            return Err(431);
        }
        if line == b"\r\n" || line == b"\n" {
            break;
        }
    }
    let head = String::from_utf8(head).map_err(|_| 400u16)?;
    let mut lines = head.lines();
    let mut request_line = lines.next().ok_or(400u16)?.split(' ');
    let method = request_line.next().ok_or(400u16)?.to_owned();
    let target = request_line.next().ok_or(400u16)?;
    if !request_line
        .next()
        .is_some_and(|version| version.starts_with("HTTP/1."))
    {
        return Err(400);
    }
    let (path, query) = target.split_once('?').unwrap_or((target, ""));
    let headers: HashMap<String, String> = lines
        .filter_map(|line| line.split_once(':'))
        .map(|(name, value)| (name.trim().to_ascii_lowercase(), value.trim().to_owned()))
        .collect();
    let length: usize = headers
        .get("content-length")
        .map_or(Ok(0), |length| length.parse().map_err(|_| 400u16))?;
    if length > BODY_LIMIT {
        return Err(413);
    }
    let mut body = vec![0; length];
    reader.read_exact(&mut body).map_err(|_| 400u16)?;
    Ok(Request {
        method,
        path: percent_decode(path),
        query: query
            .split('&')
            .filter(|pair| !pair.is_empty())
            .map(|pair| {
                let (name, value) = pair.split_once('=').unwrap_or((pair, ""));
                (
                    percent_decode(name),
                    percent_decode(&value.replace('+', " ")),
                )
            })
            .collect(),
        headers,
        body,
    })
}

fn percent_decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let hex = std::str::from_utf8(&bytes[i + 1..i + 3]).ok();
            if let Some(byte) = hex.and_then(|hex| u8::from_str_radix(hex, 16).ok()) {
                out.push(byte);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// A response to write.
#[derive(Debug, Clone, PartialEq)]
pub struct Response {
    pub status: u16,
    pub content_type: &'static str,
    pub body: Vec<u8>,
}

impl Response {
    pub fn json(value: &impl serde::Serialize) -> Response {
        Response {
            status: 200,
            content_type: "application/json",
            body: serde_json::to_vec(value).unwrap_or_default(),
        }
    }

    pub fn error(status: u16, message: &str) -> Response {
        Response {
            status,
            content_type: "application/json",
            body: serde_json::to_vec(&serde_json::json!({ "error": message })).unwrap_or_default(),
        }
    }

    pub fn write(&self, mut stream: impl Write) -> std::io::Result<()> {
        let reason = match self.status {
            200 => "OK",
            400 => "Bad Request",
            401 => "Unauthorized",
            403 => "Forbidden",
            404 => "Not Found",
            405 => "Method Not Allowed",
            413 => "Payload Too Large",
            429 => "Too Many Requests",
            431 => "Request Header Fields Too Large",
            _ => "Error",
        };
        write!(
            stream,
            "HTTP/1.1 {} {reason}\r\nContent-Type: {}\r\nContent-Length: {}\r\n\
             Cache-Control: no-store\r\nX-Content-Type-Options: nosniff\r\n\
             Referrer-Policy: no-referrer\r\n\
             Content-Security-Policy: default-src 'self'; style-src 'self' 'unsafe-inline'; \
             script-src 'self' 'unsafe-inline'; img-src 'self'\r\n\
             Connection: close\r\n\r\n",
            self.status,
            self.content_type,
            self.body.len()
        )?;
        stream.write_all(&self.body)?;
        stream.flush()
    }
}

/// Whether a peer is on the local network: loopback, private (RFC 1918),
/// link-local or IPv6 unique-local addresses. Anything else is refused.
pub fn is_local(address: IpAddr) -> bool {
    match address {
        IpAddr::V4(v4) => v4.is_loopback() || v4.is_private() || v4.is_link_local(),
        IpAddr::V6(v6) => {
            if let Some(v4) = v6.to_ipv4_mapped() {
                return is_local(IpAddr::V4(v4));
            }
            let first = v6.segments()[0];
            v6.is_loopback() || (first & 0xffc0) == 0xfe80 || (first & 0xfe00) == 0xfc00
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_requests() {
        let raw = b"POST /api/command?x=a%20b&y=1+2 HTTP/1.1\r\nHost: mac\r\n\
                    Authorization: Bearer abc\r\nContent-Length: 5\r\n\r\nhello";
        let request = read_request(&raw[..]).unwrap();
        assert_eq!(request.method, "POST");
        assert_eq!(request.path, "/api/command");
        assert_eq!(request.query["x"], "a b");
        assert_eq!(request.query["y"], "1 2");
        assert_eq!(request.bearer(), Some("abc"));
        assert_eq!(request.body, b"hello");
    }

    #[test]
    fn refuses_bad_or_big_requests() {
        assert_eq!(read_request(&b""[..]), Err(400));
        assert_eq!(read_request(&b"GARBAGE\r\n\r\n"[..]), Err(400));
        let big = format!("GET / HTTP/1.1\r\nX: {}\r\n\r\n", "a".repeat(HEADER_LIMIT));
        assert_eq!(read_request(big.as_bytes()), Err(431));
        let body = format!(
            "POST / HTTP/1.1\r\nContent-Length: {}\r\n\r\n",
            BODY_LIMIT + 1
        );
        assert_eq!(read_request(body.as_bytes()), Err(413));
        // A body shorter than it says.
        assert_eq!(
            read_request(&b"POST / HTTP/1.1\r\nContent-Length: 9\r\n\r\nabc"[..]),
            Err(400)
        );
    }

    #[test]
    fn writes_responses() {
        let mut out = Vec::new();
        Response::json(&serde_json::json!({"ok": true}))
            .write(&mut out)
            .unwrap();
        let text = String::from_utf8(out).unwrap();
        assert!(text.starts_with("HTTP/1.1 200 OK\r\n"));
        assert!(text.contains("Content-Length: 11\r\n"));
        assert!(text.ends_with("{\"ok\":true}"));
    }

    #[test]
    fn only_local_peers_are_local() {
        for local in [
            "127.0.0.1",
            "10.1.2.3",
            "172.16.0.9",
            "192.168.1.20",
            "169.254.3.4",
            "::1",
            "fe80::1",
            "fd00::5",
            "::ffff:192.168.0.2",
        ] {
            assert!(is_local(local.parse().unwrap()), "{local}");
        }
        for public in ["8.8.8.8", "172.32.0.1", "2001:4860::8888", "::ffff:1.1.1.1"] {
            assert!(!is_local(public.parse().unwrap()), "{public}");
        }
    }
}
