use std::result;
use std::time::{Duration, SystemTime};
use std::sync::{Arc, Mutex};
use serde::{Serialize, Deserialize};
use serde::ser::{Serializer};
use serde::de::{Deserializer};

use crate::{
    as_secs,
    Id,
    Error,
    Identity,
    signature::{self, KeyPair},
    cryptobox::{self, CryptoBox, Nonce},
    core::Result,
    core::CryptoContext,
};

use super::{
    profile::Profile
};

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(unused)]
pub enum ContactType {
    Unknown = 0,
    Contact = 1,
    Group   = 2,
}

pub type Contact = GenericContact<()>;

#[derive(Debug, Clone)]
pub struct GenericContact<T> where T: Clone {
    id              : Id,
    auto            : bool,

    session_keypair : Option<signature::KeyPair>,
    encrypt_keypair : Option<cryptobox::KeyPair>,
    session_id      : Option<Id>,

    _rx_crypto_context: Option<Arc<Mutex<Box<CryptoContext>>>>,
    _tx_crypto_context: Option<Arc<Mutex<Box<CryptoContext>>>>,

    home_peerid     : Option<Id>,
    name            : Option<String>,
    avatar          : bool,

    remark          : Option<String>,
    tags            : Option<String>,
    muted           : bool,
    blocked         : bool,
    created         : u64,
    last_modified   : u64,
    deleted         : bool,
    revision        : i32,

    modified        : bool,
    last_updated    : Option<SystemTime>,
    display_name    : Option<String>,

    annex_data      : Option<T>,
}

#[allow(unused)]
impl<T> GenericContact<T> where T: Clone{
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

    */

    pub(crate) fn new(id: Id, home_peerid: Id, annex: T) -> Self {
        Self {
            id,
            auto            : true,
            session_keypair : None,
            encrypt_keypair : None,
            session_id      : None,
            home_peerid     : Some(home_peerid),
            name            : None,
            avatar          : false,
            remark          : Some(String::new()),
            tags            : Some(String::new()),
            muted           : false,
            blocked         : false,
            created         : as_secs!(SystemTime::now()),
            last_modified   : as_secs!(SystemTime::now()),
            deleted         : false,
            revision        : 1,
            modified        : false,
            last_updated    : None,
            display_name    : None,

            _rx_crypto_context: None,
            _tx_crypto_context: None,
            annex_data      : Some(annex),
        }
    }

    pub(crate) fn annex_mut(&mut self) -> &mut T {
        self.annex_data.as_mut().expect("Annex data is missing")
    }

    pub(crate) fn annex(&self) -> &T {
        self.annex_data.as_ref().expect("Annex data is missing")
    }

    pub fn id(&self) -> &Id {
        &self.id
    }

    pub fn home_peerid(&self) -> Option<&Id> {
        self.home_peerid.as_ref()
    }

    pub fn is_auto(&self) -> bool {
        self.auto
    }

    pub fn set_auto(&mut self, auto: bool) {
        self.auto = auto;
    }

    pub(crate) fn is_staled(&self) -> bool {
        // TODO: unimplemented!()
        true
    }

    pub fn has_session_key(&self) -> bool {
        self.session_keypair.is_some()
    }

    pub fn session_id(&self) -> Option<Id> {
        // TODO:
        unimplemented!()
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
        if !self.avatar {
            return None;
        }
        self.home_peerid()
            .map(|peerid| format!("bmr://{}/{}", peerid, self.id))
    }

    pub fn remark(&self) -> Option<&str> {
        self.remark.as_deref()
    }

    pub fn set_remark(&mut self, remark: &str) {
        self.remark = Some(match remark.is_empty() {
            true => String::new(),
            false => remark.to_string()
        });
        self.display_name = None;
        self.touch();
    }

    pub fn tags(&self) -> Option<&str> {
        self.tags.as_deref()
    }

    pub fn set_tags(&mut self, tags: &str) {
        self.tags = Some(match tags.is_empty() {
            true => String::new(),
            false => tags.to_string()
        });
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
        SystemTime::UNIX_EPOCH + Duration::from_secs(self.created)
    }

    pub fn last_modified(&self) -> SystemTime {
        SystemTime::UNIX_EPOCH + Duration::from_secs(self.last_modified)
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

    fn self_encryption_context(&self) -> Arc<Mutex<Option<Box<CryptoContext>>>> {
        unimplemented!()
    }

    fn init_session_key(&mut self, private_key: &[u8]) -> Result<()> {
        if private_key.len() != signature::PrivateKey::BYTES + CryptoBox::MAC_BYTES + Nonce::BYTES {
            return Err(Error::Argument(format!("Invalid session key size")));
        }

        let ctx = self.self_encryption_context();
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
        self.encrypt_keypair = Some(cryptobox::KeyPair::from(&session_keypair));
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
        let Some(keypair) = self.encrypt_keypair.as_ref() else {
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
        let Some(keypair) = self.encrypt_keypair.as_ref() else {
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
        let Some(keypair) = self.encrypt_keypair.as_ref() else {
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
        let Some(keypair) = self.encrypt_keypair.as_ref() else {
            return Err(Error::State("Missing cryptobox keypair".into()));
        };

        cryptobox::decrypt_into(
            cipher,
            &sender.to_encryption_key(),
            keypair.private_key()
        )
    }

    fn create_crypto_context(&self, id: &Id) -> Result<CryptoContext> {
        let Some(keypair) = self.encrypt_keypair.as_ref() else {
            return Err(Error::State("Missing cryptobox keypair".into()));
        };

        CryptoBox::try_from((&id.to_encryption_key(), keypair.private_key())).map(|v|
            CryptoContext::new(id.clone(), v)
        )
    }
}

impl Serialize for Contact {
    fn serialize<S>(&self, _serializer: S) -> result::Result<S::Ok, S::Error>
    where
        S: Serializer
    {
        unimplemented!()
    }
}

impl<'de> Deserialize<'de> for Contact {
    fn deserialize<D>(_deserializer: D) -> result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        unimplemented!()
    }
}
