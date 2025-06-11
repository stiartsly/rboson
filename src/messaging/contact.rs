use std::fmt;
use std::time::SystemTime;
use std::sync::{Arc, Mutex};
use serde::{Serialize, Deserialize};

use crate::{
    Id,
    Error,
    error::Result,
    Identity,
    core::crypto_context::CryptoContext,
    signature,
    signature::KeyPair,
    cryptobox,
    cryptobox::{CryptoBox, Nonce},
};

use super::{
    channel::Permission,
    profile::Profile,
};

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(unused)]
pub enum ContactType {
    Unknown = 0,
    Contact = 1,
    Group   = 2,
}

#[allow(dead_code)]
pub(crate) struct ContactBuilder {
    id          : Id,
    home_peerid : Option<Id>,
    _type       : ContactType,

    session_key : Option<Vec<u8>>,

    remark      : Option<String>,
    tags        : Option<String>,

    muted       : bool,
    blocked     : bool,
    created     : SystemTime,
    last_modified   : SystemTime,

    deleted     : bool,
    revision    : i32,

    name        : Option<String>,
    notice      : Option<String>,
    avatar      : bool,

    owner       : Option<Id>,
    permission  : Option<Permission>,
}

#[allow(dead_code)]
impl ContactBuilder {
    pub(crate) fn new(id: &Id) -> Self {
        Self {
            id          : id.clone(),
            home_peerid : None,
            _type       : ContactType::Unknown,
            session_key : None,
            remark      : None,
            tags        : None,
            muted       : false,
            blocked     : false,
            created     : SystemTime::UNIX_EPOCH,
            last_modified   : SystemTime::UNIX_EPOCH,
            deleted     : false,
            revision    : 0,
            name        : None,
            notice      : None,
            avatar      : false,
            owner       : None,
            permission  : None,
        }
    }

    pub(crate) fn with_home_peerid(&mut self, peer_id:&Id) -> &mut Self {
        self.home_peerid = Some(peer_id.clone());
        self
    }

    pub(crate) fn with_type(&mut self, _type: ContactType) -> &mut Self {
        self._type = _type;
        self
    }

    pub(crate) fn with_session_key(&mut self, key: &[u8]) -> &mut Self {
        self.session_key = Some(key.to_vec());
        self
    }

    pub(crate) fn with_remark(&mut self, remark: &str) -> &mut Self {
        self.remark = Some(remark.to_string());
        self
    }

    pub(crate) fn with_tags(&mut self, tags: &str) -> &mut Self {
        self.tags = Some(tags.to_string());
        self
    }

    pub(crate) fn with_muted(&mut self, muted: bool) -> &mut Self {
        self.muted = muted;
        self
    }

    pub(crate) fn with_blocked(&mut self, blocked: bool) -> &mut Self {
        self.blocked = blocked;
        self
    }

    pub(crate) fn with_deleted(&mut self, deleted: bool) -> &mut Self {
        self.deleted = deleted;
        self
    }

    pub(crate) fn with_created(&mut self, created: SystemTime) -> &mut Self {
        self.created = created;
        self
    }

    pub(crate) fn with_last_modified(&mut self, modified: SystemTime) -> &mut Self {
        self.last_modified = modified;
        self
    }

    pub(crate) fn with_revision(&mut self, revision: i32) -> &mut Self {
        self.revision = revision;
        self
    }

    pub(crate) fn with_name(&mut self, name: &str) -> &mut Self {
        self.name = Some(name.to_string());
        self
    }

    pub(crate) fn with_avatar(&mut self, avatar: bool) -> &mut Self {
        self.avatar = avatar;
        self
    }

    pub(crate) fn with_notice(&mut self, notice: &str) -> &mut Self {
        self.notice = Some(notice.to_string());
        self
    }

    pub(crate) fn with_owner(&mut self, owner: &Id) -> &mut Self {
        self.owner = Some(owner.clone());
        self
    }

    pub(crate) fn with_permission(&mut self, permission: &Permission) -> &mut Self {
        self.permission = Some(permission.clone());
        self
    }

    pub(crate) fn check_valid(&self) -> bool {
        false
    }

