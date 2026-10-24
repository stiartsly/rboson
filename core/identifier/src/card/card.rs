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

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::HashMap;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::str::FromStr;
use std::time::SystemTime;

use crate::date;
use boson_api::{
    errors::{ArgumentError, SignatureError},
    Result,
    Error,
    signature,
    CryptoIdentity,
    Id,
    utils::codec,
};

use crate::{w3c::DIDDocument, CardBuilder, Credential};

const DEFAULT_PROFILE_CREDENTIAL_ID: &str = "profile";
const DEFAULT_PROFILE_CREDENTIAL_TYPE: &str = "BosonProfile";
const DEFAULT_HOME_NODE_SERVICE_ID: &str = "homeNode";
const DEFAULT_HOME_NODE_SERVICE_TYPE: &str = "BosonHomeNode";

fn is_none<T>(value: &&Option<T>) -> bool {
    value.is_none()
}

#[derive(Serialize)]
struct CardSignData<'a> {
    #[serde(rename = "id")]
    id: &'a Id,

    #[serde(
        rename = "c",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    credentials: &'a Option<Vec<Credential>>,

    #[serde(
        rename = "s",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    services: &'a Option<Vec<Service>>,

    #[serde(
        rename = "sat",
        default,
        serialize_with = "date::serialize_opt",
        skip_serializing_if = "Option::is_none",
    )]
    signed_at: &'a Option<SystemTime>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Eq)]
pub struct Card {
    #[serde(rename = "id")]
    id: Id,

    #[serde(
        rename = "c",
        skip_serializing_if = "codec::is_default"
    )]
    credentials: Option<Vec<Credential>>,

    #[serde(
        rename = "s",
        skip_serializing_if = "codec::is_default"
    )]
    services: Option<Vec<Service>>,

    #[serde(
        rename = "sat",
        serialize_with = "date::serialize_opt",
        deserialize_with = "date::deserialize_opt",
        skip_serializing_if = "Option::is_none",
    )]
    signed_at: Option<SystemTime>,

    #[serde(
        rename = "sig",
        default,
        serialize_with = "codec::bytes::serialize",
        deserialize_with = "codec::bytes::deserialize",
        skip_serializing_if = "codec::is_default"
    )]
    signature: Vec<u8>,

    #[serde(skip)]
    doc: Option<DIDDocument>,
}

impl Card {
    pub(crate) fn unsigned(
        id: Id,
        credentials: Option<Vec<Credential>>,
        services: Option<Vec<Service>>,
        doc: Option<DIDDocument>,
    ) -> Self {
        Self {
            id,
            credentials: credentials.filter(|values| !values.is_empty()),
            services: services.filter(|values| !values.is_empty()),
            signed_at: None,
            signature: vec![0u8; 0],
            doc,
        }
    }

    pub(crate) fn signed(
        mut unsigned: Self,
        signed_at: Option<SystemTime>,
        signature: Option<Vec<u8>>,
    ) -> Self {
        if signed_at.is_some() {
            unsigned.signed_at = signed_at;
        }
        unsigned.signature = signature.unwrap_or_default();
        unsigned
    }

    pub fn id(&self) -> &Id {
        &self.id
    }

    pub fn credentials(&self) -> Vec<&Credential> {
        self.credentials
            .as_ref()
            .map(|v| v.iter().collect())
            .unwrap_or_default()
    }

    pub fn credentials_by_type(&self, credential_type: &str) -> Vec<&Credential> {
        self.credentials
            .as_ref()
            .map(|v| {
                v.iter()
                    .filter(|c| c.types().contains(&credential_type))
                    .collect()
            })
            .unwrap_or_default()
    }

    pub fn credential(&self, id: &str) -> Option<&Credential> {
        self.credentials
            .as_ref()
            .and_then(|credentials| credentials.iter().find(|credential| credential.id() == id))
    }

    pub fn credential_with_type(&self, id: &str, credential_type: &str) -> Option<&Credential> {
        self.credentials
            .as_ref()
            .and_then(|credentials| {
                credentials.iter().find(|credential| {
                    credential.id() == id
                        && credential.types().contains(&credential_type)
                })
            })
    }

    pub fn credentials_by_id(&self, id: &str) -> Vec<&Credential> {
        self.credentials
            .as_ref()
            .map(|v| v.iter().filter(|c| c.id() == id).collect())
            .unwrap_or_default()
    }

    pub fn profile_credential(&self) -> Option<&Credential> {
        self.credentials
            .as_ref()
            .map(|v| {
                v.iter().find(|c| {
                    c.id() == DEFAULT_PROFILE_CREDENTIAL_ID
                        && c.types().contains(&DEFAULT_PROFILE_CREDENTIAL_TYPE)
                })
            })
            .flatten()
    }

    pub fn services(&self) -> Vec<&Service> {
        self.services
            .as_ref()
            .map(|v| v.iter().collect())
            .unwrap_or_default()
    }

    pub fn services_by_type(&self, service_type: &str) -> Vec<&Service> {
        self.services
            .as_ref()
            .map(|v| {
                v.iter()
                    .filter(|s| s.service_type() == service_type)
                    .collect()
            })
            .unwrap_or_default()
    }

    pub fn service(&self, id: &str) -> Option<&Service> {
        self.services
            .as_ref()
            .and_then(|services| services.iter().find(|service| service.id() == id))
    }

    pub fn service_with_type(&self, id: &str, service_type: &str) -> Option<&Service> {
        self.services.as_ref().and_then(|services| {
            services
                .iter()
                .find(|service| service.id() == id && service.service_type() == service_type)
        })
    }

