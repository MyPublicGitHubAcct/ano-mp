//! Checking for a newer release (PLAN.md §8.2). Builds are distributed from
//! the repository's GitHub Releases page (§8.1), so the check reads its
//! latest release through `http::Client` and compares versions; the user
//! downloads the new DMG from the release's page themselves. The Tauri
//! updater plugin isn't used: its macOS install replaces the app in place,
//! which the App Sandbox forbids.
//!
//! Checks are automatic only while `FeatureSettings::update_check` is on
//! (off by default: it goes online), shortly after launch and then daily;
//! a newer version found that way is announced once in `update-available`.
//! The user can always check by hand (`updates_check`), since they ask
//! for it.

use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use serde::Serialize;
use serde_json::Value;
use tauri::{AppHandle, Emitter, Manager, Runtime};

use crate::library::unix_now;
use crate::metadata::http::{Client, SystemClock, UreqTransport, JSON_LIMIT};
use crate::metadata::Error;
use crate::settings;

/// The repository's latest release that isn't a draft or a pre-release.
const LATEST_URL: &str = "https://api.github.com/repos/MyPublicGitHubAcct/ano-mp/releases/latest";

/// Where the repository's release pages are; a release's link must start
/// with it.
const RELEASES_URL: &str = "https://github.com/MyPublicGitHubAcct/ano-mp/releases/";

/// The first automatic check waits this long after launch, out of its way.
const FIRST_CHECK_AFTER: Duration = Duration::from_secs(30);

/// Automatic checks after one that worked.
const CHECK_EVERY: Duration = Duration::from_secs(24 * 60 * 60);

/// The next automatic check after one that failed (offline, say).
const RETRY_AFTER: Duration = Duration::from_secs(60 * 60);

/// Frontend event with an `UpdateCheck` payload: an automatic check found a
/// newer version, sent once per version while the app runs.
pub const UPDATE_AVAILABLE_EVENT: &str = "update-available";

/// What a check found.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct UpdateCheck {
    /// This build's version.
    pub current: String,
    /// The latest release's version; null while there is none.
    pub latest: Option<String>,
    /// Its page on GitHub, to download from.
    pub url: Option<String>,
    /// Whether `latest` is newer than `current`.
    pub newer: bool,
    /// When it was checked, in Unix seconds.
    pub checked_at: i64,
}

/// The latest release, from GitHub's answer: its version and page. `None`
/// for a 404, which is what a repository without releases answers.
fn latest(client: &Client) -> Result<Option<(semver::Version, String)>, Error> {
    let response = match client.get(LATEST_URL, "application/vnd.github+json", JSON_LIMIT) {
        Ok(response) => response,
        Err(Error::Status { status: 404, .. }) => return Ok(None),
        Err(error) => return Err(error),
    };
    let body: Value = serde_json::from_slice(&response.body)
        .map_err(|_| Error::Invalid("GitHub's answer about releases wasn't JSON".into()))?;
    let tag = body["tag_name"].as_str().unwrap_or_default();
    let version = semver::Version::parse(tag.strip_prefix('v').unwrap_or(tag))
        .map_err(|_| Error::Invalid(format!("The latest release's tag isn't a version: {tag}")))?;
    // Only a page of this repository's; anything else gets the latest one.
    let url = body["html_url"]
        .as_str()
        .filter(|url| url.starts_with(RELEASES_URL))
        .map_or_else(|| format!("{RELEASES_URL}latest"), str::to_owned);
    Ok(Some((version, url)))
}

/// Checks with `client` whether a release is newer than `current`.
fn check(client: &Client, current: &str) -> Result<UpdateCheck, Error> {
    let this = semver::Version::parse(current)
        .map_err(|_| Error::Invalid(format!("This build's version isn't one: {current}")))?;
    let found = latest(client)?;
    log::info!(
        "latest release {}",
        found
            .as_ref()
            .map_or_else(|| "none".into(), |(version, _)| version.to_string())
    );
    Ok(UpdateCheck {
        current: current.to_owned(),
        newer: found.as_ref().is_some_and(|(version, _)| *version > this),
        latest: found.as_ref().map(|(version, _)| version.to_string()),
        url: found.map(|(_, url)| url),
        checked_at: unix_now(),
    })
}

