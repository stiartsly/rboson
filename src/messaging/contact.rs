use std::fmt;
use std::time::SystemTime;
use std::sync::{Arc, Mutex};
use serde::{Serialize, Deserialize};

use crate::{
    Id,
    Error,
    Identity,
    signature::{self, KeyPair},
    cryptobox::{self, CryptoBox, Nonce},
    core::Result,
    core::CryptoContext,
};

use super::{
    // channel::Permission,
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
    #[serde(skip_serializing_if = "super::is_default")]
    remark          : String,
    #[serde(rename="ts")]
    #[serde(skip_serializing_if = "super::is_default")]
    tags            : String,
    #[serde(rename="d")]
    #[serde(skip_serializing_if = "super::is_default")]
    muted           : bool,
    #[serde(rename="b")]
    #[serde(skip_serializing_if = "super::is_default")]
    blocked         : bool,
    #[serde(rename="c")]
    #[serde(skip_serializing_if = "super::is_default")]
    created         : u64,
    #[serde(rename="m")]
    #[serde(skip_serializing_if = "super::is_default")]
    last_modified   : u64,
    #[serde(rename="e")]
    #[serde(skip_serializing_if = "super::is_default")]
    deleted         : bool,
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
    pub(crate) fn new1(
        id: Id,
        home_peerid: Option<Id>,
        session_key: Vec<u8>,
        remark: Option<String>
    ) -> Result<Self> {
        unimplemented!()
    }

    /*
    pub(crate) fn new(b: &mut ContactBuilder) -> Self {
        Self {
            id          : b.id.clone(),
            auto        : false,
            home_peerid : b.home_peerid.clone().unwrap(),
            name        : b.name.clone(),
            avatar      : b.avatar,
            remark      : b.remark.take().unwrap_or_default(),
            tags        : b.tags.take().unwrap_or_default(),
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

    pub(crate) fn new(id: Id, home_peerid: Id) -> Self {
        Self {
            id,
            auto: true,
            session_keypair: None,
            encryption_keypair: None,
            session_id: None,
            home_peerid,
            name: None,
            avatar: false,
            remark: String::new(),
            tags: String::new(),
            muted: false,
            blocked: false,
            created: SystemTime::now().duration_since(SystemTime::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            last_modified: SystemTime::now().duration_since(SystemTime::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            deleted: false,
            revision: 1,
            modified: false,
            last_updated: None,
            display_name: None,
        }
    }
    */

    pub fn id(&self) -> &Id {
        &self.id
    }

    pub fn home_peerid(&self) -> &Id {
        &self.home_peerid
    }

    pub fn is_auto(&self) -> bool {
        self.auto
    }

    pub fn set_auto(&mut self, auto: bool) {
        self.auto = auto;
    }

    pub fn has_session_key(&self) -> bool {
        self.session_keypair.is_some()
    }

    pub fn name(&self) -> Option<&str> {
        self.name.as_ref().map(|v| v.as_str())
    }

    pub(crate) fn set_name(&mut self, name: &str) {
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

    pub fn remark(&self) -> &str {
        self.remark.as_str()
    }

    pub fn set_remark(&mut self, remark: &str) {
        self.remark = match remark.is_empty() {
            true => String::new(),
            false => remark.to_string()
        };
        self.display_name = None;
        self.touch();
    }

    pub fn tags(&self) -> &str {
        self.tags.as_str()
    }

    pub fn set_tags(&mut self, tags: &str) {
        self.tags = match tags.is_empty() {
            true => String::new(),
            false => tags.to_string()
        };
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

    pub fn is_deleted(&self) -> bool {
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
        self.session_id = Some(Id::from(session_keypair.public_key()));
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

    pub(crate) fn rx_crypto_context(&self) -> Result<&CryptoContext> {
        unimplemented!()
    }

    pub(crate) fn tx_crypto_context(&self) -> Result<&CryptoContext> {
        unimplemented!()
    }
}

impl PartialEq for Contact {
    fn eq(&self, _other: &Self) -> bool {
        unimplemented!()
    }
}

impl Identity for Contact {
    type IdentityObject = Contact;

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
            CryptoContext::new(id.clone(), v)
        )
    }
}

impl fmt::Display for Contact {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Contact: {}[", self.id.to_base58())?;
        unimplemented!()
    }
}
