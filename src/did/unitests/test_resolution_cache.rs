use std::path::Path;

use crate::{
    did::filesystem_resolution_cache::FileSystemResolutionCache,
    Id
};

#[test]
fn file_name_is_derived_from_identifier() {
    let cache = FileSystemResolutionCache::new(Path::new("/tmp"), 1).unwrap();
    let id = Id::random();
    assert_eq!(cache.file(&id), Path::new("/tmp").join(id.to_string()));
}
