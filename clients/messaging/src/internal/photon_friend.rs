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

use std::{
    cell::RefCell,
    fmt,
    hash::{Hash, Hasher},
    rc::Rc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use boson_api::{
    CryptoContext,
    Id,
    Result,
};

use crate::{
    contact::{
        ContactType,
        ContactHandler,
        ContactEditorHandler
    },
    errors::EncodingError,
    internal::{
        OpaqueContact,
        PhotonContact,
    },
};

const FRIEND_HASH_SEED: i32 = 0x6030_AF91;

/// A friend contact.
///
/// This mirrors the Java `Friend` implementation: it is a contact of type
/// `Friend` with a mandatory session key.
#[derive(Debug, Clone, Eq)]
pub(crate) struct PhotonFriend {
    contact: PhotonContact,
}

impl PhotonFriend {
    pub(crate) fn new(
        id: Id,
        session_key: Vec<u8>,
        name: Option<String>,
        remark: Option<String>,
        tags: Option<String>,
        muted: bool,
        blocked: bool,
        created_at: i64,
        updated_at: i64,
        revision: i32,
    ) -> Self {
        Self {
            contact: PhotonContact {
                id,
                contact_type: ContactType::Friend as i32,
                session_key: Some(session_key),
                name,
                remark,
                tags,
                muted,
                blocked,
                created_at,
                updated_at,
                revision,
                channel: None,
            }
        }
    }

    /// Creates a friend with only a session key and optional remark.
    pub(crate) fn with_remark(
        id: Id,
        session_key: Vec<u8>,
        remark: Option<String>,
    ) -> Self {
        let created_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_millis() as i64)
            .unwrap_or(0);

        Self::new(
            id,
            session_key,
            None,
            remark,
            None,
            false,
            false,
            created_at,
            created_at,
            0,
        )
    }

    pub(crate) fn contact(&self) -> &PhotonContact {
        &self.contact
    }

    pub(crate) fn set_revision(&mut self, revision: i32) {
        self.contact.revision = revision;
    }

    pub(crate) fn contact_type(&self) -> ContactType {
        ContactType::Friend
    }

    pub(crate) fn hash_code(&self) -> i32 {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        self.contact.id.hash(&mut hasher);
        FRIEND_HASH_SEED.wrapping_add(hasher.finish() as i32)
    }

    /// Encrypts the serialized friend document for contact synchronization.
    pub(crate) fn to_opaque(&self, context: Rc<RefCell<CryptoContext>>) -> Result<OpaqueContact> {
        let data = serde_cbor::to_vec(&self.contact)
            .map_err(|error| EncodingError::new(format!("Serialization error: {error}")))?;
        let encrypted = context
            .borrow_mut()
            .encrypt_into(data.as_slice())
            .map_err(|error| EncodingError::new(format!("Encrypting friend failed: {error}")))?;
        Ok(self.contact.to_opaque(encrypted))
    }
}

impl From<PhotonContact> for PhotonFriend {
    fn from(contact: PhotonContact) -> Self {
        Self { contact }
    }
}


impl ContactHandler for PhotonFriend {
    fn id(&self) -> &Id {
        &self.contact.id
    }

    fn contact_type(&self) -> ContactType {
        ContactType::Friend
    }

    fn name(&self) -> Option<&str> {
        self.contact.name.as_deref()
    }

    fn remark(&self) -> Option<&str> {
        self.contact.remark.as_deref()
    }

    fn tags(&self) -> Option<&str> {
        self.contact.tags.as_deref()
    }

    fn is_muted(&self) -> bool {
        self.contact.muted
    }

    fn is_blocked(&self) -> bool {
        self.contact.blocked
    }

    fn created_at(&self) -> i64 {
        self.contact.created_at
    }

    fn updated_at(&self) -> i64 {
        self.contact.updated_at
    }

    fn revision(&self) -> i32 {
        self.contact.revision
    }

    fn edit(&self) -> Box<dyn ContactEditorHandler> {
        Box::new(PhotonFriendEditor(self.clone()))
    }

    fn clone_box(&self) -> Box<dyn ContactHandler> {
        Box::new(self.clone())
    }

    fn set_revision(&mut self, revision: i32) {
        self.contact.revision = revision;
    }
}

struct PhotonFriendEditor(PhotonFriend);

impl ContactEditorHandler for PhotonFriendEditor {
    fn set_name(mut self: Box<Self>, name: Option<String>) -> Box<dyn ContactEditorHandler> {
        self.0.contact.name = name;
        self
    }

    fn set_remark(mut self: Box<Self>, remark: Option<String>) -> Box<dyn ContactEditorHandler> {
        self.0.contact.remark = remark;
        self
    }

    fn set_tags(mut self: Box<Self>, tags: Option<String>) -> Box<dyn ContactEditorHandler> {
        self.0.contact.tags = tags;
        self
    }

    fn set_muted(mut self: Box<Self>, muted: bool) -> Box<dyn ContactEditorHandler> {
        self.0.contact.muted = muted;
        self
    }

    fn set_blocked(mut self: Box<Self>, blocked: bool) -> Box<dyn ContactEditorHandler> {
        self.0.contact.blocked = blocked;
        self
    }

    fn set_session_key(mut self: Box<Self>, session_key: Option<Vec<u8>>) -> Box<dyn ContactEditorHandler> {
        self.0.contact.session_key = session_key;
        self
    }

    fn set_updated_at(mut self: Box<Self>, updated_at: i64) -> Box<dyn ContactEditorHandler> {
        self.0.contact.updated_at = updated_at;
        self
    }

    fn set_revision(mut self: Box<Self>, revision: i32) -> Box<dyn ContactEditorHandler> {
        self.0.contact.revision = revision;
        self
    }

    fn build(mut self: Box<Self>) -> Box<dyn ContactHandler> {
        self.0.contact.updated_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_millis() as i64)
            .unwrap_or(0);
        Box::new(self.0) as Box<dyn ContactHandler>
    }
}

impl PartialEq for PhotonFriend {
    fn eq(&self, other: &Self) -> bool {
        self.contact.id == other.contact.id
    }
}

impl Hash for PhotonFriend {
    fn hash<H: Hasher>(&self, state: &mut H) {
        FRIEND_HASH_SEED.hash(state);
        self.contact.id.hash(state);
    }
}

impl fmt::Display for PhotonFriend {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Friend: {}", self.contact.id)?;

        if let Some(name) = &self.contact.name {
            write!(f, ", name={name}")?;
        }
        if let Some(remark) = &self.contact.remark {
            write!(f, ", remark={remark}")?;
        }
        if let Some(tags) = &self.contact.tags {
            write!(f, ", tags={tags}")?;
        }

        if self.contact.muted {
            write!(f, ", muted")?;
        }
        if self.contact.blocked {
            write!(f, ", blocked")?;
        }

        write!(
            f,
            ", createdAt={:?} updatedAt={:?} revision={}]",
            instant_from_millis(self.contact.created_at),
            instant_from_millis(self.contact.updated_at),
            self.contact.revision,
        )
    }
}

fn instant_from_millis(millis: i64) -> SystemTime {
    if millis >= 0 {
        UNIX_EPOCH + Duration::from_millis(millis as u64)
    } else {
        UNIX_EPOCH - Duration::from_millis(millis.unsigned_abs())
    }
}
