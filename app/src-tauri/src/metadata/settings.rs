//! Which metadata sources are used, and in what order. Each kind of data
//! (album details, album art) has an ordered list of the sources that can
//! supply it, and the first with a result wins. Stored as one JSON value
//! under `metadata.services` in `settings`, read like `library.sort`:
//! whatever is usable is kept and the rest falls back to the defaults.

use std::collections::{BTreeMap, HashSet};

use rusqlite::{Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::Error;

const SETTINGS_KEY: &str = "metadata.services";

/// A kind of data that sources supply.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Kind {
    /// An album's match at a service, and the details that come with it.
    Release,
    AlbumArt,
}

impl Kind {
    pub const ALL: [Kind; 2] = [Kind::Release, Kind::AlbumArt];
}

/// A metadata source. The serialized id is also what the `source` columns
/// of migration 003 store, so it must never change.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SourceId {
    /// The picture embedded in the audio files.
    Embedded,
    /// An image file next to the tracks, e.g. `cover.jpg`.
    Folder,
    #[serde(rename = "musicbrainz")]
    MusicBrainz,
    /// Album art for releases matched on MusicBrainz.
    CoverArtArchive,
}

/// What the UI shows about a source.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceInfo {
    pub id: SourceId,
    pub name: &'static str,
    pub kinds: &'static [Kind],
    /// Contacts a service over the network, so it obeys the online switch.
    pub online: bool,
    /// Needs an API key or token from the user before it can be used.
    pub needs_key: bool,
    /// Another source it relies on (the Cover Art Archive needs a
    /// MusicBrainz match).
    pub requires: Option<SourceId>,
    pub homepage: Option<&'static str>,
}

impl SourceId {
    /// Every source, in the order the UI lists them.
    pub const ALL: [SourceId; 4] = [
        SourceId::Embedded,
        SourceId::Folder,
        SourceId::MusicBrainz,
        SourceId::CoverArtArchive,
    ];

    pub fn info(self) -> SourceInfo {
        let (name, kinds, online, requires, homepage): (_, &'static [Kind], _, _, _) = match self {
            SourceId::Embedded => ("Embedded in files", &[Kind::AlbumArt], false, None, None),
            SourceId::Folder => (
                "Images in album folders",
                &[Kind::AlbumArt],
                false,
                None,
                None,
            ),
            SourceId::MusicBrainz => (
                "MusicBrainz",
                &[Kind::Release],
                true,
                None,
                Some("https://musicbrainz.org"),
            ),
            SourceId::CoverArtArchive => (
                "Cover Art Archive",
                &[Kind::AlbumArt],
                true,
                Some(SourceId::MusicBrainz),
                Some("https://coverartarchive.org"),
            ),
        };
        SourceInfo {
            id: self,
            name,
            kinds,
            online,
            needs_key: false,
            requires,
            homepage,
        }
    }

    pub fn supplies(self, kind: Kind) -> bool {
        self.info().kinds.contains(&kind)
    }

