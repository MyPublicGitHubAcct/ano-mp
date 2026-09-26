//! HTTP for the metadata services. `Client` puts every request through a
//! per-host rate limiter, retries when a service says it is busy (503/429),
//! and stops calling a host for a while after it couldn't be reached, so the
//! app degrades quietly when offline. It fetches through a `Transport`: ureq
//! in the app, a fake in tests, which never touch the network.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use rusqlite::Connection;

use super::{cache, Error};
use crate::anomp;

/// Where service operators can reach whoever runs this client. MusicBrainz
/// throttles clients whose `User-Agent` has no contact (PLAN.md §8.1).
const CONTACT: &str = "https://github.com/MyPublicGitHubAcct/ano-mp";

/// The `User-Agent` sent with every request, e.g.
/// "ano-mp/0.1.0 ( https://… )".
pub fn user_agent() -> String {
    format!("ano-mp/{} ( {CONTACT} )", anomp::version())
}

/// JSON responses larger than this are refused.
pub const JSON_LIMIT: u64 = 8 << 20;

/// Images larger than this are refused.
pub const IMAGE_LIMIT: u64 = 32 << 20;

/// Minimum time between requests to a host. MusicBrainz allows about one a
/// second per IP address and answers 503 beyond it.
fn request_interval(host: &str) -> Duration {
    match host {
        "musicbrainz.org" => Duration::from_secs(1),
        _ => Duration::from_millis(250),
    }
}

/// Tries of a request that a busy service turns away (503/429).
const BUSY_TRIES: u32 = 3;

/// How long to wait after a busy answer without a `Retry-After`.
const BUSY_WAIT: Duration = Duration::from_secs(2);

/// Longest `Retry-After` honoured; a longer one fails the request instead.
const BUSY_WAIT_MAX: Duration = Duration::from_secs(30);

/// After a host can't be reached it isn't called for this long, doubling
/// with each failure up to `UNREACHABLE_MAX`.
const UNREACHABLE_FIRST: Duration = Duration::from_secs(60);
const UNREACHABLE_MAX: Duration = Duration::from_secs(30 * 60);

#[derive(Debug, Clone, PartialEq)]
pub struct Response {
    pub status: u16,
    pub content_type: Option<String>,
    /// From the `Retry-After` header, in seconds.
    pub retry_after: Option<Duration>,
    pub body: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TransportError {
    /// No answer: no network, DNS failure, refused connection, timeout.
    Unreachable(String),
    /// Anything else, e.g. a TLS failure or an oversized body.
    Failed(String),
}

/// Fetches a URL, following redirects. A response with an error status is
/// still a response.
pub trait Transport: Send + Sync {
    fn get(&self, url: &str, accept: &str, limit: u64) -> Result<Response, TransportError>;
}

/// The real transport.
pub struct UreqTransport(ureq::Agent);

impl UreqTransport {
    pub fn new() -> UreqTransport {
        let tls = ureq::tls::TlsConfig::builder()
            .root_certs(ureq::tls::RootCerts::PlatformVerifier)
            .build();
        let config = ureq::Agent::config_builder()
            .user_agent(user_agent())
            .http_status_as_error(false)
            .timeout_connect(Some(Duration::from_secs(10)))
            .timeout_global(Some(Duration::from_secs(30)))
            .tls_config(tls)
            .build();
        UreqTransport(config.into())
    }
}

impl Transport for UreqTransport {
    fn get(&self, url: &str, accept: &str, limit: u64) -> Result<Response, TransportError> {
        let mut response = self
            .0
            .get(url)
            .header("Accept", accept)
            .call()
            .map_err(|error| match error {
                ureq::Error::Io(_)
                | ureq::Error::Timeout(_)
                | ureq::Error::HostNotFound
                | ureq::Error::ConnectionFailed => TransportError::Unreachable(error.to_string()),
                other => TransportError::Failed(other.to_string()),
            })?;
        let header = |name: &str| {
            response
                .headers()
                .get(name)
                .and_then(|value| value.to_str().ok())
                .map(str::to_owned)
        };
        let content_type = header("content-type");
        let retry_after = header("retry-after")
            .and_then(|value| value.trim().parse().ok())
            .map(Duration::from_secs);
        let status = response.status().as_u16();
        let body = response
            .body_mut()
            .with_config()
            .limit(limit)
            .read_to_vec()
            .map_err(|error| match error {
                ureq::Error::Io(_) | ureq::Error::Timeout(_) => {
                    TransportError::Unreachable(error.to_string())
                }
                other => TransportError::Failed(other.to_string()),
            })?;
        Ok(Response {
            status,
            content_type,
            retry_after,
            body,
        })
    }
}

/// Time as the client sees it, so tests can run the limiter and backoff
/// without waiting.
pub trait Clock: Send + Sync {
    fn now(&self) -> Instant;
    fn sleep(&self, duration: Duration);
}

pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> Instant {
        Instant::now()
    }

