//! Which metadata sources are used, and in what order. Each kind of data
//! (album details, album art, album descriptions, artist biographies) has
//! an ordered list of the sources that can supply it, and the first with a result wins. Stored as one JSON value
//! under `metadata.services` in `settings`, read like `library.sort`:
//! whatever is usable is kept and the rest falls back to the defaults.
//! Keys are not stored here but in the OS keychain (`keys`); the settings
//! only say whether a source has one.

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
    /// An artist's biography. The artist's own match (on MusicBrainz, which
    /// links to the other sources) comes with the `Release` source.
    ArtistInfo,
    /// A description of an album, reached through its `Release` match.
    AlbumInfo,
}

impl Kind {
    pub const ALL: [Kind; 4] = [
        Kind::Release,
        Kind::AlbumArt,
        Kind::ArtistInfo,
        Kind::AlbumInfo,
    ];
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
    /// Artist biographies and album descriptions: the lead of the English
    /// Wikipedia article that the artist's or album's MusicBrainz entry
    /// links to through Wikidata.
    Wikipedia,
    /// Album details (credits, styles, labels, formats) with the user's own
    /// token. Its terms allow keeping only the match: details are fetched
    /// when shown and never stored, and its pictures aren't used at all
    /// (PLAN.md 4.8).
    Discogs,
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
    /// What the key is called, and where the user gets one.
    pub key_name: Option<&'static str>,
    pub key_url: Option<&'static str>,
    /// On until the user turns it off.
    pub enabled_by_default: bool,
    /// Its details are kept in the library; otherwise only the match is,
    /// and the details are fetched when shown (`metadata_release_details`).
    pub stores_details: bool,
    /// Shown next to its data, linked to the page it came from, as its
    /// terms require.
    pub credit: Option<&'static str>,
    /// Shown with the source in the settings, as its terms require.
    pub notice: Option<&'static str>,
    /// Another source it relies on (the Cover Art Archive needs a
    /// MusicBrainz match, and Wikipedia a MusicBrainz artist or album).
    pub requires: Option<SourceId>,
    pub homepage: Option<&'static str>,
    /// The hosts it contacts, as `metadata-progress` names those that
    /// can't be reached.
    pub hosts: &'static [&'static str],
}

impl SourceId {
    /// Every source, in the order the UI lists them.
    pub const ALL: [SourceId; 6] = [
        SourceId::Embedded,
        SourceId::Folder,
        SourceId::MusicBrainz,
        SourceId::CoverArtArchive,
        SourceId::Wikipedia,
        SourceId::Discogs,
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
            SourceId::Wikipedia => (
                "Wikipedia",
                &[Kind::ArtistInfo, Kind::AlbumInfo],
                true,
                Some(SourceId::MusicBrainz),
                Some("https://en.wikipedia.org"),
            ),
            SourceId::Discogs => (
                "Discogs",
                &[Kind::Release],
                true,
                None,
                Some("https://www.discogs.com"),
            ),
        };
        let hosts: &'static [&'static str] = match self {
            SourceId::Embedded | SourceId::Folder => &[],
            SourceId::MusicBrainz => &[super::musicbrainz::HOST],
            SourceId::CoverArtArchive => &[super::coverartarchive::HOST],
            SourceId::Wikipedia => &[super::wikipedia::WIKIDATA_HOST, super::wikipedia::HOST],
            SourceId::Discogs => &[super::discogs::HOST],
        };
        let discogs = self == SourceId::Discogs;
        SourceInfo {
            id: self,
            name,
            kinds,
            online,
            needs_key: discogs,
            key_name: discogs.then_some("Personal access token"),
            key_url: discogs.then_some("https://www.discogs.com/settings/developers"),
            enabled_by_default: !discogs,
            stores_details: !discogs,
            credit: discogs.then_some(super::discogs::CREDIT),
            notice: discogs.then_some(super::discogs::NOTICE),
            requires,
            homepage,
            hosts,
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
            SourceId::Wikipedia => "wikipedia",
            SourceId::Discogs => "discogs",
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
    /// Whether its key is in the keychain (`keys`). Only `keys::set_key`
    /// changes it: saving the settings keeps what was stored.
    #[serde(default)]
    pub has_key: bool,
}

