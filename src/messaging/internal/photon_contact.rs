use std::cmp::Ordering;
use serde::{Deserialize, Serialize};
use serde_cbor::Value;
use crate::{CryptoIdentity, Id, Identity, errors::Result};
use crate::messaging::{
    contact::{Contact, ContactEditor, ContactType},
    errors::{AuthenticationError, EncodingError},
};
use super::contact_editor::PhotonContactEditor;

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct OpaqueContact {
    pub(crate) id: Id,
    #[serde(rename = "v")]
    pub(crate) revision: i32,
    #[serde(rename = "d")]
    pub(crate) data: Value,
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct ContactData {
    pub(crate) id: Id,
    #[serde(rename = "t")]
    pub(crate) kind: u8,
    #[serde(rename = "sk", default)]
    pub(crate) session_key: Option<Value>,
    #[serde(rename = "n", default)]
    pub(crate) name: Option<String>,
    #[serde(rename = "r", default)]
    pub(crate) remark: Option<String>,
    #[serde(rename = "ts", default)]
    pub(crate) tags: Option<String>,
    #[serde(rename = "m", default)]
    pub(crate) muted: bool,
    #[serde(rename = "b", default)]
    pub(crate) blocked: bool,
    #[serde(rename = "c")]
    pub(crate) created_at: i64,
    #[serde(rename = "u")]
    pub(crate) updated_at: i64,
}

impl ContactData {
    pub(crate) fn contact(&self, revision: i32) -> Result<PhotonContact> {
        let contact_type = match self.kind {
            0 => ContactType::Auto,
            1 => ContactType::Friend,
            2 => ContactType::Channel,
            _ => return Err(EncodingError::new("Unknown contact type")),
        };
        let contact = PhotonContact {
            id: self.id, contact_type,
            name: self.name.clone(), remark: self.remark.clone(), tags: self.tags.clone(),
            muted: self.muted, blocked: self.blocked,
            created_at: self.created_at, updated_at: self.updated_at, revision,
        };
        contact.validate()?;
        Ok(contact)
    }

    pub(crate) fn session(&self, identity: &CryptoIdentity) -> Result<Option<CryptoIdentity>> {
        match &self.session_key {
            Some(Value::Bytes(encrypted)) => {
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
        let bytes = serde_cbor::to_vec(self)
            .map_err(|error| EncodingError::new(format!("Encoding contact failed: {error}")))?;
        let encrypted = identity.encrypt_into(identity.id(), &bytes)
            .map_err(|error| AuthenticationError::new(format!("Encrypting contact failed: {error}")))?;
        Ok(OpaqueContact { id: self.id, revision, data: Value::Bytes(encrypted) })
    }
}

impl OpaqueContact {
    pub(crate) fn decode(&self, identity: &CryptoIdentity) -> Result<(ContactData, Vec<u8>)> {
        if self.revision < 0 {
            return Err(EncodingError::new("Contact revision is negative"));
        }
        let Value::Bytes(encrypted) = &self.data else {
            return Err(EncodingError::new("Opaque contact data is not binary"));
        };
        let plaintext = identity.decrypt_into(identity.id(), encrypted)
            .map_err(|error| AuthenticationError::new(format!("Decrypting contact failed: {error}")))?;
        let data: ContactData = serde_cbor::from_slice(&plaintext)
            .map_err(|error| EncodingError::new(format!("Malformed contact data: {error}")))?;
        if data.id != self.id {
            return Err(EncodingError::new("Contact ID mismatch"));
        }
        data.contact(self.revision)?;
        Ok((data, plaintext))
    }
}

#[derive(Debug, Clone)]
pub struct PhotonContact {
    pub id: Id,
    pub contact_type: ContactType,
    pub name: Option<String>,
    pub remark: Option<String>,
    pub tags: Option<String>,
    pub muted: bool,
    pub blocked: bool,
    pub created_at: i64,
    pub updated_at: i64,
    pub revision: i32,
}

impl PhotonContact {
    pub(crate) fn validate(&self) -> Result<()> {
        if self.created_at < 0 || self.updated_at < self.created_at || self.revision < 0 {
            return Err(EncodingError::new("Invalid contact timestamps or revision"));
        }
        Ok(())
    }

    pub(crate) fn compare(&self, other: &dyn Contact) -> Ordering {
        fn optional(left: Option<&str>, right: Option<&str>) -> Ordering {
            match (left, right) {
                (Some(left), Some(right)) => left.cmp(right),
                (Some(_), None) => Ordering::Less,
                (None, Some(_)) => Ordering::Greater,
                (None, None) => Ordering::Equal,
            }
        }
        optional(self.remark(), other.remark())
            .then_with(|| optional(self.name(), other.name()))
            .then_with(|| self.id().cmp(other.id()))
    }
}

impl From<&dyn Contact> for PhotonContact {
    fn from(contact: &dyn Contact) -> Self {
        Self {
            id: contact.id().clone(),
            contact_type: contact.contact_type(),
            name: contact.name().map(|s| s.to_string()),
            remark: contact.remark().map(|s| s.to_string()),
            tags: contact.tags().map(|s| s.to_string()),
            muted: contact.is_muted(),
            blocked: contact.is_blocked(),
            created_at: contact.created_at(),
            updated_at: contact.updated_at(),
            revision: contact.revision(),
        }
    }
}

impl PartialEq for PhotonContact {
    fn eq(&self, other: &Self) -> bool { self.id == other.id }
}

impl Eq for PhotonContact {}

impl std::hash::Hash for PhotonContact {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) { self.id.hash(state); }
}

impl Contact for PhotonContact {
    fn id(&self) -> &Id {
        &self.id
    }

    fn contact_type(&self) -> ContactType {
        self.contact_type
    }

    fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    fn remark(&self) -> Option<&str> {
        self.remark.as_deref()
    }

    fn tags(&self) -> Option<&str> {
        self.tags.as_deref()
    }

    fn is_muted(&self) -> bool {
        self.muted
    }

    fn is_blocked(&self) -> bool {
        self.blocked
    }

    fn created_at(&self) -> i64 {
        self.created_at
    }

    fn updated_at(&self) -> i64 {
        self.updated_at
    }

    fn revision(&self) -> i32 {
        self.revision
    }

    fn edit(&self) -> Box<dyn ContactEditor> {
        Box::new(PhotonContactEditor {
            contact: self.clone(),
        })
    }
}