/// When the next automatic check is due, given when the app started and
/// the last check's time and outcome (`true` if it worked).
fn next_due(started: Instant, last: Option<(Instant, bool)>) -> Instant {
    match last {
        None => started + FIRST_CHECK_AFTER,
        Some((at, true)) => at + CHECK_EVERY,
        Some((at, false)) => at + RETRY_AFTER,
    }
}

#[derive(Default)]
struct Inner {
    stop: bool,
    /// The last check's time and whether it worked.
    last_at: Option<(Instant, bool)>,
    /// The last check that worked.
    last: Option<UpdateCheck>,
    /// The newest version announced in `update-available`.
    announced: Option<String>,
}

#[derive(Default)]
struct Shared {
    inner: Mutex<Inner>,
    changed: Condvar,
}

impl Shared {
    fn lock(&self) -> MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Records a check's outcome; returns the check if it's a newer version
    /// not yet announced.
    fn record(&self, result: &Result<UpdateCheck, Error>) -> Option<UpdateCheck> {
        let mut inner = self.lock();
        inner.last_at = Some((Instant::now(), result.is_ok()));
        let check = result.as_ref().ok()?;
        inner.last = Some(check.clone());
        if !check.newer || inner.announced == check.latest {
            return None;
        }
        inner.announced = check.latest.clone();
        Some(check.clone())
    }
}

/// The checker, managed by Tauri.
pub struct Updates {
    shared: Arc<Shared>,
    client: Arc<Client>,
}

/// Starts the thread that checks automatically while the switch is on.
pub fn init<R: Runtime>(app: &AppHandle<R>) -> Result<(), String> {
    let shared = Arc::new(Shared::default());
    let client = Arc::new(Client::new(
        Box::new(UreqTransport::new()),
        Box::new(SystemClock),
    ));
    let (thread_app, thread_shared, thread_client) = (app.clone(), shared.clone(), client.clone());
    std::thread::Builder::new()
        .name("updates".into())
        .spawn(move || run(&thread_app, &thread_shared, &thread_client))
        .map_err(|e| format!("Cannot start the update checks: {e}"))?;
    app.manage(Updates { shared, client });
    Ok(())
}

fn run<R: Runtime>(app: &AppHandle<R>, shared: &Shared, client: &Client) {
    let started = Instant::now();
    let mut inner = shared.lock();
    loop {
        if inner.stop {
            return;
        }
        if !settings::current(app).features.update_check {
            inner = shared
                .changed
                .wait(inner)
                .unwrap_or_else(|e| e.into_inner());
            continue;
        }
        let due = next_due(started, inner.last_at);
        let now = Instant::now();
        if now < due {
            inner = shared
                .changed
                .wait_timeout(inner, due - now)
                .unwrap_or_else(|e| e.into_inner())
                .0;
            continue;
        }
        drop(inner);
        let result = check(client, env!("CARGO_PKG_VERSION"));
        if let Err(error) = &result {
            log::warn!("{error}");
        }
        if let Some(newer) = shared.record(&result) {
            let _ = app.emit(UPDATE_AVAILABLE_EVENT, &newer);
        }
        inner = shared.lock();
    }
}

/// The switch may have changed: the thread looks again.
pub fn configure<R: Runtime>(app: &AppHandle<R>) {
    if let Some(updates) = app.try_state::<Updates>() {
        updates.shared.changed.notify_all();
    }
}

/// Stops the thread, without waiting for it.
pub fn shutdown<R: Runtime>(app: &AppHandle<R>) {
    if let Some(updates) = app.try_state::<Updates>() {
        updates.shared.lock().stop = true;
        updates.shared.changed.notify_all();
    }
}

/// The last check that worked, automatic or not, since launch.
#[tauri::command]
pub fn updates_status<R: Runtime>(app: AppHandle<R>) -> Option<UpdateCheck> {
    app.try_state::<Updates>()
        .and_then(|updates| updates.shared.lock().last.clone())
}