impl SourceSettings {
    fn default_for(id: SourceId) -> SourceSettings {
        SourceSettings {
            id,
            enabled: id.info().enabled_by_default,
            has_key: false,
        }
    }
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
                .map(SourceSettings::default_for)
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
    pub fn source(&self, id: SourceId) -> Option<&SourceSettings> {
        self.sources.iter().find(|source| source.id == id)
    }

    pub fn source_mut(&mut self, id: SourceId) -> Option<&mut SourceSettings> {
        self.sources.iter_mut().find(|source| source.id == id)
    }

    /// Whether `id` may be used now: enabled, online sources only while the
    /// online switch is on, with a key if it needs one, and with the source
    /// it relies on usable too.
    pub fn is_usable(&self, id: SourceId) -> bool {
        self.is_enabled(id, self.online)
    }

    /// Whether what `id` supplied is shown: as `is_usable`, but whatever
    /// the online switch says. The switch stops the app contacting
    /// services; what it already fetched keeps showing, as it does when
    /// the network is down.
    pub fn is_shown(&self, id: SourceId) -> bool {
        self.is_enabled(id, true)
    }

    fn is_enabled(&self, id: SourceId, online: bool) -> bool {
        let info = id.info();
        self.source(id)
            .is_some_and(|source| source.enabled && (!info.needs_key || source.has_key))
            && (!info.online || online)
            && info
                .requires
                .is_none_or(|required| self.is_enabled(required, online))
    }

    /// The usable sources for `kind`, in priority order.
    pub fn sources_for(&self, kind: Kind) -> Vec<SourceId> {
        self.ordered(kind, |id| self.is_usable(id))
    }

    /// The sources whose data for `kind` is shown, in priority order: the
    /// usable ones, and online ones while the online switch is off.
    pub fn sources_shown(&self, kind: Kind) -> Vec<SourceId> {
        self.ordered(kind, |id| self.is_shown(id))
    }

    fn ordered(&self, kind: Kind, keep: impl Fn(SourceId) -> bool) -> Vec<SourceId> {
        self.order
            .get(&kind)
            .map(|order| order.iter().copied().filter(|&id| keep(id)).collect())
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
            let source = self
                .sources
                .iter()
                .find(|source| source.id == id)
                .cloned()
                .unwrap_or_else(|| SourceSettings::default_for(id));
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
        ServiceSettings {
            online: flag("online", defaults.online),
            auto_match: flag("autoMatch", defaults.auto_match),
            sources,
            order,
        }
        .normalized()
    }

