//! Keys for the sources that need one (a Discogs personal access token),
//! kept in the OS keychain rather than the library database: a token can
//! act on the user's account at the service, and the database is a plain
//! file (PLAN.md 4.8). The settings record only whether a source has a key
//! (`SourceSettings::has_key`).
//!
//! Reads are cached for the life of the process, since the worker asks for
//! a key on every job. Tests use a map in memory instead of the keychain.

use std::collections::HashMap;
use std::sync::Mutex;

use super::settings::SourceId;
use super::Error;

/// The keychain entries' service; the account is the source id.
#[cfg(all(target_os = "macos", not(test)))]
const SERVICE: &str = "ano-mp metadata";

/// Keys read or written so far: `None` when the keychain has none.
static CACHE: Mutex<Option<HashMap<SourceId, Option<String>>>> = Mutex::new(None);

/// Whether `key` could be a key: no spaces or control characters, and not
/// absurdly long.
pub fn is_valid(key: &str) -> bool {
    !key.is_empty() && key.len() <= 256 && !key.chars().any(|c| c.is_whitespace() || c.is_control())
}

/// The key stored for `source`, if any.
pub fn get(source: SourceId) -> Result<Option<String>, Error> {
    let mut cache = CACHE.lock().unwrap_or_else(|e| e.into_inner());
    let cache = cache.get_or_insert_with(HashMap::new);
    if let Some(key) = cache.get(&source) {
        return Ok(key.clone());
    }
    let key = store::read(source)?;
    cache.insert(source, key.clone());
    Ok(key)
}

/// Stores `key` for `source` (trimmed), or removes it with `None`.
pub fn set(source: SourceId, key: Option<&str>) -> Result<(), Error> {
    let key = key.map(str::trim).filter(|key| !key.is_empty());
    if let Some(key) = key {
        if !is_valid(key) {
            return Err(Error::Invalid(format!(
                "That isn't a valid {} key",
                source.info().name
            )));
        }
    }
    let mut cache = CACHE.lock().unwrap_or_else(|e| e.into_inner());
    match key {
        Some(key) => store::write(source, key)?,
        None => store::delete(source)?,
    }
    cache
        .get_or_insert_with(HashMap::new)
        .insert(source, key.map(str::to_owned));
    Ok(())
}

#[cfg(all(target_os = "macos", not(test)))]
mod store {
    use apple_native_keyring_store::keychain::{Cred, MacKeychainDomain};
    use keyring_core::{Entry, Error as KeyringError};

    use super::{SourceId, SERVICE};
    use crate::metadata::Error;

    fn entry(source: SourceId) -> Result<Entry, Error> {
        Cred::build(MacKeychainDomain::User, SERVICE, source.as_str()).map_err(failed)
    }

    fn failed(error: KeyringError) -> Error {
        Error::Invalid(format!("The keychain couldn't be used: {error}"))
    }

    pub fn read(source: SourceId) -> Result<Option<String>, Error> {
        match entry(source)?.get_password() {
            Ok(key) => Ok(Some(key)),
            Err(KeyringError::NoEntry) => Ok(None),
            Err(error) => Err(failed(error)),
        }
    }

    pub fn write(source: SourceId, key: &str) -> Result<(), Error> {
        entry(source)?.set_password(key).map_err(failed)
    }

    pub fn delete(source: SourceId) -> Result<(), Error> {
        match entry(source)?.delete_credential() {
            Ok(()) | Err(KeyringError::NoEntry) => Ok(()),
            Err(error) => Err(failed(error)),
        }
    }
}

/// Other platforms get their keychain in their phases (PLAN.md 8–10; iOS
/// needs the protected data store). Until then keys can't be saved there.
#[cfg(all(not(target_os = "macos"), not(test)))]
mod store {
    use super::SourceId;
    use crate::metadata::Error;

    fn unsupported() -> Error {
        Error::Invalid("Keys can't be saved on this platform yet".into())
    }

    pub fn read(_source: SourceId) -> Result<Option<String>, Error> {
        Ok(None)
    }

    pub fn write(_source: SourceId, _key: &str) -> Result<(), Error> {
        Err(unsupported())
    }

    pub fn delete(_source: SourceId) -> Result<(), Error> {
        Ok(())
    }
}

/// A map in memory, so tests never touch the keychain. The cache above is
/// shared by all tests, so they must use it only through `set` and `get`.
#[cfg(test)]
mod store {
    use super::SourceId;
    use crate::metadata::Error;

    pub fn read(_source: SourceId) -> Result<Option<String>, Error> {
        // Whatever `set` wrote is in the cache; nothing else exists.
        Ok(None)
    }

    pub fn write(_source: SourceId, _key: &str) -> Result<(), Error> {
        Ok(())
    }

    pub fn delete(_source: SourceId) -> Result<(), Error> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checks_keys() {
        assert!(is_valid("abcDEF123_-"));
        for bad in ["", "two words", "tab\there", &"x".repeat(257)] {
            assert!(!is_valid(bad), "{bad}");
        }
        let error = set(SourceId::Discogs, Some("not a key")).unwrap_err();
        assert!(error.to_string().contains("valid"), "{error}");
    }
}
