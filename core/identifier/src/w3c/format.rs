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
