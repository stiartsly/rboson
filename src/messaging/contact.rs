use std::time::SystemTime;
use serde::{Serialize};

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
};

#[derive(Clone, Copy)]
pub enum ContactType {
    Unknown = 0,
    Contact = 1,
    Group   = 2,
}

#[allow(dead_code)]
pub struct ContactBuilder {
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
    pub fn new(id: &Id) -> Self {
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

    pub fn with_home_peerid(&mut self, peer_id:&Id) -> &mut Self {
        self.home_peerid = Some(peer_id.clone());
        self
    }

    pub fn with_type(&mut self, _type: ContactType) -> &mut Self {
        self._type = _type;
        self
    }

    pub fn with_session_key(&mut self, key: &[u8]) -> &mut Self {
        self.session_key = Some(key.to_vec());
        self
    }

    pub fn with_remark(&mut self, remark: &str) -> &mut Self {
        self.remark = Some(remark.to_string());
        self
    }

    pub fn with_tags(&mut self, tags: &str) -> &mut Self {
        self.tags = Some(tags.to_string());
        self
    }

    pub fn with_muted(&mut self, muted: bool) -> &mut Self {
        self.muted = muted;
        self
    }

    pub fn with_blocked(&mut self, blocked: bool) -> &mut Self {
        self.blocked = blocked;
        self
    }

    pub fn with_deleted(&mut self, deleted: bool) -> &mut Self {
        self.deleted = deleted;
        self
    }

    pub fn with_created(&mut self, created: SystemTime) -> &mut Self {
        self.created = created;
        self
    }

    pub fn with_last_modified(&mut self, modified: SystemTime) -> &mut Self {
        self.last_modified = modified;
        self
    }

    pub fn with_revision(&mut self, revision: i32) -> &mut Self {
        self.revision = revision;
        self
    }

    pub fn with_name(&mut self, name: &str) -> &mut Self {
        self.name = Some(name.to_string());
        self
    }

    pub fn with_avatar(&mut self, avatar: bool) -> &mut Self {
        self.avatar = avatar;
        self
    }

    pub fn with_notice(&mut self, notice: &str) -> &mut Self {
        self.notice = Some(notice.to_string());
        self
    }

    pub fn with_owner(&mut self, owner: &Id) -> &mut Self {
        self.owner = Some(owner.clone());
        self
    }

    pub fn with_permission(&mut self, permission: &Permission) -> &mut Self {
        self.permission = Some(permission.clone());
        self
    }

    pub fn check_valid(&self) -> bool {
        false
    }

    pub fn build(&mut self) -> Result<Contact> {
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

#[derive(Debug, Serialize)]
pub struct Contact {
    #[serde(rename = "id")]
    id:             Id,

    #[serde(rename = "home_peerid")]
    home_peerid:    Id,

    #[serde(skip)]
    session_keypair : Option<signature::KeyPair>,
    #[serde(skip)]
    encryption_keypair: Option<cryptobox::KeyPair>,

    name:           String,

    avatar:         bool,

    remark:         Option<String>,
    tags:           Option<String>,

    muted:          bool,
    blocked:        bool,
    deleted:        bool,

    created:       SystemTime,
    last_modified: SystemTime,
    modified: bool,

    revision: i32,

    display_name:   Option<String>,
}

#[allow(unused)]
impl Contact {
    pub(crate) fn new(b: &mut ContactBuilder) -> Self {
        Self {
            id:         std::mem::take(&mut b.id),
            home_peerid: std::mem::take(&mut b.home_peerid).unwrap_or_default(),
            name:       b.name.take().unwrap_or_default(),
            avatar:     b.avatar,
            remark:     b.remark.take(),
            tags:       b.tags.take(),
            muted:      b.muted,
            blocked:    b.blocked,
            deleted:    b.deleted,
            created:    b.created.clone(),
            last_modified:  b.last_modified.clone(),
            modified:   false,
            revision:   b.revision,

            display_name: None,

            session_keypair: None,
            encryption_keypair: None,
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

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn set_name(&mut self, name: &str) {
        if self.name.is_empty() {
            return;
        }
        self.name = name.to_string();
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
            return;
        }
        self.remark = Some(remark.to_string());
    }

    pub fn tags(&self) -> Option<&str> {
        self.tags.as_ref().map(|v| v.as_str())
    }

    pub fn set_tags(&mut self, tags: &str) {
        if tags.is_empty() {
            self.tags = None;
            return;
        }
        self.tags = Some(tags.to_string());
    }

    pub fn is_muted(&self) -> bool {
        self.muted
    }

    pub fn set_muted(&mut self, muted: bool) {
        self.muted = muted;
    }

    pub fn is_blocked(&self) -> bool {
        self.blocked
    }

    pub fn set_blocked(&mut self, blocked: bool) {
        self.blocked = blocked;
    }

    pub fn is_delted(&self) -> bool {
        self.deleted
    }

    pub fn set_deleted(&mut self, deleted: bool) {
        self.deleted = deleted;
    }

    pub fn revision(&self) -> i32 {
        self.revision
    }

    pub fn increment_revision(&mut self) {
        self.revision += 1;
    }

    pub fn created(&self) -> SystemTime {
        self.created.clone()
    }

    pub fn last_modified(&self) -> SystemTime {
        self.last_modified.clone()
    }

    pub fn is_modified(&self) -> bool {
        self.modified
    }

    pub fn display_name(&self) -> String {
        unimplemented!()
    }

    fn init_session_key(&mut self, private_key: &[u8]) -> Result<()> {
        // TODO more.

        let session_keypair = KeyPair::try_from(private_key)?;
        let cryptobox_keypair = cryptobox::KeyPair::from(&session_keypair);
        self.session_keypair = Some(session_keypair);
        self.encryption_keypair = Some(cryptobox_keypair);
        Ok(())
    }

    pub(crate) fn set_session_key(&mut self, private_key: &[u8]) -> Result<()> {
        self.init_session_key(private_key)?;
        self.touch();
        Ok(())
    }

    fn touch(&mut self) {
        unimplemented!()
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
            CryptoContext::from_cryptobox(id, v)
        )
    }
}
