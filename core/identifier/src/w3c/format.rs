//! Shared JSON/CBOR format helpers for W3C DID objects.

use serde::de::DeserializeOwned;
use serde::Serialize;

use boson_api::{errors::ArgumentError, Result};

/// Common serialization surface of W3C DID objects.
pub trait W3CDIDFormat: Serialize + DeserializeOwned + Sized {
    fn to_bytes(&self) -> Result<Vec<u8>> {
        serde_cbor::to_vec(self).map_err(|error| {
            ArgumentError::new(format!("Failed to encode {}: {error}", std::any::type_name::<Self>()))
                .into()
        })
    }

    fn to_pretty_string(&self) -> Result<String> {
        serde_json::to_string_pretty(self).map_err(|error| {
            ArgumentError::new(format!("Failed to encode {}: {error}", std::any::type_name::<Self>()))
                .into()
        })
    }

    fn parse(json: &str) -> Result<Self> {
        serde_json::from_str(json).map_err(|error| {
            ArgumentError::new(format!(
                "Failed to parse {} from string: {error}",
                std::any::type_name::<Self>()
            ))
            .into()
        })
    }

    fn parse_bytes(cbor: &[u8]) -> Result<Self> {
        serde_cbor::from_slice(cbor).map_err(|error| {
            ArgumentError::new(format!(
                "Failed to parse {} from CBOR: {error}",
                std::any::type_name::<Self>()
            ))
            .into()
        })
    }
}

impl W3CDIDFormat for super::DIDDocument {}
impl W3CDIDFormat for super::VerifiableCredential {}
impl W3CDIDFormat for super::VerifiablePresentation {}
