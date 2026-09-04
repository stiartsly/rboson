use std::{path::Path, sync::Arc};

use crate::{Id, Result};
use super::{
    Card,
    ResolutionResult,
    FileSystemResolutionCache
};
pub trait ResolutionCache: Send + Sync {
    fn put(&self, id: &Id, result: &ResolutionResult<Card>) -> Result<()>;
    fn get(&self, id: &Id) -> Result<Option<ResolutionResult<Card>>>;
    fn evict_expired(&self) -> Result<()>;
    fn clear(&self) -> Result<()>;
}
pub fn filesystem(
    path: impl AsRef<Path>,
    expiration_secs: u64,
) -> Result<Arc<dyn ResolutionCache>> {
    Ok(Arc::new(
        FileSystemResolutionCache::new(
            path.as_ref(),
            expiration_secs,
        )?,
    ))
}