    fn sleep(&self, duration: Duration) {
        std::thread::sleep(duration);
    }
}

#[derive(Default)]
struct HostState {
    /// When the next request may be sent.
    next_request: Option<Instant>,
    /// Set while the host is considered unreachable: until when, and the
    /// wait that led to it.
    unreachable: Option<(Instant, Duration)>,
}

pub struct Client {
    transport: Box<dyn Transport>,
    clock: Box<dyn Clock>,
    hosts: Mutex<HashMap<String, HostState>>,
}

impl Client {
    pub fn new(transport: Box<dyn Transport>, clock: Box<dyn Clock>) -> Client {
        Client {
            transport,
            clock,
            hosts: Mutex::new(HashMap::new()),
        }
    }

    fn hosts(&self) -> std::sync::MutexGuard<'_, HashMap<String, HostState>> {
        self.hosts.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Fetches `url` with a 2xx status, waiting for its host's rate limit.
    /// Fails at once with `Error::Offline` while the host is unreachable.
    pub fn get(&self, url: &str, accept: &str, limit: u64) -> Result<Response, Error> {
        let host = host_of(url).ok_or_else(|| Error::Invalid(format!("Not a URL: {url}")))?;
        for attempt in 1..=BUSY_TRIES {
            self.wait_turn(host)?;
            let response = match self.transport.get(url, accept, limit) {
                Ok(response) => response,
                Err(TransportError::Unreachable(message)) => {
                    self.mark_unreachable(host);
                    return Err(Error::Offline(format!("{host}: {message}")));
                }
                Err(TransportError::Failed(message)) => {
                    return Err(Error::Invalid(format!("{url}: {message}")));
                }
            };
            self.mark_reachable(host);
            match response.status {
                200..=299 => return Ok(response),
                429 | 503 if attempt < BUSY_TRIES => {
                    let wait = response.retry_after.unwrap_or(BUSY_WAIT * attempt);
                    if wait > BUSY_WAIT_MAX {
                        break;
                    }
                    self.clock.sleep(wait);
                }
                status => {
                    return Err(Error::Status {
                        url: url.into(),
                        status,
                    })
                }
            }
        }
        Err(Error::Status {
            url: url.into(),
            status: 503,
        })
    }

    /// The body of the JSON document at `url`, from the response cache when
    /// it's younger than `max_age`. When the host can't be reached, an older
    /// cached copy is better than nothing and is returned instead.
    pub fn get_json(
        &self,
        conn: &Connection,
        url: &str,
        max_age: Duration,
    ) -> Result<String, Error> {
        let now = cache::unix_now();
        let cached = cache::lookup(conn, url)?;
        if let Some((body, fetched_at)) = &cached {
            if now.saturating_sub(*fetched_at) < max_age.as_secs() as i64 {
                return Ok(body.clone());
            }
        }
        match self.get(url, "application/json", JSON_LIMIT) {
            Ok(response) => {
                let body = String::from_utf8(response.body)
                    .map_err(|_| Error::Invalid(format!("{url}: the response is not UTF-8")))?;
                cache::store(conn, url, &body, now)?;
                Ok(body)
            }
            Err(Error::Offline(message)) => {
                cached.map(|(body, _)| body).ok_or(Error::Offline(message))
            }
            Err(error) => Err(error),
        }
    }

