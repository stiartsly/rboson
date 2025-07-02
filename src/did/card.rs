use std::fmt;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::time::{Duration, SystemTime};
use serde::{Serialize, Deserialize};
use serde::de::DeserializeOwned;
use serde_json::{Map, Value};

use crate::{
    as_secs,
    Id,
    core::{Error, Result},
    signature,
    CryptoIdentity,
};

use super::{
    Credential,
    CardBuilder,
    w3c::DIDDocument
};

const DEFAULT_PROFILE_CREDENTIAL_ID     : &str = "profile";
const DEFAULT_PROFILE_CREDENTIAL_TYPE   : &str = "BosonProfile";
const DEFAULT_HOME_NODE_SERVICE_ID      : &str = "homeNode";
const DEFAULT_HOME_NODE_SERVICE_TYPE    : &str = "BosonHomeNode";

#[derive(Debug, Clone, Serialize, Deserialize, Eq)]
pub struct Card {
    #[serde(rename = "id")]
    id: Id,

    #[serde(rename = "c", skip_serializing_if = "super::is_none_or_empty")]
    credentials: Option<Vec<Credential>>,

    #[serde(rename = "s", skip_serializing_if = "super::is_none_or_empty")]
    services: Option<Vec<Service>>,

    #[serde(rename = "sat", skip_serializing_if = "super::is_none_or_empty")]
    signed_at: Option<u64>,

    #[serde(rename = "sig")]
    #[serde(with = "super::serde_bytes_with_base64")]
    signature: Vec<u8>,

    #[serde(skip)]
    doc: Option<DIDDocument>,
}

impl Card {
    pub(crate) fn unsigned(
        id: Id,
        credentials: Option<Vec<Credential>>,
        services: Option<Vec<Service>>,
        doc: Option<DIDDocument>
    ) -> Self {
        Self {
            id,
            credentials,
            services,
            signed_at: None,
            signature: vec![0u8;0],
            doc,
        }
    }

    pub(crate) fn signed(
        mut unsigned: Self,
        signed_at: Option<SystemTime>,
        signature: Option<Vec<u8>>
    ) -> Self {
        unsigned.signed_at = signed_at.map(|v|as_secs!(v));
        unsigned.signature = signature.unwrap_or_else(|| vec![0u8; 0]);
        unsigned
    }

    pub fn id(&self) -> &Id {
        &self.id
    }

    pub fn credentials(&self) -> Vec<&Credential> {
        self.credentials.as_ref().map(|v|
            v.iter().collect()
        ).unwrap_or_default()
    }

    pub fn credentials_by_type(&self, credential_type: &str) -> Vec<&Credential> {
        self.credentials.as_ref().map(|v|
            v.iter().filter(|c| c.types().contains(&credential_type)).collect()
        ).unwrap_or_default()
    }

    pub fn credentials_by_id(&self, id: &str) -> Vec<&Credential> {
        self.credentials.as_ref().map(|v|
            v.iter().filter(|c| c.id() == id).collect()
        ).unwrap_or_default()
    }

    pub fn profile_credential(&self) -> Option<&Credential> {
        self.credentials.as_ref().map(|v| {
            v.iter().find(|c|
                c.id() == DEFAULT_PROFILE_CREDENTIAL_ID &&
                c.types().contains(&DEFAULT_PROFILE_CREDENTIAL_TYPE)
            )
        }).flatten()
    }

    pub fn services(&self) -> Vec<&Service> {
        self.services.as_ref().map(|v|
            v.iter().collect()
        ).unwrap_or_default()
    }

    pub fn services_by_type(&self, service_type: &str) -> Vec<&Service> {
        self.services.as_ref().map(|v|
            v.iter().filter(|s| s.service_type() == service_type).collect()
        ).unwrap_or_default()
    }

    pub fn services_by_id(&self, id: &str) -> Vec<&Service> {
        self.services.as_ref().map(|v|
            v.iter().filter(|s| s.id() == id).collect()
        ).unwrap_or_default()
    }

    pub fn  homenode_service(&self) -> Option<&Service> {
        self.services.as_ref().map(|v| {
            v.iter().find(|s|
                s.id() == DEFAULT_HOME_NODE_SERVICE_ID &&
                s.service_type() == DEFAULT_HOME_NODE_SERVICE_TYPE
            )
        }).flatten()
    }

    pub fn signed_at(&self) -> Option<SystemTime> {
        self.signed_at.map(|s| {
            SystemTime::UNIX_EPOCH + Duration::from_secs(s)
        })
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
            &self.id.to_signature_key()
        ).is_ok()
    }

    pub fn validate(&self) -> Result<()> {
        if self.signature.is_empty() {
            return Err(Error::Signature("Card signature is empty".into()));
        }

        match self.is_genuine() {
            true => Ok(()),
            false => Err(Error::Signature("Card signature is not valid".into())),
        }
    }

    pub(crate) fn to_sign_data(&self) -> Vec<u8> {
        match self.signature.is_empty() {
            true    => self.into(),
            false   => Vec::from(&Self::signed(self.clone(), None, None))
        }
    }

    pub fn did_doc(&self) -> Option<&DIDDocument> {
        self.doc.as_ref()
    }

    pub fn builder(subject: CryptoIdentity) -> CardBuilder {
        CardBuilder::new(subject)
    }
}

impl PartialEq<Self> for Card {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id &&
        self.credentials == other.credentials &&
        self.services == other.services &&
        self.signed_at == other.signed_at &&
        self.signature == other.signature
    }
}

impl fmt::Display for Card {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        serde_json::to_string(self)
            .map_err(|_| fmt::Error)?
            .fmt(f)
    }
}

impl TryFrom<&str> for Card {
    type Error = Error;

    fn try_from(data: &str) -> Result<Self> {
        serde_json::from_str(data).map_err(|e| {
            Error::Argument(format!("Failed to parse Card from string: {}", e))
        })
    }
}

impl TryFrom<&[u8]> for Card {
    type Error = Error;

    fn try_from(data: &[u8]) -> Result<Self> {
        serde_cbor::from_slice(data).map_err(|e| {
            Error::Argument(format!("Failed to parse Card from bytes: {}", e))
        })
    }
}

impl From<&Card> for Vec<u8> {
    fn from(card: &Card) -> Self {
        serde_cbor::to_vec(card).unwrap()
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

    properties: Map<String, Value>,
}

impl Service {
    pub(crate) fn new(id: String,
        service_type: String,
        endpoint: String,
        properties: Map<String, Value >
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
    where T: Serialize + DeserializeOwned
    {
        self.properties.iter().filter_map(|(k, v)| {
                serde_json::from_value(v.clone())
                    .ok()
                    .map(|value| (k.as_str(), value))
            }).collect()
    }

    pub fn property<T>(&self, key: &str) -> Option<T>
    where T: Serialize + DeserializeOwned
    {
        self.properties.get(key)
            .and_then(|v| serde_json::from_value(v.clone()).ok())
    }
}

impl PartialEq<Self> for Service {
    fn eq(&self, other: &Service) -> bool {
        self.id == other.id &&
        self.service_type == other.service_type &&
        self.endpoint == other.endpoint
    }
}

impl Hash for Service {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.id.hash(state);
        self.service_type.hash(state);
        self.endpoint.hash(state);
    }
}
