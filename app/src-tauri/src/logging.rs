//! Logs and panics (PLAN.md H9).
//!
//! Everything logs through the `log` facade, which tauri-plugin-log writes
//! to `ano-mp.log` in the app's log directory (on macOS
//! `~/Library/Logs/<identifier>`, inside the container when sandboxed),
//! starting a new file past `MAX_FILE_SIZE` and keeping `KEEP` old ones.
//! Release builds write info and above; debug builds also debug, and copy
//! everything to stderr. Targets are the module paths ("library::scanner"),
//! "core" for the C++ core (`anomp::forward_core_log`: JUCE's Logger and
//! failed assertions too), and "webview" for the page's uncaught errors.
//!
//! What may be written (`format`):
//! - Every record is redacted: keys the app holds (`keep_secret`, which
//!   `metadata::keys` calls for each), `Authorization` headers, and
//!   `token=`-like query parameters.
//! - At info and above, absolute paths become `<path>` and URLs keep only
//!   their scheme and host (their queries and paths name albums and
//!   artists). Write ids and counts at those levels; library paths, file
//!   names, titles and artists go at debug, which release builds don't
//!   write.
//!
//! A panic hook writes the message, where it happened and a backtrace
//! before the default hook runs and the release build aborts
//! (`panic = "abort"`).

use std::fmt::Arguments;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use log::{Level, LevelFilter, Record};
use tauri::plugin::TauriPlugin;
use tauri::{AppHandle, Manager, Runtime};
use tauri_plugin_log::fern::FormatCallback;
use tauri_plugin_log::{RotationStrategy, Target, TargetKind};

/// The log file's name, without ".log".
pub const FILE_NAME: &str = "ano-mp";
/// A file past this many bytes is put aside and a new one started.
const MAX_FILE_SIZE: u128 = 2 * 1024 * 1024;
/// Old files kept, besides the current one.
const KEEP: usize = 2;

/// Query parameters whose values are never written.
const SECRET_PARAMETERS: [&str; 7] = [
    "token",
    "access_token",
    "api_key",
    "apikey",
    "key",
    "secret",
    "password",
];

/// Keys the app holds, never written whatever the level.
static SECRETS: Mutex<Vec<String>> = Mutex::new(Vec::new());

/// Never writes `secret` (a key or token the app holds) to the log.
pub fn keep_secret(secret: &str) {
    let secret = secret.trim();
    // Shorter strings would match ordinary text.
    if secret.len() < 8 {
        return;
    }
    let mut secrets = SECRETS.lock().unwrap_or_else(|e| e.into_inner());
    if !secrets.iter().any(|known| known == secret) {
        secrets.push(secret.to_owned());
    }
}

/// The log plugin, writing to the app's log directory.
pub fn plugin<R: Runtime>() -> TauriPlugin<R> {
    let mut builder = tauri_plugin_log::Builder::new()
        .clear_targets()
        .target(Target::new(TargetKind::LogDir {
            file_name: Some(FILE_NAME.into()),
        }))
        .max_file_size(MAX_FILE_SIZE)
        .rotation_strategy(RotationStrategy::KeepSome(KEEP))
        .level(if cfg!(debug_assertions) {
            LevelFilter::Debug
        } else {
            LevelFilter::Info
        })
        // Other crates' chatter: only their problems.
        .level_for("tao", LevelFilter::Warn)
        .level_for("wry", LevelFilter::Warn)
        .level_for("tauri", LevelFilter::Warn)
        .level_for("tauri_runtime_wry", LevelFilter::Warn)
        .level_for("notify", LevelFilter::Warn)
        .level_for("ureq", LevelFilter::Warn)
        .level_for("ureq_proto", LevelFilter::Warn)
        .level_for("rustls", LevelFilter::Warn)
        .format(format);
    if cfg!(debug_assertions) {
        builder = builder.target(Target::new(TargetKind::Stderr));
    }
    builder.build()
}

/// The log directory.
pub fn log_dir<R: Runtime>(app: &AppHandle<R>) -> Result<PathBuf, String> {
    app.path().app_log_dir().map_err(|e| e.to_string())
}

/// `2026-10-02T09:15:42.123Z WARN library::scanner: message`, redacted,
/// and scrubbed at info and above.
fn format(out: FormatCallback, message: &Arguments, record: &Record) {
    let text = line(record.level(), record.target(), &message.to_string());
    out.finish(format_args!("{} {text}", timestamp(SystemTime::now())));
}

/// A record as written, without its time.
pub fn line(level: Level, target: &str, message: &str) -> String {
    let target = match target {
        "ano_mp_lib" => "app",
        target => target.strip_prefix("ano_mp_lib::").unwrap_or(target),
    };
    let mut text = redact(message);
    if level <= Level::Info {
        text = scrub(&text);
    }
    // One record, one line.
    let text = text.replace('\n', "\n    ");
    format!("{level:<5} {target}: {text}")
}

