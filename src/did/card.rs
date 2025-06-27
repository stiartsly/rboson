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
    error::{Error, Result},
    signature,
    core::crypto_identity::CryptoIdentity,
};

use super::{
    Credential,
    CardBuilder
};

const DEFAULT_PROFILE_CREDENTIAL_ID     : &str = "profile";
const DEFAULT_PROFILE_CREDENTIAL_TYPE   : &str = "BosonProfile";
const DEFAULT_HOME_NODE_SERVICE_ID      : &str = "homeNode";
const DEFAULT_HOME_NODE_SERVICE_TYPE    : &str = "BosonHomeNode";

#[derive(Debug, Clone, Serialize, Deserialize, Eq)]
pub struct Card {
    #[serde(rename = "id")]
    id: Id,

    #[serde(rename = "c", skip_serializing_if = "Vec::is_empty")]
    credentials: Vec<Credential>,

    #[serde(rename = "s", skip_serializing_if = "Vec::is_empty")]
    services: Vec<Service>,

    #[serde(rename = "sat")]
    signed_at: u64,

    #[serde(rename = "sig")]
    signature: Vec<u8>
}

impl Card {
    #[allow(unused)]
    #[cfg(test)]
    pub(crate) fn new(
        id: Id,
        credentials: Vec<Credential>,
        services: Vec<Service>,
        signed_at: SystemTime,
        signature: Vec<u8>
    ) -> Self {
        Self {
            id,
            credentials,
            services,
            signed_at: as_secs!(signed_at) as u64,
            signature,
        }
    }

    pub(crate) fn unsigned(
        id: Id,
        credentials: Vec<Credential>,
        services: Vec<Service>
    ) -> Self {
        Self {
            id,
            credentials,
            services,
            signed_at: 0,
            signature: vec![0u8;0],
        }
    }

    pub(crate) fn signed(
        mut unsigned: Self,
        signed_at: Option<SystemTime>,
        signature: Option<Vec<u8>>
    ) -> Self {
        unsigned.signed_at = signed_at.map(|v|as_secs!(v)).unwrap_or(0);
        unsigned.signature = signature.unwrap_or_else(|| vec![0u8; 0]);
        unsigned
    }

    pub fn id(&self) -> &Id {
        &self.id
    }

    pub fn credentials(&self) -> Vec<&Credential> {
        self.credentials.iter().collect()
    }

    pub fn credentials_with_type(&self, credential_type: &str) -> Option<&Credential> {
        self.credentials.iter()
            .find(|c| c.types().contains(&credential_type))
    }

    pub fn profile_credential(&self) -> Option<&Credential> {
        self.credentials.iter()
            .find(|cred|
                cred.id() == DEFAULT_PROFILE_CREDENTIAL_ID &&
                cred.types().contains(&DEFAULT_PROFILE_CREDENTIAL_TYPE)
            )
    }

    pub fn services(&self) -> &[Service] {
        &self.services
    }

    pub fn service_with_type(&self, service_type: &str) -> Option<&Service> {
        self.services.iter()
            .find(|s| s.service_type() == service_type)
    }

    pub fn services_with_id(&self, id: &str) -> Option<&Service> {
        self.services.iter()
            .find(|s| s.id() == id)
    }

    pub fn  homenode_service(&self) -> Option<&Service> {
        self.services.iter()
            .find(|s|
                s.id() == DEFAULT_HOME_NODE_SERVICE_ID &&
                s.service_type() == DEFAULT_HOME_NODE_SERVICE_TYPE
            )
    }

    pub fn signed_at(&self) -> SystemTime {
        SystemTime::UNIX_EPOCH + Duration::from_secs(self.signed_at)
    }

    pub fn signature(&self) -> &[u8] {
        &self.signature
    }

    pub fn is_geniune(&self) -> bool {
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
        match self.is_geniune() {
            true => Ok(()),
            false => Err(Error::Signature("Card signature is not valid".into())),
        }
    }

    pub(crate) fn to_sign_data(&self) -> Vec<u8> {
        if self.signature.is_empty() {
            let card = Self::signed(self.clone(), None, None);
            Vec::from(&card)
        } else {
            Vec::from(self)
        }
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
        serde_json::from_slice(data).map_err(|e| {
            Error::Argument(format!("Failed to parse Card from bytes: {}", e))
        })
    }
}

impl From<&Card> for String {
    fn from(card: &Card) -> Self {
        serde_json::to_string(&card).unwrap()
    }
}

impl From<&Card> for Vec<u8> {
    fn from(card: &Card) -> Self {
        serde_json::to_vec(card).unwrap()
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

    pub fn properties<T>(&self) -> HashMap<&str, T>
    where T: Serialize + DeserializeOwned + Clone {
        self.properties.iter()
            .filter_map(|(k, v)| {
                serde_json::from_value(v.clone())
                    .ok()
                    .map(|value| (k.as_str(), value))
            })
            .collect()
    }

    pub fn property<T>(&self, key: &str) -> Option<T>
    where T: Serialize + DeserializeOwned + Clone {
        self.properties.get(key)
            .and_then(|value| serde_json::from_value(value.clone()).ok())
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
