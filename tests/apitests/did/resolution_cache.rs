use std::{
    fs,
    time::{SystemTime, UNIX_EPOCH},
};

use boson::{
    did::{Card, ResolutionResult},
    CryptoIdentity, Id,
};

fn temporary_directory() -> std::path::PathBuf {
    let suffix = format!(
        "{}-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos(),
        Id::random()
    );
    let path = std::env::temp_dir().join(format!("boson-resolution-cache-{suffix}"));
    fs::create_dir_all(&path).unwrap();
    path
}

#[test]
fn filesystem_cache_put_get_clear_and_evict() {
    let directory = temporary_directory();
    let cache = boson::did::resolution_cache::filesystem(&directory, 3600).unwrap();
    let id = Id::random();
    let card = Card::builder(CryptoIdentity::new()).build().unwrap();
    let result = ResolutionResult::success(card, None);

    cache.put(&id, &result).unwrap();
    let loaded = cache.get(&id).unwrap().unwrap();
    assert_eq!(loaded.status, result.status);

    cache.evict_expired().unwrap();
    cache.clear().unwrap();
    assert!(cache.get(&id).unwrap().is_none());
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn filesystem_cache_reports_misses() {
    let directory = temporary_directory();
    let cache = boson::did::resolution_cache::filesystem(&directory, 3600).unwrap();
    assert!(cache.get(&Id::random()).unwrap().is_none());
    fs::remove_dir_all(directory).unwrap();
}