    pub(crate) fn build(&mut self) -> Result<Contact> {
        if self.check_valid() {
            return Err(Error::Argument(format!("Invalid contact")));
        }

        match self._type {
            ContactType::Unknown => return Err(Error::Argument(format!("Invalid contact type"))),
            ContactType::Contact => Ok(Contact::new(self)),
            ContactType::Group => Ok(Contact::new_group(self))
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Contact {
    #[serde(rename = "id")]
    id              : Id,

    #[serde(skip)]
    auto            : bool,

    #[serde(skip)]  // internal use only.
    session_keypair : Option<signature::KeyPair>,
    #[serde(skip)]  // internal use only.
    encryption_keypair  : Option<cryptobox::KeyPair>,
    #[serde(skip)]  //
    session_id      : Option<Id>,

    #[serde(skip)]
    home_peerid     : Id,
    #[serde(skip)]  // padding expicitly.
    name            : Option<String>,
    #[serde(skip)]  // padding expicitly.
    avatar          : bool,

    #[serde(rename="r")]
    #[serde(skip_serializing_if = "super::is_none_or_empty_string")]
    remark          : Option<String>,
    #[serde(rename="ts")]
    #[serde(skip_serializing_if = "super::is_none_or_empty_string")]
    tags            : Option<String>,
    #[serde(rename="d")]
    #[serde(skip_serializing_if = "super::is_false")]
    muted           : bool,
    #[serde(rename="b")]
    #[serde(skip_serializing_if = "super::is_false")]
    blocked         : bool,
    #[serde(rename="e")]
    #[serde(skip_serializing_if = "super::is_false")]
    deleted         : bool,
    #[serde(rename="c")]
    #[serde(skip_serializing_if = "super::is_zero")]
    created         : u64,
    #[serde(rename="m")]
    #[serde(skip_serializing_if = "super::is_zero")]
    last_modified   : u64,
    #[serde(rename="v")]
    revision        : i32,

    #[serde(skip)]
    modified        : bool,
    #[serde(skip)]
    last_updated    : Option<SystemTime>,
    #[serde(skip)]
    display_name    : Option<String>,
}

#[allow(unused)]
impl Contact {
    pub(crate) fn new(b: &ContactBuilder) -> Self {
        Self {
            id          : b.id.clone(),
            auto        : false,
            home_peerid : b.home_peerid.clone().unwrap(),
            name        : b.name.clone(),
            avatar      : b.avatar,
            remark      : b.remark.clone(),
            tags        : b.tags.clone(),
            muted       : b.muted,
            blocked     : b.blocked,
            deleted     : b.deleted,
            created     : b.created.duration_since(SystemTime::UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_secs(),
            last_modified:b.last_modified.duration_since(SystemTime::UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_secs(),
            revision    : b.revision,
            modified    :   false,
            last_updated: None,
            display_name: None,

            session_keypair     : None,
            encryption_keypair  : None,
            session_id          : None,
        }
    }

    pub(crate) fn new_group(_b: &mut ContactBuilder) -> Self {
        unimplemented!()
    }

    pub fn id(&self) -> &Id {
        &self.id
    }

    pub fn home_peerid(&self) -> &Id {
        &self.home_peerid
    }

    pub fn name(&self) -> Option<&str> {
        self.name.as_ref().map(|v| v.as_str())
    }

    pub fn set_name(&mut self, name: &str) {
        self.name = Some(name.into());
        self.display_name = None;
        self.touch();
    }

    pub fn has_avatar(&self) -> bool {
        self.avatar
    }

    pub fn set_avatar(&mut self, avatar: bool) {
        self.avatar = avatar;
    }

    pub fn avatar_url(&self) -> Option<String> {
        match self.avatar {
            true => Some(format!("bmr://{}/{}", self.home_peerid, self.id)),
            false => None,
        }
    }

    pub fn remark(&self) -> Option<&str> {
        self.remark.as_ref().map(|v| v.as_str())
    }

    pub fn set_remark(&mut self, remark: &str) {
        if remark.is_empty() {
            self.remark = None;
        }else {
            self.remark = Some(remark.into());
        }
        self.display_name = None;
        self.touch();
    }

    pub fn tags(&self) -> Option<&str> {
        self.tags.as_ref().map(|v| v.as_str())
    }

    pub fn set_tags(&mut self, tags: &str) {
        if tags.is_empty() {
            self.tags = None;
        } else {
            self.tags = Some(tags.into());
        }
        self.display_name = None;
        self.touch();
    }

    pub fn is_muted(&self) -> bool {
        self.muted
    }

    pub fn set_muted(&mut self, muted: bool) {
        self.muted = muted;
        self.touch();
    }

    pub fn is_blocked(&self) -> bool {
        self.blocked
    }

    pub fn set_blocked(&mut self, blocked: bool) {
        self.blocked = blocked;
        self.touch();
    }

    pub fn is_delted(&self) -> bool {
        self.deleted
    }

    pub fn set_deleted(&mut self, deleted: bool) {
        self.deleted = deleted;
        self.touch();
    }

    pub fn revision(&self) -> i32 {
        self.revision
    }

    pub fn increment_revision(&mut self) {
        self.revision += 1;
    }

    pub fn created(&self) -> SystemTime {
        SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(self.created)
    }

    pub fn last_modified(&self) -> SystemTime {
        SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(self.last_modified)
    }

    pub fn is_modified(&self) -> bool {
        self.modified
    }

    pub(crate) fn udpated(&mut self) {
        self.last_updated = Some(SystemTime::now());
    }

    pub fn last_updated(&self) -> SystemTime{
        self.last_updated.unwrap_or(SystemTime::UNIX_EPOCH)
    }

    pub fn display_name(&self) -> String {
        unimplemented!()
    }

    pub fn update_profile(&mut self, profile: &Profile) {
        unimplemented!()
    }

    pub fn update_contact(&mut self, contact: &Contact) {
        unimplemented!()
    }

    fn get_self_encryption_context(&self) -> Arc<Mutex<Option<Box<CryptoContext>>>> {
        unimplemented!()
    }

    fn init_session_key(&mut self, private_key: &[u8]) -> Result<()> {
        if private_key.len() != signature::PrivateKey::BYTES + CryptoBox::MAC_BYTES + Nonce::BYTES {
            return Err(Error::Argument(format!("Invalid session key size")));
        }

        let ctx = self.get_self_encryption_context();
        let ctx = ctx.lock().unwrap();

        let Some(ctx) = ctx.as_ref() else {
            return Err(Error::State("No self encryption context".into()));
        };

        let private_key = ctx.decrypt_into(private_key).map_err(|e| {
            Error::Crypto(format!("Failed to decrypt session key: {}", e))
        })?;

        if private_key.len() != signature::PrivateKey::BYTES {
            return Err(Error::Crypto(format!("Invalid session key size")));
        }

        let session_keypair = KeyPair::try_from(private_key.as_slice())?;
        self.session_id = Some(Id::from(session_keypair.to_public_key()));
        self.encryption_keypair = Some(cryptobox::KeyPair::from(&session_keypair));
        self.session_keypair = Some(session_keypair);

        Ok(())
    }

    pub(crate) fn set_session_key(&mut self, private_key: &[u8]) -> Result<()> {
        self.init_session_key(private_key)?;
        self.touch();
        Ok(())
    }

    fn touch(&mut self) {
        if self.auto {
            self.auto = false;
        }
        if !self.modified {
            self.modified = true;
            self.increment_revision();
        }
        self.last_modified = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
    }
}

impl PartialEq for Contact {
    fn eq(&self, _other: &Self) -> bool {
        unimplemented!()
    }
}

impl Identity for Contact {
    fn id(&self) -> &Id {
        &self.id
    }

    fn sign(&self, data: &[u8], sig: &mut [u8]) -> Result<usize> {
        let Some(keypair) = self.session_keypair.as_ref() else {
            return Err(Error::State("Missing session keypair".into()));
        };
        signature::sign(data, sig, keypair.private_key())
    }

    fn sign_into(&self, data: &[u8]) -> Result<Vec<u8>> {
        let Some(keypair) = self.session_keypair.as_ref() else {
            return Err(Error::State("Missing session keypair".into()));
        };
        signature::sign_into(data, keypair.private_key())
    }

    fn verify(&self, data: &[u8], sig: &[u8]) -> Result<()> {
        let Some(keypair) = self.session_keypair.as_ref() else {
            return Err(Error::State("Missing session keypair".into()));
        };
        signature::verify(data,sig, keypair.public_key())
    }

    fn encrypt(&self, recipient: &Id, plain: &[u8], cipher: &mut [u8]) -> Result<usize> {
        let Some(keypair) = self.encryption_keypair.as_ref() else {
            return Err(Error::State("Missing cryptobox keypair".into()));
        };

        cryptobox::encrypt(
            plain,
            cipher,
            &Nonce::random(),
            &recipient.to_encryption_key(),
            keypair.private_key()
        )
    }

    fn decrypt(&self, sender: &Id, cipher: &[u8], plain: &mut [u8]) -> Result<usize> {
        let Some(keypair) = self.encryption_keypair.as_ref() else {
            return Err(Error::State("Missing cryptobox keypair".into()));
        };

        cryptobox::decrypt(
            cipher,
            plain,
            &sender.to_encryption_key(),
            keypair.private_key()
        )
    }

    fn encrypt_into(&self, recipient: &Id, plain: &[u8]) -> Result<Vec<u8>> {
        let Some(keypair) = self.encryption_keypair.as_ref() else {
            return Err(Error::State("Missing cryptobox keypair".into()));
        };

        cryptobox::encrypt_into(
            plain,
            &Nonce::random(),
            &recipient.to_encryption_key(),
            keypair.private_key()
        )
    }

    fn decrypt_into(&self, sender: &Id, cipher: &[u8]) -> Result<Vec<u8>> {
        let Some(keypair) = self.encryption_keypair.as_ref() else {
            return Err(Error::State("Missing cryptobox keypair".into()));
        };

        cryptobox::decrypt_into(
            cipher,
            &sender.to_encryption_key(),
            keypair.private_key()
        )
    }

    fn create_crypto_context(&self, id: &Id) -> Result<CryptoContext> {
        let Some(keypair) = self.encryption_keypair.as_ref() else {
            return Err(Error::State("Missing cryptobox keypair".into()));
        };

        CryptoBox::try_from((&id.to_encryption_key(), keypair.private_key())).map(|v|
            CryptoContext::new(id, v)
        )
    }
}

impl fmt::Display for Contact {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Contact: {}[", self.id.to_base58())?;
        unimplemented!()
    }
}
