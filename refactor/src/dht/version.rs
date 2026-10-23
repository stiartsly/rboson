use std::collections::HashMap;
use once_cell::sync::Lazy;
use crate::errors::{ArgumentError, Result};

/// The general node implementation's short name.
pub const NODE_TAG_NAME: &str = "MK";
/// The current node implementation's version.
pub const NODE_VERSION: i32 = 1;
/// Version string used when the peer did not provide one.
pub const VERSION_NOT_AVAILABLE: &str = "N/A";

static NAMES: Lazy<HashMap<&'static str, &'static str>> = Lazy::new(|| {
    HashMap::from([("OR", "Orca"), ("MK", "Meerkat")])
});

/// Encodes a two-character software name and a 16-bit version into one integer.
pub fn build(name: &str, version: i32) -> Result<i32> {
    let bytes = name.as_bytes();
    if bytes.len() < 2 {
        return Err(ArgumentError::new(
            "Invalid name: must be at least 2 characters".to_string(),
        )
        .into());
    }
    Ok((bytes[0] as i32) << 24
        | (bytes[1] as i32) << 16
        | (version & 0x0000_FFFF))
}

/// Returns the current implementation's version code.
pub fn ver() -> i32 {
    build(NODE_TAG_NAME, NODE_VERSION).expect("current version constants are valid")
}

/// Formats a version code as `Name/version`, with non-printable name bytes replaced by dots.
pub fn format_version(version: i32) -> String {
    if version == 0 {
        return VERSION_NOT_AVAILABLE.to_string();
    }

    let name_bytes = [(version >> 24) as u8, ((version & 0x00FF_0000) >> 16) as u8];
    let name: String = name_bytes
        .iter()
        .map(|byte| {
            if (0x20..=0x7E).contains(byte) {
                *byte as char
            } else {
                '.'
            }
        })
        .collect();
    let display = NAMES.get(name.as_str()).copied().unwrap_or(name.as_str());
    format!("{display}/{}", version & 0x0000_FFFF)
}

#[cfg(test)]
#[path = "unitests/test_version.rs"]
mod test_version;