    pub fn services_by_id(&self, id: &str) -> Vec<&Service> {
        self.services
            .as_ref()
            .map(|v| v.iter().filter(|s| s.id() == id).collect())
            .unwrap_or_default()
    }

    pub fn homenode_service(&self) -> Option<&Service> {
        self.services
            .as_ref()
            .map(|v| {
                v.iter().find(|s| {
                    s.id() == DEFAULT_HOME_NODE_SERVICE_ID
                        && s.service_type() == DEFAULT_HOME_NODE_SERVICE_TYPE
                })
            })
            .flatten()
    }

    pub fn signed_at(&self) -> Option<SystemTime> {
        self.signed_at
    }

    pub fn signature(&self) -> &[u8] {
        &self.signature
    }

    pub fn is_genuine(&self) -> bool {
        if self.signature.len() != signature::Signature::BYTES {
            return false;
        }

        signature::verify(
            &self.to_sign_data(),
            &self.signature,
            &self.id.to_signature_key(),
        )
        .unwrap_or(false)
    }

    pub fn validate(&self) -> Result<()> {
        if self.signature.is_empty() {
            return Err(SignatureError::new("Card signature is empty"));
        }

        match self.is_genuine() {
            true => Ok(()),
            false => Err(SignatureError::new("Card signature is not valid")),
        }
    }

    pub(crate) fn to_sign_data(&self) -> Vec<u8> {
        serde_cbor::to_vec(&CardSignData {
            id: &self.id,
            credentials: &self.credentials,
            services: &self.services,
            signed_at: &self.signed_at,
        })
        .unwrap()
    }

    pub fn did_doc(&self) -> Option<&DIDDocument> {
        self.doc.as_ref()
    }

    pub fn parse(json: &str) -> Result<Self> {
        Self::try_from(json)
    }

    pub fn parse_bytes(cbor: &[u8]) -> Result<Self> {
        Self::try_from(cbor)
    }

    pub fn to_bytes(&self) -> Result<Vec<u8>> {
        serde_cbor::to_vec(self).map_err(|error| {
            ArgumentError::new(format!("Failed to encode Card: {error}")).into()
        })
    }

    pub fn to_pretty_string(&self) -> Result<String> {
        serde_json::to_string_pretty(self).map_err(|error| {
            ArgumentError::new(format!("Failed to encode Card: {error}")).into()
        })
    }

    pub fn builder(subject: CryptoIdentity) -> CardBuilder {
        CardBuilder::new(subject)
    }
}

impl PartialEq<Self> for Card {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
            && self.credentials == other.credentials
            && self.services == other.services
            && self.signed_at == other.signed_at
            && self.signature == other.signature
    }
}

impl fmt::Display for Card {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        serde_json::to_string(self).map_err(|_| fmt::Error)?.fmt(f)
    }
}

impl TryFrom<&str> for Card {
    type Error = Error;

    fn try_from(data: &str) -> Result<Self> {
        serde_json::from_str(data).map_err(|e| {
            ArgumentError::new(format!("Failed to parse Card from string: {}", e)).into()
        })
    }
}

impl TryFrom<&[u8]> for Card {
    type Error = Error;

    fn try_from(data: &[u8]) -> Result<Self> {
        serde_cbor::from_slice(data).map_err(|e| {
            ArgumentError::new(format!("Failed to parse Card from bytes: {}", e)).into()
        })
    }
}

impl FromStr for Card {
    type Err = Error;

    fn from_str(data: &str) -> Result<Self> {
        Self::try_from(data)
    }
}

impl From<&Card> for Vec<u8> {
    fn from(card: &Card) -> Self {
        serde_cbor::to_vec(card).unwrap()
    }
}

impl From<&Card> for String {
    fn from(card: &Card) -> Self {
        serde_json::to_string(card).unwrap()
    }
}

#[derive(Debug, Clone, Eq, Serialize, Deserialize)]
pub struct Service {
    #[serde(rename = "id")]
    id: String,

    #[serde(rename = "t")]
    service_type: String,

    #[serde(rename = "e")]
    endpoint: String,

    #[serde(flatten)]
    properties: Map<String, Value>,
}

impl Service {
    pub(crate) fn new(
        id: String,
        service_type: String,
        endpoint: String,
        properties: Map<String, Value>,
    ) -> Self {
        Self {
            id,
            service_type,
            endpoint,
            properties,
        }
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn service_type(&self) -> &str {
        &self.service_type
    }

    pub fn endpoint(&self) -> &str {
        &self.endpoint
    }

    pub fn properties_map(&self) -> &Map<String, Value> {
        &self.properties
    }

    pub fn properties<T>(&self) -> HashMap<&str, T>
    where
        T: Serialize + DeserializeOwned,
    {
        self.properties
            .iter()
            .filter_map(|(k, v)| {
                serde_json::from_value(v.clone())
                    .ok()
                    .map(|value| (k.as_str(), value))
            })
            .collect()
    }

    pub fn property<T>(&self, key: &str) -> Option<T>
    where
        T: Serialize + DeserializeOwned,
    {
        self.properties
            .get(key)
            .and_then(|v| serde_json::from_value(v.clone()).ok())
    }
}

impl PartialEq<Self> for Service {
    fn eq(&self, other: &Service) -> bool {
        self.id == other.id
            && self.service_type == other.service_type
            && self.endpoint == other.endpoint
            && self.properties == other.properties
    }
}

impl Hash for Service {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.id.hash(state);
        self.service_type.hash(state);
        self.endpoint.hash(state);
        let mut properties = self
            .properties
            .iter()
            .map(|(key, value)| (key, serde_json::to_string(value).unwrap()))
            .collect::<Vec<_>>();
        properties.sort_by(|(left, _), (right, _)| left.cmp(right));
        properties.hash(state);
    }
}