/// Checks now, whether or not automatic checks are on: the user asked.
#[tauri::command]
pub async fn updates_check<R: Runtime>(app: AppHandle<R>) -> Result<UpdateCheck, String> {
    let updates = app
        .try_state::<Updates>()
        .ok_or("The update check isn't running")?;
    let (shared, client) = (updates.shared.clone(), updates.client.clone());
    tauri::async_runtime::spawn_blocking(move || {
        let result = check(&client, env!("CARGO_PKG_VERSION"));
        // Shown here, so not announced again.
        shared.record(&result);
        result.map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metadata::http::testing::fake_client;

    fn release(tag: &str, url: &str) -> String {
        serde_json::json!({ "tag_name": tag, "html_url": url, "draft": false, "prerelease": false })
            .to_string()
    }

    #[test]
    fn finds_a_newer_release() {
        let (client, transport, _) = fake_client();
        let page = format!("{RELEASES_URL}tag/v0.2.0");
        transport.push_status(LATEST_URL, 200, &release("v0.2.0", &page));
        let found = check(&client, "0.1.0").unwrap();
        assert!(found.newer);
        assert_eq!(found.current, "0.1.0");
        assert_eq!(found.latest.as_deref(), Some("0.2.0"));
        assert_eq!(found.url.as_deref(), Some(page.as_str()));

        // The same version, or an older one, isn't newer.
        for tag in ["v0.2.0", "0.1.9", "v0.2.0-beta.1"] {
            transport.push_status(LATEST_URL, 200, &release(tag, &page));
            assert!(!check(&client, "0.2.0").unwrap().newer, "{tag}");
        }
    }

    #[test]
    fn links_only_to_this_repositorys_releases() {
        let (client, transport, _) = fake_client();
        transport.push_status(LATEST_URL, 200, &release("v1.0.0", "https://example.com/x"));
        assert_eq!(
            check(&client, "0.1.0").unwrap().url,
            Some(format!("{RELEASES_URL}latest"))
        );
    }

    #[test]
    fn no_release_yet_is_not_an_error() {
        let (client, transport, _) = fake_client();
        transport.push_status(LATEST_URL, 404, r#"{"message": "Not Found"}"#);
        let found = check(&client, "0.1.0").unwrap();
        assert!(!found.newer);
        assert_eq!((found.latest, found.url), (None, None));
    }

    #[test]
    fn a_tag_that_isnt_a_version_fails() {
        let (client, transport, _) = fake_client();
        transport.push_status(LATEST_URL, 200, &release("nightly", RELEASES_URL));
        assert!(check(&client, "0.1.0").is_err());
        transport.push_status(LATEST_URL, 500, "");
        assert!(check(&client, "0.1.0").is_err());
    }

    #[test]
    fn checks_after_launch_then_daily_and_retries_failures_sooner() {
        let started = Instant::now();
        assert_eq!(next_due(started, None), started + FIRST_CHECK_AFTER);
        let at = started + Duration::from_secs(100);
        assert_eq!(next_due(started, Some((at, true))), at + CHECK_EVERY);
        assert_eq!(next_due(started, Some((at, false))), at + RETRY_AFTER);
    }

    #[test]
    fn announces_each_newer_version_once() {
        let shared = Shared::default();
        let found = |latest: &str, newer| UpdateCheck {
            current: "0.1.0".into(),
            latest: Some(latest.into()),
            url: None,
            newer,
            checked_at: 0,
        };
        assert!(shared.record(&Ok(found("0.1.0", false))).is_none());
        assert!(shared.record(&Ok(found("0.2.0", true))).is_some());
        assert!(shared.record(&Ok(found("0.2.0", true))).is_none());
        assert!(shared.record(&Err(Error::Offline("x".into()))).is_none());
        // A failure keeps the last check that worked.
        assert_eq!(shared.lock().last, Some(found("0.2.0", true)));
        assert!(shared.record(&Ok(found("0.3.0", true))).is_some());
    }
}
