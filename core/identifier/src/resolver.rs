use serde::{Deserialize, Serialize};
use std::{future::Future, time::SystemTime};

use crate::did::date;
use crate::errors::ArgumentError;
use super::{w3c::DIDDocument, Card};
use crate::{Id, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(i32)]
pub enum ResolutionStatus {
    Success = 0,
    Invalid = -1,
    NotFound = -2,
    RepresentationNotSupported = -3,
    UnsupportedMethod = -4,
}

impl ResolutionStatus {
    pub const fn code(self) -> i32 {
        self as i32
    }
}

impl TryFrom<i32> for ResolutionStatus {
    type Error = crate::Error;

    fn try_from(code: i32) -> Result<Self> {
        match code {
            0 => Ok(Self::Success),
            -1 => Ok(Self::Invalid),
            -2 => Ok(Self::NotFound),
            -3 => Ok(Self::RepresentationNotSupported),
            -4 => Ok(Self::UnsupportedMethod),
            _ => Err(ArgumentError::new(format!(
                "Invalid resolution status: {code}"
            ))),
        }
    }
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
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        serialize_with = "date::serialize_opt",
        deserialize_with = "date::deserialize_opt"
    )]
    pub created: Option<SystemTime>,

    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        serialize_with = "date::serialize_opt",
        deserialize_with = "date::deserialize_opt"
    )]
    pub updated: Option<SystemTime>,

    #[serde(serialize_with = "date::serialize", deserialize_with = "date::deserialize")]
    pub resolved: SystemTime,

    pub deactivated: bool,
    pub version: i32,
}

impl ResolutionMetadata {
    pub fn new(
        created: Option<SystemTime>,
        updated: Option<SystemTime>,
        resolved: SystemTime,
        deactivated: bool,
        version: i32,
    ) -> Self {
        Self {
            created,
            updated,
            resolved,
            deactivated,
            version,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolutionResult<T> {
    pub status: ResolutionStatus,
    pub result: Option<T>,
    pub metadata: Option<ResolutionMetadata>,
}
impl<T> ResolutionResult<T> {
    pub fn success(value: T, metadata: Option<ResolutionMetadata>) -> Self {
        Self {
            status: ResolutionStatus::Success,
            result: Some(value),
            metadata,
        }
    }
    pub fn not_found() -> Self {
        Self {
            status: ResolutionStatus::NotFound,
            result: None,
            metadata: None,
        }
    }
    pub fn invalid() -> Self {
        Self {
            status: ResolutionStatus::Invalid,
            result: None,
            metadata: None,
        }
    }
    pub fn status(&self) -> ResolutionStatus {
        self.status
    }

    pub fn value(&self) -> Option<&T> {
        self.result.as_ref()
    }

    pub fn metadata(&self) -> Option<&ResolutionMetadata> {
        self.metadata.as_ref()
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
                metadata: r.metadata,
            })
        }
    }
}
