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
    collections::HashMap,
    sync::{Arc, Mutex},
};

use super::{Card, ResolutionCache, ResolutionOptions, ResolutionResult, Resolver};
use boson_api::{Id, Result};

pub struct CachedResolver<R> {
    inner: R,
    cache: Arc<Mutex<HashMap<Id, ResolutionResult<Card>>>>,
    persistent: Option<Arc<dyn ResolutionCache>>,
}

impl<R> CachedResolver<R> {
    pub fn new(inner: R, persistent: Option<Arc<dyn ResolutionCache>>) -> Self {
        Self {
            inner,
            cache: Arc::new(Mutex::new(HashMap::new())),
            persistent,
        }
    }

    /// Drops a cached answer for `id`.
    pub fn invalidate(&self, id: &Id) {
        self.cache.lock().unwrap().remove(id);
    }
}

impl<R: Resolver> Resolver for CachedResolver<R> {
    async fn resolve<'a>(
        &'a self,
        id: &'a Id,
        options: Option<ResolutionOptions>,
    ) -> Result<ResolutionResult<Card>> {
        let opts = options.unwrap_or_default();
        if opts.use_cache {
            if let Some(v) = self.cache.lock().unwrap().get(id).cloned() {
                return Ok(v);
            }
            if let Some(p) = &self.persistent {
                if let Some(v) = p.get(id)? {
                    if opts.valid_ttl == 0
                        || v.metadata
                            .as_ref()
                            .and_then(|m| m.resolved.elapsed().ok())
                            .map(|d| d.as_secs() < opts.valid_ttl)
                            .unwrap_or(false)
                    {
                        self.cache.lock().unwrap().insert(*id, v.clone());
                        return Ok(v);
                    }
                }
            }
        }
        let result = self.inner.resolve(id, Some(opts)).await?;
        if result.succeeded() {
            self.cache.lock().unwrap().insert(*id, result.clone());
            if let Some(p) = &self.persistent {
                let _ = p.put(id, &result);
            }
        }
        Ok(result)
    }
}
