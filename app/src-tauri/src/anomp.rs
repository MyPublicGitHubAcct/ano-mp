//! Safe wrappers over the anomp_core C API (core/include/anomp/anomp.h).
//! All `unsafe` FFI stays in this module.

use std::ffi::{c_char, c_int, CStr, CString};

extern "C" {
    fn anomp_version() -> *const c_char;
    fn anomp_can_decode_extension(extension: *const c_char) -> c_int;
}

/// The core library version as "major.minor.patch".
pub fn version() -> String {
    // SAFETY: anomp_version returns a static, never-null, NUL-terminated string.
    unsafe { CStr::from_ptr(anomp_version()) }
        .to_string_lossy()
        .into_owned()
}

/// Whether the core can decode files with the given extension ("flac", ".mp3").
#[allow(dead_code)]
pub fn can_decode_extension(extension: &str) -> bool {
    let Ok(extension) = CString::new(extension) else {
        return false;
    };
    // SAFETY: the pointer is a valid NUL-terminated string for the whole call.
    unsafe { anomp_can_decode_extension(extension.as_ptr()) != 0 }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_matches_crate() {
        assert_eq!(version(), env!("CARGO_PKG_VERSION"));
    }

    #[test]
    fn decodes_flac_but_not_text() {
        assert!(can_decode_extension("flac"));
        assert!(can_decode_extension(".FLAC"));
        assert!(!can_decode_extension("txt"));
        assert!(!can_decode_extension("fl\0ac"));
    }
}