    /// These settings with each source's `has_key` taken from `stored`.
    fn with_keys_of(mut self, stored: &ServiceSettings) -> ServiceSettings {
        for source in &mut self.sources {
            source.has_key = stored.source(source.id).is_some_and(|s| s.has_key);
        }
        self
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
/// was stored. Which sources have a key is kept as it was.
pub fn save_service_settings(
    conn: &Connection,
    settings: ServiceSettings,
) -> Result<ServiceSettings, Error> {
    let settings = settings.normalized().with_keys_of(&service_settings(conn)?);
    store(conn, &settings)?;
    Ok(settings)
}

/// Records whether `source` has a key in the keychain; saving a key turns
/// the source on.
pub fn set_has_key(
    conn: &Connection,
    source: SourceId,
    has_key: bool,
) -> Result<ServiceSettings, Error> {
    let mut settings = service_settings(conn)?;
    if let Some(entry) = settings.source_mut(source) {
        entry.has_key = has_key;
        entry.enabled |= has_key;
    }
    store(conn, &settings)?;
    Ok(settings)
}

fn store(conn: &Connection, settings: &ServiceSettings) -> Result<(), Error> {
    let json = serde_json::to_string(settings).expect("service settings serialize to JSON");
    conn.execute(
        "INSERT INTO settings (key, value) VALUES (?1, ?2)
         ON CONFLICT (key) DO UPDATE SET value = excluded.value",
        [SETTINGS_KEY, &json],
    )?;
    Ok(())
}

/// Back to the default sources and order. Keys stay in the keychain, so
/// which sources have one is kept.
pub fn reset_service_settings(conn: &Connection) -> Result<ServiceSettings, Error> {
    let settings = ServiceSettings::default().with_keys_of(&service_settings(conn)?);
    conn.execute("DELETE FROM settings WHERE key = ?1", [SETTINGS_KEY])?;
    if settings != ServiceSettings::default() {
        store(conn, &settings)?;
    }
    Ok(settings)
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
        // Discogs is off, and needs a key besides.
        assert_eq!(settings.sources_for(Kind::Release), [SourceId::MusicBrainz]);
        assert_eq!(
            settings.order[&Kind::Release],
            [SourceId::MusicBrainz, SourceId::Discogs]
        );
        assert_eq!(
            settings.sources_for(Kind::ArtistInfo),
            [SourceId::Wikipedia]
        );
        assert_eq!(settings.sources_for(Kind::AlbumInfo), [SourceId::Wikipedia]);
    }

    #[test]
    fn a_source_needing_a_key_is_used_once_it_has_one() {
        let conn = db::open_in_memory().unwrap();
        let mut settings = ServiceSettings::default();
        settings.source_mut(SourceId::Discogs).unwrap().enabled = true;
        // The UI can't claim a key the keychain doesn't have.
        settings.source_mut(SourceId::Discogs).unwrap().has_key = true;
        let saved = save_service_settings(&conn, settings).unwrap();
        assert!(!saved.source(SourceId::Discogs).unwrap().has_key);
        assert!(!saved.is_usable(SourceId::Discogs));

        let saved = set_has_key(&conn, SourceId::Discogs, true).unwrap();
        assert!(saved.is_usable(SourceId::Discogs));
        assert_eq!(
            saved.sources_for(Kind::Release),
            [SourceId::MusicBrainz, SourceId::Discogs]
        );
        assert_eq!(service_settings(&conn).unwrap(), saved);

        // Saving keeps the key; so does a reset, which turns Discogs off.
        let saved = save_service_settings(&conn, ServiceSettings::default()).unwrap();
        assert!(saved.source(SourceId::Discogs).unwrap().has_key);
        let reset = reset_service_settings(&conn).unwrap();
        assert!(reset.source(SourceId::Discogs).unwrap().has_key);
        assert!(!reset.is_usable(SourceId::Discogs));
        assert_eq!(service_settings(&conn).unwrap(), reset);

        // A key removed: off until another is saved, which turns it on.
        let saved = set_has_key(&conn, SourceId::Discogs, false).unwrap();
        assert!(!saved.source(SourceId::Discogs).unwrap().has_key);
        let saved = set_has_key(&conn, SourceId::Discogs, true).unwrap();
        assert!(saved.is_usable(SourceId::Discogs));
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
    fn online_sources_name_their_hosts() {
        for id in SourceId::ALL {
            let info = id.info();
            assert_eq!(info.online, !info.hosts.is_empty(), "{id:?}");
        }
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
        // What was fetched still shows.
        assert_eq!(
            settings.sources_shown(Kind::AlbumArt),
            default_order(Kind::AlbumArt)
        );

        // The Cover Art Archive needs a MusicBrainz match.
        let mut settings = ServiceSettings::default();
        settings.sources[2].enabled = false; // MusicBrainz.
        assert!(!settings.is_usable(SourceId::CoverArtArchive));
        assert!(!settings.is_shown(SourceId::CoverArtArchive));
        assert!(!settings.is_usable(SourceId::Wikipedia));
        assert!(settings.sources_for(Kind::AlbumInfo).is_empty());
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
                    {"id": "musicbrainz", "enabled": true, "hasKey": "yes"}
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
        assert!(
            settings.sources[2].enabled,
            "an entry that doesn't parse falls back"
        );
        assert_eq!(
            settings.order[&Kind::AlbumArt],
            [
                SourceId::CoverArtArchive,
                SourceId::Folder,
                SourceId::Embedded
            ]
        );
        // Settings stored before Discogs existed get it, off, at the end.
        assert_eq!(
            settings.order[&Kind::Release],
            [SourceId::MusicBrainz, SourceId::Discogs]
        );
        assert!(!settings.source(SourceId::Discogs).unwrap().enabled);
        // Settings stored before a kind existed get its default order.
        assert_eq!(settings.order[&Kind::ArtistInfo], [SourceId::Wikipedia]);
        assert_eq!(settings.order[&Kind::AlbumInfo], [SourceId::Wikipedia]);

        for garbage in ["", "[]", "{", "null"] {
            store_json(&conn, garbage);
            assert_eq!(
                service_settings(&conn).unwrap(),
                ServiceSettings::default(),
                "{garbage}"
            );
        }
    }
}
