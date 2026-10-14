use serde::{Deserialize, Serialize};
use serde_cbor::Value;
use crate::{CryptoIdentity, Id, Identity, errors::Result, utils};
use crate::messaging::{
    contact::{Contact, ContactOps},
    errors::{AuthenticationError, EncodingError},
};
use super::OpaqueContact;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ContactData {
    pub(crate) id: Id,
    #[serde(rename = "t")]
    pub(crate) kind: u8,

    #[serde(
        rename = "sk",
        default,
        skip_serializing_if="Option::is_none"
    )]
    session_key: Option<Vec<u8>>,

    #[serde(
        rename = "n",
        default,
        skip_serializing_if="Option::is_none"
    )]
    pub(crate) name: Option<String>,

    #[serde(
        rename = "r",
        default,
        skip_serializing_if="Option::is_none"
    )]
    pub(crate) remark: Option<String>,

    #[serde(
        rename = "ts",
        default,
        skip_serializing_if="Option::is_none"
    )]
    pub(crate) tags: Option<String>,

    #[serde(
        rename = "m",
        default,
        skip_serializing_if="utils::is_default"
    )]
    pub(crate) muted: bool,
    #[serde(
        rename = "b",
        default,
        skip_serializing_if="utils::is_default"
    )]
    pub(crate) blocked: bool,

    #[serde(rename = "c")]
    pub(crate) created_at: i64,

    #[serde(rename = "u")]
    pub(crate) updated_at: i64,

    #[serde(rename = "v")]
    pub(crate) revision: i32,
}

fn is_zero(value: &i32) -> bool { *value == 0 }

impl ContactData {
    pub(crate) fn new(id: Id, kind: u8, created_at: i64) -> Self {
        Self {
            id, kind, session_key: None, name: None, remark: None, tags: None,
            muted: false, blocked: false, created_at, updated_at: created_at, revision: 0,
        }
    }

    pub(crate) fn set_session_key(&mut self, session_key: Vec<u8>) {
        self.session_key = Some(session_key);
    }

    pub(crate) fn contact(&self, revision: i32) -> Result<Contact> {
        let mut data = self.clone();
        data.revision = revision;
        Contact::try_from(data)
    }

    pub(crate) fn session(&self, identity: &CryptoIdentity) -> Result<Option<CryptoIdentity>> {
        match &self.session_key {
            Some(encrypted) => {
                let key = identity.decrypt_into(identity.id(), encrypted)
                    .map_err(|error| AuthenticationError::new(format!("Decrypting contact session key failed: {error}")))?;
                Ok(Some(CryptoIdentity::try_from(key.as_slice())
                    .map_err(|error| AuthenticationError::new(format!("Invalid contact session key: {error}")))?))
            }
            None if self.kind == 0 => Ok(None),
            _ => Err(EncodingError::new("Contact session key is missing or malformed")),
        }
    }

    pub(crate) fn to_opaque(&self, revision: i32, identity: &CryptoIdentity) -> Result<OpaqueContact> {
        self.contact(revision)?;
        self.session(identity)?;
        let Value::Map(mut data) = serde_cbor::value::to_value(self)
            .map_err(|error| EncodingError::new(format!("Encoding contact failed: {error}")))? else {
            return Err(EncodingError::new("Contact data is not a map"));
        };
        if let Some(key) = &self.session_key {
            data.insert(Value::Text("sk".into()), Value::Bytes(key.clone()));
        }
        let bytes = serde_cbor::to_vec(&Value::Map(data))
            .map_err(|error| EncodingError::new(format!("Encoding contact failed: {error}")))?;
        let encrypted = identity.encrypt_into(identity.id(), &bytes)
            .map_err(|error| AuthenticationError::new(format!("Encrypting contact failed: {error}")))?;
        Ok(OpaqueContact::new(self.id, revision, Value::Bytes(encrypted)))
    }
}

impl From<Contact> for ContactData {
    fn from(contact: Contact) -> Self {
        Self {
            id: *contact.id(),
            kind: contact.contact_type() as u8,
            session_key: None,
            name: contact.name().map(ToString::to_string),
            remark: contact.remark().map(ToString::to_string),
            tags: contact.tags().map(ToString::to_string),
            muted: contact.is_muted(),
            blocked: contact.is_blocked(),
            created_at: contact.created_at(),
            updated_at: contact.updated_at(),
            revision: contact.revision(),
        }
    }
}
