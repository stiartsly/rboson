use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use super::{
    Card,
    ResolutionCache,
    ResolutionOptions,
    ResolutionResult,
    Resolver
};
use crate::{Id, Result};

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
}

impl<R: Resolver> Resolver for CachedResolver<R> {
    async fn resolve<'a>(
        &'a self,
        id: &'a Id,
        options: Option<ResolutionOptions>,
    ) -> Result<ResolutionResult<Card>>
    {
        let opts = options.unwrap_or_default();
        if opts.use_cache {
            if let Some(v) = self.cache.lock().unwrap().get(id).cloned() {
                return Ok(v);
            }
            if let Some(p) = &self.persistent {
                if let Some(v) = p.get(id)? {
                    if opts.valid_ttl == 0
                        || v.result_metadata
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
