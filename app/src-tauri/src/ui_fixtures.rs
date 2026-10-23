//! Payloads for the frontend's state tests (PLAN.md H13), recorded from
//! the Rust side so the fake backend there answers as this one does:
//! samples built the way the commands and events build them, as serde
//! sends them, one JSON file per area in `app/tests/state/fixtures/`.
//!
//! Each module that builds a payload has a `ui_fixtures` test calling
//! `record`. Like the bindings, it fails while its file is out of date;
//! `ANOMP_WRITE_BINDINGS=1 cargo test ui_fixtures` rewrites them. Only the
//! payloads the state tests use are recorded.

use std::path::{Path, PathBuf};

use serde::Serialize;
use serde_json::{Map, Value};

/// The fixture files, each written by one module's `ui_fixtures` test.
pub const FILES: &[&str] = &["library", "queue", "servers", "settings"];

fn dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../tests/state/fixtures")
}

/// `value` as the frontend receives it.
pub fn sample(value: impl Serialize) -> Value {
    serde_json::to_value(value).expect("a payload serializes")
}

/// Checks fixture `name` (with `ANOMP_WRITE_BINDINGS` set, writes it):
/// its samples, by name.
pub fn record(name: &str, samples: &[(&str, Value)]) {
    assert!(FILES.contains(&name), "{name} isn't in ui_fixtures::FILES");
    let samples: Map<String, Value> = samples
        .iter()
        .map(|(key, value)| ((*key).to_owned(), value.clone()))
        .collect();
    let text = serde_json::to_string_pretty(&samples).unwrap() + "\n";
    let path = dir().join(format!("{name}.json"));
    if std::env::var_os("ANOMP_WRITE_BINDINGS").is_some() {
        std::fs::create_dir_all(dir()).unwrap();
        std::fs::write(&path, text).unwrap();
        return;
    }
    let committed = std::fs::read_to_string(&path).unwrap_or_default();
    assert!(
        committed == text,
        "{} is out of date: run `ANOMP_WRITE_BINDINGS=1 cargo test ui_fixtures` in src-tauri",
        path.display()
    );
}

/// A file no test writes any more is stale too (a missing one fails its
/// `record`).
#[test]
fn ui_fixtures_are_all_written() {
    for entry in std::fs::read_dir(dir()).unwrap() {
        let path = entry.unwrap().path();
        let name = path.file_stem().unwrap().to_string_lossy();
        assert!(
            path.extension().is_some_and(|ext| ext == "json") && FILES.contains(&name.as_ref()),
            "no ui_fixtures test writes {}: delete it",
            path.display()
        );
    }
}
