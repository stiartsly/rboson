use std::result;
use std::time::{Duration, SystemTime};
use std::sync::{Arc, Mutex};
use serde::{Serialize, Deserialize};
use serde::ser::{Serializer};
use serde::de::{Deserializer};

use crate::{
    as_secs,
    lock,
    Id,
    Error,
    Identity,
    signature::{self, KeyPair},
    cryptobox::{self, CryptoBox, Nonce},
    core::Result,
    core::CryptoContext,
    messaging::{
        profile::Profile
    }
};

#[allow(unused)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ContactType {
    Unknown = 0,
    Contact = 1,
    Group   = 2,
}

pub type Contact = GenericContact<()>;

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct GenericContact<T> where T: Clone {
    id              : Id,
    auto            : bool,

    session_kp      : Option<signature::KeyPair>,
    encryption_kp   : Option<cryptobox::KeyPair>,
    session_id      : Option<Id>,

    rx_crypto_context: Option<Arc<Mutex<Box<CryptoContext>>>>,
    tx_crypto_context: Option<Arc<Mutex<Box<CryptoContext>>>>,

    home_peerid     : Id,
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

    // derived data field used to store additional data for derived Contact types,
    // For example, type 'Channel' is the derived struct with extra data
    derived_data    : Option<T>,
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

    pub(crate) fn new(id: Id, home_peerid: Id, addtional: T) -> Self {
        Self {
            id,
            auto            : true,
            session_kp      : None,
            encryption_kp : None,
            session_id      : None,
            home_peerid,
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

            rx_crypto_context: None,
            tx_crypto_context: None,
            derived_data    : Some(addtional),
        }
    }

    pub(crate) fn derived_mut(&mut self) -> &mut T {
        self.derived_data.as_mut().expect("Derived data is missing")
    }

    pub(crate) fn derived(&self) -> &T {
        self.derived_data.as_ref().expect("Derived data is missing")
    }

    pub fn id(&self) -> &Id {
        &self.id
    }

    pub fn home_peerid(&self) -> &Id {
        &self.home_peerid
    }

    pub(crate) fn set_home_peerid(&mut self, peerid: Id) {
        self.home_peerid = peerid;
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

    pub fn session_keypair(&self) -> Option<&signature::KeyPair> {
        self.session_kp.as_ref()
    }

    fn init_session_key(&mut self, sk: &[u8]) -> Result<()> {
        let key = if sk.len() == signature::PrivateKey::BYTES {
            // Do nothing, no decryption here.
            None
        } else if sk.len() == signature::PrivateKey::BYTES + CryptoBox::MAC_BYTES + Nonce::BYTES {
            let Some(ctxt) = self.self_encryption_context() else {
                return Err(Error::State("No self encryption context".into()));
            };
            let Ok(key) = lock!(ctxt).decrypt_into(sk) else {
                return Err(Error::Crypto(format!("Error decrypting session key.")))?
            };
            Some(key)
        } else {
           return Err(Error::Crypto(format!("Invalid session key size")));
        };

        let kp = KeyPair::try_from(
            match key {
                Some(ref v) => v.as_slice(),
                None => sk,
            }
        )?;
        self.session_id = Some(Id::from(kp.public_key()));
        self.encryption_kp = Some(cryptobox::KeyPair::from(&kp));
        self.session_kp = Some(kp);

        Ok(())
    }

    pub(crate) fn set_session_key(&mut self, sk: &[u8]) -> Result<()> {
        self.init_session_key(sk)?;
        self.touch();
        Ok(())
    }

    pub(crate) fn has_session_key(&self) -> bool {
        self.session_kp.is_some()
    }

    pub fn session_id(&self) -> Option<&Id> {
        self.session_id.as_ref()
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
            false => None
        }
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

    pub fn display_name(&mut self) -> &str {
        if let Some(name) = self.display_name.as_ref() {
            // Do nothing.
        } else if let Some(remark) = self.remark.as_ref().filter(|v| !v.is_empty()) {
            self.display_name = Some(remark.to_string());
        } else if let Some(name) = self.name.as_ref().filter(|v| !v.is_empty()) {
            self.display_name = Some(name.to_string());
        } else {
            self.display_name = Some(self.id.to_abbr_str());
        }
        crate::unwrap!(self.display_name)
    }

    pub fn update_profile(&mut self, profile: &Profile) -> Result<()> {
        if profile.id() != &self.id {
            return Err(Error::Argument("Profile does not match contact".into()));
        }
        if !profile.is_genuine() {
            return Err(Error::Argument("Profile is not genuine".into()));
        }

        self.home_peerid = profile.home_peerid().clone();
        self.name = Some(profile.name().to_string());
        self.avatar = profile.has_avatar();
        self.display_name = None;

        self.updated();
        Ok(())
    }

    pub fn update_contact(&mut self, contact: &Self) -> Result<()> {
        if contact.id() != &self.id {
            return Err(Error::Argument("Contact is not matched".into()));
        }

        self.home_peerid = contact.home_peerid().clone();
        self.remark = contact.remark.as_ref().map(|v| v.clone());
        self.tags = contact.tags.as_ref().map(|v| v.clone());
        self.muted = contact.muted;
        self.blocked = contact.blocked;
        self.created = contact.created;
        self.last_modified = contact.last_modified;
        self.name = contact.name.as_ref().map(|v| v.clone());
        self.avatar = contact.avatar;
        self.display_name = None;

        self.updated();
        Ok(())
    }

    fn self_encryption_context(&self) -> Option<Arc<Mutex<CryptoContext>>> {
        unimplemented!()
    }

    fn touch(&mut self) {
        if self.auto {
            self.auto = false;
        }
        if !self.modified {
            self.modified = true;
            self.increment_revision();
        }
        self.last_modified = as_secs!(SystemTime::now());
    }

    fn updated(&mut self) {
		self.last_updated = Some(SystemTime::now());
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
    fn id(&self) -> &Id {
        &self.id
    }

    fn sign(&self, data: &[u8], sig: &mut [u8]) -> Result<usize> {
        let Some(kp) = self.session_kp.as_ref() else {
            Err(Error::State("Session keypair is missing".into()))?
        };
        signature::sign(data, sig, kp.private_key())
    }

    fn verify(&self, data: &[u8], sig: &[u8]) -> Result<()> {
        let Some(kp) = self.session_kp.as_ref() else {
            Err(Error::State("Session keypair is missing".into()))?
        };
        signature::verify(data, sig, kp.public_key())
    }

    fn encrypt(&self, recipient: &Id, plain: &[u8], cipher: &mut [u8]) -> Result<usize> {
        let Some(kp) = self.encryption_kp.as_ref() else {
            Err(Error::State("Encryption keypair is missing".into()))?
        };

        cryptobox::encrypt(
            plain,
            cipher,
            &Nonce::random(),
            &recipient.to_encryption_key(),
            kp.private_key()
        )
    }

    fn decrypt(&self, sender: &Id, cipher: &[u8], plain: &mut [u8]) -> Result<usize> {
        let Some(kp) = self.encryption_kp.as_ref() else {
            return Err(Error::State("Encryption keypair is missing".into()));
        };

        cryptobox::decrypt(
            cipher,
            plain,
            &sender.to_encryption_key(),
            kp.private_key()
        )
    }

    fn create_crypto_context(&self, id: &Id) -> Result<CryptoContext> {
        let Some(kp) = self.encryption_kp.as_ref() else {
            return Err(Error::State("Encryption keypair is missing".into()));
        };

        CryptoBox::try_from((&id.to_encryption_key(), kp.private_key())).map(|v|
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
