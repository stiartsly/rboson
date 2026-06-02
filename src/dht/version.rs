use std::collections::HashMap;
use once_cell::sync::Lazy;

pub(crate) const NODE_TAG_NAME: &str = "MK";
pub(crate) const NODE_VERSION: i32 = 1;

static NAMES: Lazy<HashMap<String, String>> = Lazy::new(|| {
    let mut map = HashMap::new();
    map.insert("OR".to_string(), "Orca".to_string());
    map.insert("MK".to_string(), "Meerkat".to_string());
    map
});

pub(crate) fn ver() -> i32 {
    build(NODE_TAG_NAME, NODE_VERSION)
}

// Build a version from the software name and version number.
pub(crate) fn build(short_name: &str, ver: i32) -> i32 {
    let bytes = short_name.as_bytes();
    ((bytes[0] as u32) << 24 | (ver as u32) & 0x0000FF00 |
    (bytes[1] as u32) << 16 | (ver as u32) & 0x000000FF) as i32
}

pub(crate) fn format_version(ver: i32) -> String {
    let ver = ver as u32;
    if ver == 0 {
        return String::from("N/A");
    }

    let mut bytes = vec![0u8; 2];
    bytes[0] = (ver >> 24) as u8;
    bytes[1] = ((ver & 0x00FF0000) >> 16) as u8;

    let n: String = bytes.iter().map(|&b| b as char).collect();
    let v = (ver & 0x0000FFFF).to_string();

    let mut str = String::new();
    str.push_str(NAMES.get(&n).map_or(NAMES.get("MK"), |v|Some(v)).unwrap());
    str.push_str("/");
    str.push_str(&v);
    str
}