/// `text` without keys the app holds, `Authorization` headers' values, or
/// the values of query parameters named like secrets.
pub fn redact(text: &str) -> String {
    let mut text = text.to_owned();
    for secret in SECRETS.lock().unwrap_or_else(|e| e.into_inner()).iter() {
        text = text.replace(secret.as_str(), "<redacted>");
    }
    let text = redact_after(&text, "authorization:", |c| c == '\n' || c == '"');
    let text = redact_after(&text, "authorization\": \"", |c| c == '"');
    let text = redact_after(&text, "authorization\":\"", |c| c == '"');
    let text = redact_after(&text, "authorization=", |c| c == '&' || c.is_whitespace());
    let mut text = text;
    for name in SECRET_PARAMETERS {
        text = redact_parameter(&text, name);
    }
    text
}

/// Replaces what follows each `marker` (case-insensitive), up to `ends`,
/// with `<redacted>`.
fn redact_after(text: &str, marker: &str, ends: impl Fn(char) -> bool) -> String {
    let lower = text.to_ascii_lowercase();
    let mut out = String::with_capacity(text.len());
    let mut rest = 0;
    let mut from = 0;
    while let Some(found) = lower[from..].find(marker) {
        let start = from + found + marker.len();
        let value_start = start + text[start..].len() - text[start..].trim_start().len();
        let end = text[value_start..]
            .find(&ends)
            .map_or(text.len(), |end| value_start + end);
        out.push_str(&text[rest..value_start]);
        if end > value_start {
            out.push_str("<redacted>");
        }
        rest = end;
        from = end.max(start);
    }
    out.push_str(&text[rest..]);
    out
}

/// Replaces the value of each `name=` query parameter (after `?`, `&` or
/// `;`, case-insensitive) with `<redacted>`.
fn redact_parameter(text: &str, name: &str) -> String {
    let lower = text.to_ascii_lowercase();
    let marker = format!("{name}=");
    let mut out = String::with_capacity(text.len());
    let mut rest = 0;
    let mut from = 0;
    while let Some(found) = lower[from..].find(&marker) {
        let at = from + found;
        let start = at + marker.len();
        let follows_separator =
            at > 0 && matches!(text.as_bytes()[at - 1], b'?' | b'&' | b';' | b' ');
        if !follows_separator {
            from = start;
            continue;
        }
        let end = text[start..]
            .find(|c: char| c == '&' || c == '#' || c == '"' || c == '\'' || c.is_whitespace())
            .map_or(text.len(), |end| start + end);
        out.push_str(&text[rest..start]);
        if end > start {
            out.push_str("<redacted>");
        }
        rest = end;
        from = end;
    }
    out.push_str(&text[rest..]);
    out
}

/// `text` with each absolute path as `<path>` and each URL cut to its
/// scheme and host.
pub fn scrub(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len());
    let mut i = 0;
    while i < chars.len() {
        let starts_token = i == 0 || is_boundary(chars[i - 1]);
        if starts_token {
            if let Some(end) = url_at(&chars, i) {
                out.push_str(&url_head(&chars[i..end].iter().collect::<String>()));
                i = end;
                continue;
            }
            if let Some(end) = path_at(&chars, i) {
                out.push_str("<path>");
                i = end;
                continue;
            }
        }
        out.push(chars[i]);
        i += 1;
    }
    out
}

fn is_boundary(c: char) -> bool {
    c.is_whitespace() || matches!(c, '"' | '\'' | '(' | '[' | '<' | '=' | ',' | '`')
}

/// Where a URL starting at `i` ends, if one does.
fn url_at(chars: &[char], i: usize) -> Option<usize> {
    let rest: String = chars[i..chars.len().min(i + 12)].iter().collect();
    let scheme = rest.find("://")?;
    if scheme == 0 || !rest[..scheme].chars().all(|c| c.is_ascii_alphabetic()) {
        return None;
    }
    let end = chars[i..]
        .iter()
        .position(|&c| c.is_whitespace() || matches!(c, '"' | '\'' | ')' | ']' | '>' | '`'))
        .map_or(chars.len(), |end| i + end);
    Some(end)
}

/// `https://host/…` for a URL with more than a host.
fn url_head(url: &str) -> String {
    let after_scheme = url.find("://").map_or(0, |at| at + 3);
    match url[after_scheme..].find(['/', '?', '#']) {
        Some(at) => format!("{}…", &url[..after_scheme + at + 1]),
        None => url.to_owned(),
    }
}

