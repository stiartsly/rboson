// Copyright (c) 2023 -      bosonnetwork.io
//
// Permission is hereby granted, free of charge, to any person obtaining a copy
// of this software and associated documentation files (the "Software"), to deal
// in the Software without restriction, including without limitation the rights
// to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
// copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions:
//
// The above copyright notice and this permission notice shall be included in all
// copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
// OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
// SOFTWARE.

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

#[cfg(test)]
mod tests {
    use super::*;

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
}