    /// Waits until a request to `host` may be sent, and reserves that slot.
    fn wait_turn(&self, host: &str) -> Result<(), Error> {
        let now = self.clock.now();
        let slot = {
            let mut hosts = self.hosts();
            let state = hosts.entry(host.to_owned()).or_default();
            if let Some((until, _)) = state.unreachable {
                if now < until {
                    return Err(Error::Offline(format!(
                        "{host} couldn't be reached; retrying in {} s",
                        (until - now).as_secs().max(1)
                    )));
                }
            }
            let slot = state.next_request.map_or(now, |next| next.max(now));
            state.next_request = Some(slot + request_interval(host));
            slot
        };
        if slot > now {
            self.clock.sleep(slot - now);
        }
        Ok(())
    }

    fn mark_unreachable(&self, host: &str) {
        let now = self.clock.now();
        let mut hosts = self.hosts();
        let state = hosts.entry(host.to_owned()).or_default();
        let wait = state.unreachable.map_or(UNREACHABLE_FIRST, |(_, wait)| {
            (wait * 2).min(UNREACHABLE_MAX)
        });
        state.unreachable = Some((now + wait, wait));
    }

    fn mark_reachable(&self, host: &str) {
        if let Some(state) = self.hosts().get_mut(host) {
            state.unreachable = None;
        }
    }

    /// Forgets that any host was unreachable, e.g. when the user asks to
    /// try again.
    pub fn retry_now(&self) {
        for state in self.hosts().values_mut() {
            state.unreachable = None;
        }
    }

    /// The hosts currently considered unreachable.
    pub fn unreachable_hosts(&self) -> Vec<String> {
        let now = self.clock.now();
        let mut hosts: Vec<String> = self
            .hosts()
            .iter()
            .filter(|(_, state)| state.unreachable.is_some_and(|(until, _)| now < until))
            .map(|(host, _)| host.clone())
            .collect();
        hosts.sort();
        hosts
    }
}

/// The host of an http(s) URL, e.g. "musicbrainz.org".
fn host_of(url: &str) -> Option<&str> {
    let rest = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))?;
    let end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
    let authority = &rest[..end];
    let host = authority
        .rsplit_once('@')
        .map_or(authority, |(_, host)| host);
    let host = host.split(':').next().unwrap_or(host);
    (!host.is_empty()).then_some(host)
}

#[cfg(test)]
pub mod testing {
    //! A fake transport and clock for tests.

    use std::collections::VecDeque;
    use std::sync::{Arc, Mutex};
    use std::time::{Duration, Instant};

    use super::*;

    /// Answers requests from a script of responses per URL, in order, and
    /// records every request with the fake time it was sent at.
    #[derive(Clone, Default)]
    pub struct FakeTransport {
        script: Arc<Mutex<HashMap<String, VecDeque<Result<Response, TransportError>>>>>,
        pub requests: Arc<Mutex<Vec<(String, Duration)>>>,
        clock: Option<FakeClock>,
    }

    impl FakeTransport {
        pub fn new(clock: &FakeClock) -> FakeTransport {
            FakeTransport {
                clock: Some(clock.clone()),
                ..FakeTransport::default()
            }
        }

        pub fn push(&self, url: &str, answer: Result<Response, TransportError>) {
            self.script
                .lock()
                .unwrap()
                .entry(url.to_owned())
                .or_default()
                .push_back(answer);
        }

        pub fn push_status(&self, url: &str, status: u16, body: &str) {
            self.push(url, Ok(response(status, body)));
        }

        pub fn urls(&self) -> Vec<String> {
            self.requests
                .lock()
                .unwrap()
                .iter()
                .map(|(url, _)| url.clone())
                .collect()
        }
    }

    pub fn response(status: u16, body: &str) -> Response {
        Response {
            status,
            content_type: Some("application/json".into()),
            retry_after: None,
            body: body.as_bytes().to_vec(),
        }
    }