/// Where an absolute path starting at `i` ends, if one does: at the end, a
/// line break, a quote or bracket, `": "` or `" ("`. Spaces inside it are
/// allowed ("/Users/me/My Music/a b.flac").
fn path_at(chars: &[char], i: usize) -> Option<usize> {
    let home = chars[i] == '~' && chars.get(i + 1) == Some(&'/');
    if chars[i] != '/' && !home {
        return None;
    }
    let first = chars.get(i + if home { 2 } else { 1 })?;
    if !(first.is_alphanumeric() || matches!(first, '.' | '_' | '-')) {
        return None;
    }
    let mut end = i + 1;
    while end < chars.len() {
        let c = chars[end];
        let next = chars.get(end + 1).copied();
        let ends = matches!(c, '\n' | '\r' | '\t' | '"' | '\'' | ')' | ']' | '>' | '`')
            || (c == ':' && next.is_none_or(char::is_whitespace))
            || (c == ',' && next.is_none_or(char::is_whitespace))
            || (c == ' ' && matches!(next, Some('(') | Some('-') | None));
        if ends {
            break;
        }
        end += 1;
    }
    Some(end)
}

/// `time` in UTC as `2026-10-02T09:15:42.123Z`.
pub fn timestamp(time: SystemTime) -> String {
    let elapsed = time.duration_since(UNIX_EPOCH).unwrap_or_default();
    let seconds = elapsed.as_secs();
    let (days, of_day) = (seconds / 86_400, seconds % 86_400);
    // Howard Hinnant's days-from-civil, inverted.
    let z = days as i64 + 719_468;
    let era = z.div_euclid(146_097);
    let day_of_era = z.rem_euclid(146_097);
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let mp = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = year_of_era + era * 400 + i64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}.{:03}Z",
        of_day / 3600,
        of_day % 3600 / 60,
        of_day % 60,
        elapsed.subsec_millis()
    )
}

/// Writes a panic's message, where it happened and a backtrace to the log
/// before the default hook runs (and, in release builds, the process
/// aborts).
pub fn install_panic_hook() {
    let default = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let message = info
            .payload()
            .downcast_ref::<&str>()
            .map(|text| (*text).to_owned())
            .or_else(|| info.payload().downcast_ref::<String>().cloned())
            .unwrap_or_else(|| "(no message)".into());
        let location = info
            .location()
            .map(|at| format!("{}:{}", at.file(), at.line()))
            .unwrap_or_default();
        let thread = std::thread::current()
            .name()
            .unwrap_or("unnamed")
            .to_owned();
        let backtrace = std::backtrace::Backtrace::force_capture();
        log::error!(
            target: "panic",
            "thread '{thread}' panicked at {location}: {message}\n{backtrace}"
        );
        log::logger().flush();
        default(info);
    }));
}

/// The last `count` lines of the log at info and above, oldest first; only
/// those levels are scrubbed (`format`).
pub fn recent_lines<R: Runtime>(app: &AppHandle<R>, count: usize) -> Vec<String> {
    let Ok(dir) = log_dir(app) else {
        return Vec::new();
    };
    let Ok(text) = std::fs::read_to_string(dir.join(format!("{FILE_NAME}.log"))) else {
        return Vec::new();
    };
    last_lines(&text, count)
}