    /// The id as stored in the database.
    pub fn as_str(self) -> &'static str {
        match self {
            SourceId::Embedded => "embedded",
            SourceId::Folder => "folder",
            SourceId::MusicBrainz => "musicbrainz",
            SourceId::CoverArtArchive => "cover-art-archive",
        }
    }

    pub fn from_str(id: &str) -> Option<SourceId> {
        SourceId::ALL
            .into_iter()
            .find(|source| source.as_str() == id)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceSettings {
    pub id: SourceId,
    pub enabled: bool,
    /// For sources that need one; trimmed, and `None` rather than empty.
    #[serde(default)]
    pub api_key: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceSettings {
    /// The switch for every online source; local ones are unaffected.
    pub online: bool,
    /// Look albums up in the background after each scan.
    pub auto_match: bool,
    /// One entry per source, in `SourceId::ALL` order.
    pub sources: Vec<SourceSettings>,
    /// For each kind, every source that supplies it, in priority order.
    pub order: BTreeMap<Kind, Vec<SourceId>>,
}

impl Default for ServiceSettings {
    fn default() -> Self {
        ServiceSettings {
            online: true,
            auto_match: true,
            sources: SourceId::ALL
                .into_iter()
                .map(|id| SourceSettings {
                    id,
                    enabled: true,
                    api_key: None,
                })
                .collect(),
            order: Kind::ALL
                .into_iter()
                .map(|kind| (kind, default_order(kind)))
                .collect(),
        }
    }
}

/// Local sources first: they are free, and the files' own art is usually
/// what the user put there on purpose.
fn default_order(kind: Kind) -> Vec<SourceId> {
    SourceId::ALL
        .into_iter()
        .filter(|source| source.supplies(kind))
        .collect()
}

impl ServiceSettings {
    fn source(&self, id: SourceId) -> Option<&SourceSettings> {
        self.sources.iter().find(|source| source.id == id)
    }

    /// Whether `id` may be used now: enabled, online sources only while the
    /// online switch is on, with a key if it needs one, and with the source
    /// it relies on usable too.
    pub fn is_usable(&self, id: SourceId) -> bool {
        let info = id.info();
        self.source(id)
            .is_some_and(|source| source.enabled && (!info.needs_key || source.api_key.is_some()))
            && (!info.online || self.online)
            && info
                .requires
                .is_none_or(|required| self.is_usable(required))
    }

    /// The usable sources for `kind`, in priority order.
    pub fn sources_for(&self, kind: Kind) -> Vec<SourceId> {
        self.order
            .get(&kind)
            .map(|order| {
                order
                    .iter()
                    .copied()
                    .filter(|&id| self.is_usable(id))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Settings as given by the user or read from storage, made complete:
    /// one entry per known source (missing ones get the defaults, repeats
    /// are dropped), and each kind's order listing exactly the sources that
    /// supply it (unknown or unsuitable ones dropped, missing ones added at
    /// the end in their default order).
    fn normalized(mut self) -> ServiceSettings {
        let mut sources = Vec::new();
        for id in SourceId::ALL {
            let mut source = self
                .sources
                .iter()
                .find(|source| source.id == id)
                .cloned()
                .unwrap_or(SourceSettings {
                    id,
                    enabled: true,
                    api_key: None,
                });
            source.api_key = source
                .api_key
                .map(|key| key.trim().to_owned())
                .filter(|key| !key.is_empty());
            sources.push(source);
        }
        self.sources = sources;
        let mut order = BTreeMap::new();
        for kind in Kind::ALL {
            let mut seen = HashSet::new();
            let mut sources: Vec<SourceId> = self
                .order
                .remove(&kind)
                .unwrap_or_default()
                .into_iter()
                .filter(|source| source.supplies(kind) && seen.insert(*source))
                .collect();
            sources.extend(default_order(kind).into_iter().filter(|s| seen.insert(*s)));
            order.insert(kind, sources);
        }
        self.order = order;
        self
    }

    fn validate(&self) -> Result<(), Error> {
        for source in &self.sources {
            if let Some(key) = &source.api_key {
                if key.len() > 256 || key.chars().any(|c| c.is_whitespace() || c.is_control()) {
                    return Err(Error::Invalid(format!(
                        "The key for {} is not valid",
                        source.id.info().name
                    )));
                }
            }
        }
        Ok(())
    }

    /// Reads stored settings, keeping what is usable: entries this version
    /// doesn't know (e.g. a source added by a newer one) are dropped, and
    /// anything missing or invalid falls back to the defaults.
    fn from_json(json: &str) -> ServiceSettings {
        let Ok(Value::Object(stored)) = serde_json::from_str(json) else {
            return ServiceSettings::default();
        };
        let defaults = ServiceSettings::default();
        let flag = |name: &str, default: bool| {
            stored.get(name).and_then(Value::as_bool).unwrap_or(default)
        };
        let sources = match stored.get("sources") {
            Some(Value::Array(sources)) => sources
                .iter()
                .filter_map(|source| serde_json::from_value(source.clone()).ok())
                .collect(),
            _ => Vec::new(),
        };
        let mut order = BTreeMap::new();
        if let Some(Value::Object(stored_order)) = stored.get("order") {
            for (kind, sources) in stored_order {
                let (Ok(kind), Value::Array(sources)) =
                    (serde_json::from_value(Value::String(kind.clone())), sources)
                else {
                    continue;
                };
                let sources = sources
                    .iter()
                    .filter_map(|source| serde_json::from_value(source.clone()).ok())
                    .collect();
                order.insert(kind, sources);
            }
        }
        let settings = ServiceSettings {
            online: flag("online", defaults.online),
            auto_match: flag("autoMatch", defaults.auto_match),
            sources,
            order,
        }
        .normalized();
        if settings.validate().is_ok() {
            settings
        } else {
            defaults
        }
    }
}

pub fn service_settings(conn: &Connection) -> Result<ServiceSettings, Error> {
    let stored: Option<String> = conn
        .query_row(
            "SELECT value FROM settings WHERE key = ?1",
            [SETTINGS_KEY],
            |row| row.get(0),
        )
        .optional()?;
    Ok(stored.map_or_else(ServiceSettings::default, |json| {
        ServiceSettings::from_json(&json)
    }))
}

/// Stores `settings`, completed as `normalized` describes, and returns what
/// was stored.
pub fn save_service_settings(
    conn: &Connection,
    settings: ServiceSettings,
) -> Result<ServiceSettings, Error> {
    let settings = settings.normalized();
    settings.validate()?;
    let json = serde_json::to_string(&settings).expect("service settings serialize to JSON");
    conn.execute(
        "INSERT INTO settings (key, value) VALUES (?1, ?2)
         ON CONFLICT (key) DO UPDATE SET value = excluded.value",
        [SETTINGS_KEY, &json],
    )?;
    Ok(settings)
}

pub fn reset_service_settings(conn: &Connection) -> Result<ServiceSettings, Error> {
    conn.execute("DELETE FROM settings WHERE key = ?1", [SETTINGS_KEY])?;
    Ok(ServiceSettings::default())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::library::db;

    fn store_json(conn: &Connection, json: &str) {
        conn.execute(
            "INSERT OR REPLACE INTO settings (key, value) VALUES (?1, ?2)",
            [SETTINGS_KEY, json],
        )
        .unwrap();
    }

    #[test]
    fn defaults_list_every_source_with_local_art_first() {
        let conn = db::open_in_memory().unwrap();
        let settings = service_settings(&conn).unwrap();
        assert_eq!(settings, ServiceSettings::default());
        assert_eq!(settings.sources.len(), SourceId::ALL.len());
        assert_eq!(
            settings.sources_for(Kind::AlbumArt),
            [
                SourceId::Embedded,
                SourceId::Folder,
                SourceId::CoverArtArchive
            ]
        );
        assert_eq!(settings.sources_for(Kind::Release), [SourceId::MusicBrainz]);
    }

    #[test]
    fn ids_match_their_serialized_form() {
        for id in SourceId::ALL {
            assert_eq!(
                serde_json::to_value(id).unwrap(),
                Value::String(id.as_str().into())
            );
            assert_eq!(SourceId::from_str(id.as_str()), Some(id));
        }
        assert_eq!(SourceId::from_str("lastfm"), None);
    }

    #[test]
    fn saves_order_and_switches() {
        let conn = db::open_in_memory().unwrap();
        let mut settings = ServiceSettings::default();
        settings.order.insert(
            Kind::AlbumArt,
            vec![SourceId::CoverArtArchive, SourceId::Folder],
        );
        settings.sources[0].enabled = false; // Embedded.
        let saved = save_service_settings(&conn, settings).unwrap();
        // Embedded is added back to the order, at the end, but is disabled.
        assert_eq!(
            saved.order[&Kind::AlbumArt],
            [
                SourceId::CoverArtArchive,
                SourceId::Folder,
                SourceId::Embedded
            ]
        );
        assert_eq!(
            saved.sources_for(Kind::AlbumArt),
            [SourceId::CoverArtArchive, SourceId::Folder]
        );
        assert_eq!(service_settings(&conn).unwrap(), saved);

        reset_service_settings(&conn).unwrap();
        assert_eq!(service_settings(&conn).unwrap(), ServiceSettings::default());
    }

    #[test]
    fn online_switch_and_dependencies_gate_sources() {
        let mut settings = ServiceSettings::default();
        settings.online = false;
        assert_eq!(
            settings.sources_for(Kind::AlbumArt),
            [SourceId::Embedded, SourceId::Folder]
        );
        assert!(settings.sources_for(Kind::Release).is_empty());

        // The Cover Art Archive needs a MusicBrainz match.
        let mut settings = ServiceSettings::default();
        settings.sources[2].enabled = false; // MusicBrainz.
        assert!(!settings.is_usable(SourceId::CoverArtArchive));
        assert!(settings.is_usable(SourceId::Folder));
    }

    #[test]
    fn keeps_what_is_usable_from_stored_settings() {
        let conn = db::open_in_memory().unwrap();
        store_json(
            &conn,
            r#"{
                "online": false,
                "autoMatch": "yes",
                "sources": [
                    {"id": "folder", "enabled": false},
                    {"id": "lastfm", "enabled": true, "apiKey": "abc"},
                    {"id": "folder", "enabled": true},
                    {"id": "musicbrainz", "enabled": true, "apiKey": "  "}
                ],
                "order": {
                    "albumArt": ["lastfm", "cover-art-archive", "musicbrainz", "folder", "folder"],
                    "lyrics": ["lrclib"],
                    "release": "musicbrainz"
                }
            }"#,
        );
        let settings = service_settings(&conn).unwrap();
        assert!(!settings.online);
        assert!(settings.auto_match, "a non-boolean falls back");
        let folder = &settings.sources[1];
        assert_eq!(folder.id, SourceId::Folder);
        assert!(!folder.enabled, "the first entry for a source wins");
        assert_eq!(settings.sources[2].api_key, None, "blank keys are none");
        assert_eq!(
            settings.order[&Kind::AlbumArt],
            [
                SourceId::CoverArtArchive,
                SourceId::Folder,
                SourceId::Embedded
            ]
        );
        assert_eq!(settings.order[&Kind::Release], [SourceId::MusicBrainz]);

        for garbage in ["", "[]", "{", "null"] {
            store_json(&conn, garbage);
            assert_eq!(
                service_settings(&conn).unwrap(),
                ServiceSettings::default(),
                "{garbage}"
            );
        }
    }

    #[test]
    fn refuses_bad_keys() {
        let conn = db::open_in_memory().unwrap();
        let mut settings = ServiceSettings::default();
        settings.sources[2].api_key = Some("two words".into());
        let error = save_service_settings(&conn, settings).unwrap_err();
        assert!(error.to_string().contains("not valid"), "{error}");
    }
}