    impl Transport for FakeTransport {
        fn get(&self, url: &str, _accept: &str, _limit: u64) -> Result<Response, TransportError> {
            let elapsed = self
                .clock
                .as_ref()
                .map_or(Duration::ZERO, FakeClock::elapsed);
            self.requests
                .lock()
                .unwrap()
                .push((url.to_owned(), elapsed));
            self.script
                .lock()
                .unwrap()
                .get_mut(url)
                .and_then(VecDeque::pop_front)
                .unwrap_or_else(|| Ok(response(404, "")))
        }
    }

    /// A clock whose sleeps move time on at once.
    #[derive(Clone)]
    pub struct FakeClock {
        start: Instant,
        elapsed: Arc<Mutex<Duration>>,
    }

    impl FakeClock {
        pub fn new() -> FakeClock {
            FakeClock {
                start: Instant::now(),
                elapsed: Arc::default(),
            }
        }

        pub fn elapsed(&self) -> Duration {
            *self.elapsed.lock().unwrap()
        }

        pub fn advance(&self, duration: Duration) {
            *self.elapsed.lock().unwrap() += duration;
        }
    }

    impl Clock for FakeClock {
        fn now(&self) -> Instant {
            self.start + self.elapsed()
        }

        fn sleep(&self, duration: Duration) {
            self.advance(duration);
        }
    }

    /// A client over a fresh fake transport and clock.
    pub fn fake_client() -> (Client, FakeTransport, FakeClock) {
        let clock = FakeClock::new();
        let transport = FakeTransport::new(&clock);
        let client = Client::new(Box::new(transport.clone()), Box::new(clock.clone()));
        (client, transport, clock)
    }
}

#[cfg(test)]
mod tests {
    use super::testing::*;
    use super::*;
    use crate::library::db;

    const MB: &str = "https://musicbrainz.org/ws/2/release/1";
    const CAA: &str = "https://coverartarchive.org/release/1";

    fn secs(seconds: f64) -> Duration {
        Duration::from_secs_f64(seconds)
    }

    #[test]
    fn parses_hosts() {
        assert_eq!(
            host_of("https://musicbrainz.org/ws/2?x=1"),
            Some("musicbrainz.org")
        );
        assert_eq!(host_of("http://user@host:8080#frag"), Some("host"));
        assert_eq!(host_of("https://host?q"), Some("host"));
        for bad in ["ftp://host", "https://", "musicbrainz.org/ws"] {
            assert_eq!(host_of(bad), None, "{bad}");
        }
    }

    #[test]
    fn user_agent_names_the_app_version_and_a_contact() {
        let agent = user_agent();
        assert!(
            agent.starts_with(&format!("ano-mp/{} ( http", anomp::version())),
            "{agent}"
        );
    }

    #[test]
    fn limits_each_host_to_its_rate() {
        let (client, transport, _clock) = fake_client();
        for url in [MB, CAA, MB, MB, CAA] {
            let _ = client.get(url, "application/json", JSON_LIMIT);
        }
        let times = transport.requests.lock().unwrap().clone();
        let at = |url: &str| -> Vec<Duration> {
            times
                .iter()
                .filter(|(u, _)| u == url)
                .map(|(_, t)| *t)
                .collect()
        };
        // MusicBrainz a second apart; the other host isn't held up by it
        // beyond the wait the shared thread spends.
        assert_eq!(at(MB), [secs(0.0), secs(1.0), secs(2.0)]);
        assert_eq!(at(CAA), [secs(0.0), secs(2.0)]);
    }

    #[test]
    fn retries_when_busy() {
        let (client, transport, clock) = fake_client();
        transport.push_status(MB, 503, "");
        transport.push(
            MB,
            Ok(Response {
                retry_after: Some(Duration::from_secs(5)),
                ..response(429, "")
            }),
        );
        transport.push_status(MB, 200, "{}");
        let response = client.get(MB, "application/json", JSON_LIMIT).unwrap();
        assert_eq!(response.body, b"{}");
        // 2 s after the first 503, then the 5 s the 429 asked for.
        assert_eq!(clock.elapsed(), secs(7.0));
        assert_eq!(transport.urls().len(), 3);

        for _ in 0..BUSY_TRIES {
            transport.push_status(MB, 503, "");
        }
        let error = client.get(MB, "application/json", JSON_LIMIT).unwrap_err();
        assert!(
            matches!(error, Error::Status { status: 503, .. }),
            "{error}"
        );

        transport.push_status(MB, 404, "");
        let error = client.get(MB, "application/json", JSON_LIMIT).unwrap_err();
        assert!(
            matches!(error, Error::Status { status: 404, .. }),
            "{error}"
        );
    }