fn last_lines(text: &str, count: usize) -> Vec<String> {
    let mut records: Vec<String> = Vec::new();
    for line in text.lines() {
        // Continuation lines (a backtrace) belong to the record before.
        if line.starts_with("    ") {
            if let Some(last) = records.last_mut() {
                last.push('\n');
                last.push_str(line);
            }
            continue;
        }
        records.push(line.to_owned());
    }
    let kept: Vec<String> = records
        .into_iter()
        .filter(|record| {
            let level = record.split(' ').nth(1).unwrap_or("");
            matches!(level, "ERROR" | "WARN" | "INFO")
        })
        .collect();
    kept[kept.len().saturating_sub(count)..].to_vec()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redacts_keys_headers_and_secret_parameters() {
        keep_secret("s3cr3t-discogs-token");
        keep_secret("short");
        assert_eq!(
            redact("Discogs said: invalid token s3cr3t-discogs-token"),
            "Discogs said: invalid token <redacted>"
        );
        assert_eq!(redact("short words stay"), "short words stay");
        assert_eq!(
            redact("request failed: Authorization: Discogs token=abc123\nnext line"),
            "request failed: Authorization: <redacted>\nnext line"
        );
        assert_eq!(
            redact("GET https://api.example.com/x?q=a&Token=abc&api_key=def#frag failed"),
            "GET https://api.example.com/x?q=a&Token=<redacted>&api_key=<redacted>#frag failed"
        );
        // Only parameters, not words that end the same way.
        assert_eq!(redact("monkey=1 donkey=2"), "monkey=1 donkey=2");
        assert_eq!(
            redact("{\"authorization\": \"Token xyz\"}"),
            "{\"authorization\": \"<redacted>\"}"
        );
    }

    #[test]
    fn scrubs_paths_and_urls() {
        assert_eq!(
            scrub("Cannot open /Users/me/My Music/Björk/01 Joga.flac: No such file (os error 2)"),
            "Cannot open <path>: No such file (os error 2)"
        );
        assert_eq!(
            scrub(
                "Folder not available (missing): /Volumes/NAS/Music (The file couldn’t be opened)"
            ),
            "Folder not available (missing): <path> (The file couldn’t be opened)"
        );
        assert_eq!(scrub("moved to \"~/Music/Old\""), "moved to \"<path>\"");
        assert_eq!(
            scrub("GET https://musicbrainz.org/ws/2/release?query=artist:Björk failed: 503"),
            "GET https://musicbrainz.org/… failed: 503"
        );
        assert_eq!(
            scrub("https://coverartarchive.org"),
            "https://coverartarchive.org"
        );
        // Not paths: ratios, units, ids.
        assert_eq!(
            scrub("folder 3: 12/40 tracks, 48 kHz, 1/2 done"),
            "folder 3: 12/40 tracks, 48 kHz, 1/2 done"
        );
        assert_eq!(scrub("a / b"), "a / b");
    }

    #[test]
    fn only_info_and_above_are_scrubbed() {
        let path = "reading /Users/me/Music/a.flac";
        assert_eq!(
            line(Level::Warn, "ano_mp_lib::library::scanner", path),
            "WARN  library::scanner: reading <path>"
        );
        assert_eq!(
            line(Level::Debug, "ano_mp_lib::library::scanner", path),
            "DEBUG library::scanner: reading /Users/me/Music/a.flac"
        );
        assert_eq!(
            line(Level::Error, "panic", "boom\n   0: frame"),
            "ERROR panic: boom\n       0: frame"
        );
        assert_eq!(
            line(Level::Info, "ano_mp_lib", "starting"),
            "INFO  app: starting"
        );
    }

    #[test]
    fn a_panic_is_logged_before_the_default_hook() {
        static CAPTURED: Mutex<Vec<String>> = Mutex::new(Vec::new());
        struct Capture;
        impl log::Log for Capture {
            fn enabled(&self, _: &log::Metadata) -> bool {
                true
            }
            fn log(&self, record: &Record) {
                if record.target() == "panic" {
                    CAPTURED
                        .lock()
                        .unwrap_or_else(|e| e.into_inner())
                        .push(record.args().to_string());
                }
            }
            fn flush(&self) {}
        }
        // The only test that sets a logger, so the only one to succeed.
        let _ = log::set_logger(&Capture);
        log::set_max_level(LevelFilter::Debug);
        install_panic_hook();
        let result = std::thread::Builder::new()
            .name("doomed".into())
            .spawn(|| panic!("on purpose"))
            .unwrap()
            .join();
        assert!(result.is_err());
        let captured = CAPTURED.lock().unwrap_or_else(|e| e.into_inner()).clone();
        assert!(
            captured
                .iter()
                .any(|text| text.starts_with("thread 'doomed' panicked at")
                    && text.contains("logging.rs")
                    && text.contains("on purpose")),
            "{captured:?}"
        );
    }

    #[test]
    fn timestamps_are_utc() {
        let at = UNIX_EPOCH + std::time::Duration::from_millis(1_790_932_542_123);
        assert_eq!(timestamp(at), "2026-10-02T09:15:42.123Z");
        assert_eq!(timestamp(UNIX_EPOCH), "1970-01-01T00:00:00.000Z");
        let leap = UNIX_EPOCH + std::time::Duration::from_secs(951_782_400);
        assert_eq!(timestamp(leap), "2000-02-29T00:00:00.000Z");
    }

    #[test]
    fn recent_lines_leave_out_debug_and_keep_backtraces() {
        let text = "\
t INFO  a: one
t DEBUG a: /Users/me/secret.flac
t ERROR panic: boom
    0: frame
t WARN  b: three
";
        assert_eq!(
            last_lines(text, 2),
            ["t ERROR panic: boom\n    0: frame", "t WARN  b: three"]
        );
        assert_eq!(last_lines(text, 10).len(), 3);
    }

    #[test]
    fn the_core_logs_through_the_facade() {
        // The facade has no logger in tests; this checks the callback runs
        // and returns without one.
        crate::anomp::forward_core_log();
        crate::anomp::core_log_write(Level::Warn, "from the core");
        crate::anomp::stop_core_log();
    }
}
