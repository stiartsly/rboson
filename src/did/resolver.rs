use std::{
    future::Future,
    time::SystemTime
};
use serde::{
    Deserialize,
    Serialize
};

use super::{w3c::DIDDocument, Card};
use crate::{Id, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ResolutionStatus {
    Success = 0,
    Invalid = -1,
    NotFound = -2,
    RepresentationNotSupported = -3,
    UnsupportedMethod = -4,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct ResolutionOptions {
    pub use_cache: bool,
    pub valid_ttl: u64,
}

impl Default for ResolutionOptions {
    fn default() -> Self {
        Self {
            use_cache: true,
            valid_ttl: 0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolutionMetadata {
    pub created: Option<SystemTime>,
    pub updated: Option<SystemTime>,
    pub resolved: SystemTime,
    pub deactivated: bool,
    pub version: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolutionResult<T> {
    pub status: ResolutionStatus,
    pub result: Option<T>,
    pub result_metadata: Option<ResolutionMetadata>,
}
impl<T> ResolutionResult<T> {
    pub fn success(value: T, metadata: Option<ResolutionMetadata>) -> Self {
        Self {
            status: ResolutionStatus::Success,
            result: Some(value),
            result_metadata: metadata,
        }
    }
    pub fn not_found() -> Self {
        Self {
            status: ResolutionStatus::NotFound,
            result: None,
            result_metadata: None,
        }
    }
    pub fn invalid() -> Self {
        Self {
            status: ResolutionStatus::Invalid,
            result: None,
            result_metadata: None,
        }
    }
    pub fn succeeded(&self) -> bool {
        self.status == ResolutionStatus::Success
    }
    pub fn failed(&self) -> bool {
        !self.succeeded()
    }
}

pub trait Resolver: Send + Sync {
    fn resolve<'a>(
        &'a self,
        id: &'a Id,
        options: Option<ResolutionOptions>,
    ) -> impl Future<Output = Result<ResolutionResult<Card>>> + 'a;

    fn resolve_document<'a>(
        &'a self,
        id: &'a Id,
        options: Option<ResolutionOptions>,
    ) -> impl Future<Output = Result<ResolutionResult<DIDDocument>>> + 'a {
        async move {
            let r = self.resolve(id, options).await?;
            let doc = r.result.as_ref().map(DIDDocument::from_card);
            Ok(ResolutionResult {
                status: r.status,
                result: doc,
                result_metadata: r.result_metadata,
            })
        }
    }
}
