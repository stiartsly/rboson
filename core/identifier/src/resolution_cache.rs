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

use std::{path::Path, sync::Arc};

use super::{Card, FileSystemResolutionCache, ResolutionResult};
use boson_api::{Id, Result};
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
    Ok(Arc::new(FileSystemResolutionCache::new(
        path.as_ref(),
        expiration_secs,
    )?))
}

#[cfg(test)]
#[path = "unitests/test_resolution_cache.rs"]
mod test_resolution_cache;