    #[test]
    fn a_long_retry_after_fails_instead_of_waiting() {
        let (client, transport, clock) = fake_client();
        transport.push(
            MB,
            Ok(Response {
                retry_after: Some(Duration::from_secs(3600)),
                ..response(503, "")
            }),
        );
        assert!(client.get(MB, "application/json", JSON_LIMIT).is_err());
        assert_eq!(transport.urls().len(), 1);
        assert!(clock.elapsed() < secs(1.0));
    }

    #[test]
    fn backs_off_from_unreachable_hosts() {
        let (client, transport, clock) = fake_client();
        let offline = || Err(TransportError::Unreachable("no route".into()));
        transport.push(MB, offline());
        let error = client.get(MB, "application/json", JSON_LIMIT).unwrap_err();
        assert!(matches!(error, Error::Offline(_)), "{error}");
        assert_eq!(client.unreachable_hosts(), ["musicbrainz.org"]);

        // Refused without a request until the backoff ends; other hosts
        // are still tried.
        let error = client.get(MB, "application/json", JSON_LIMIT).unwrap_err();
        assert!(error.to_string().contains("retrying in"), "{error}");
        assert_eq!(transport.urls(), [MB]);
        let _ = client.get(CAA, "application/json", JSON_LIMIT);
        assert_eq!(transport.urls().len(), 2);

        // A second failure doubles the wait.
        clock.advance(UNREACHABLE_FIRST);
        transport.push(MB, offline());
        assert!(client.get(MB, "application/json", JSON_LIMIT).is_err());
        clock.advance(UNREACHABLE_FIRST);
        assert_eq!(client.unreachable_hosts(), ["musicbrainz.org"]);
        clock.advance(UNREACHABLE_FIRST);
        assert!(client.unreachable_hosts().is_empty());

        // A success clears it; so does asking to retry.
        transport.push_status(MB, 200, "{}");
        client.get(MB, "application/json", JSON_LIMIT).unwrap();
        transport.push(MB, offline());
        assert!(client.get(MB, "application/json", JSON_LIMIT).is_err());
        client.retry_now();
        transport.push_status(MB, 200, "{}");
        client.get(MB, "application/json", JSON_LIMIT).unwrap();
    }

    #[test]
    fn caches_json_and_falls_back_to_stale_copies_offline() {
        let conn = db::open_in_memory().unwrap();
        let (client, transport, _clock) = fake_client();
        let day = Duration::from_secs(86400);
        transport.push_status(MB, 200, r#"{"v":1}"#);
        assert_eq!(client.get_json(&conn, MB, day).unwrap(), r#"{"v":1}"#);
        assert_eq!(client.get_json(&conn, MB, day).unwrap(), r#"{"v":1}"#);
        assert_eq!(transport.urls().len(), 1, "the second came from the cache");

        // Expired: fetched again.
        transport.push_status(MB, 200, r#"{"v":2}"#);
        assert_eq!(
            client.get_json(&conn, MB, Duration::ZERO).unwrap(),
            r#"{"v":2}"#
        );
        // Expired and offline: the stale copy.
        transport.push(MB, Err(TransportError::Unreachable("down".into())));
        assert_eq!(
            client.get_json(&conn, MB, Duration::ZERO).unwrap(),
            r#"{"v":2}"#
        );
        // Errors aren't cached, and nothing cached means the error.
        transport.push_status(CAA, 500, "oops");
        assert!(client.get_json(&conn, CAA, day).is_err());
        assert!(cache::lookup(&conn, CAA).unwrap().is_none());
    }
}
