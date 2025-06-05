
use std::time::SystemTime;
use std::fmt;
use serde::{ Serialize, Deserialize};
use sha2::{Digest, Sha256};

use crate::{
    id,
    Id
};

pub static DEFAULT_EXPIRATION: u64 = 7 * 24 * 60 * 60 * 1000; // 7 days

#[derive(Debug, Clone, Deserialize, Serialize, Hash)]
#[allow(unused)]
pub struct InviteTicket {
    #[serde(rename = "c")]
    channel_id: Id,

    #[serde(rename = "i")]
    inviter:    Id,

    #[serde(rename = "p")]
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    is_public:  bool,

    #[serde(rename = "e")]
    expire:     SystemTime,

    #[serde(rename = "s")]
    sig:       Vec<u8>,

    #[serde(rename = "sk")]
    #[serde(skip_serializing_if = "Option::is_none")]
    session_key: Option<Vec<u8>>
}

#[allow(unused)]
impl InviteTicket {
    pub(crate) fn new(channel_id: Id,
        inviter: Id,
        is_public: bool,
        expire: SystemTime,
        sig: Vec<u8>,
        session_key: Option<Vec<u8>>
    ) -> Self {
        InviteTicket {
            channel_id,
            inviter,
            is_public,
            expire,
            sig,
            session_key
        }
    }

    pub fn channel_id(&self) -> &Id {
        &self.channel_id
    }

    pub fn inviter(&self) -> &Id {
        &self.inviter
    }

    pub fn is_public(&self) -> bool {
        self.is_public
    }

    pub fn is_expired(&self) -> bool {
        self.expire < SystemTime::now()
    }

    pub fn session_key(&self) -> Option<&[u8]> {
        self.session_key.as_ref().map(|v|v.as_slice())
    }

    pub fn is_valid(&self, invitee: &Id) -> bool {
        let digest = generate_digest(
            &self.channel_id,
            &self.inviter,
            self.is_public,
            self.expire,
            invitee,
        );

        self.inviter
            .to_signature_key()
            .verify(digest.as_slice(), &self.sig)
            .is_ok()
    }

    pub fn proof(&self) -> InviteTicket {
        InviteTicket::new(
            self.channel_id.clone(),
            self.inviter.clone(),
            self.is_public,
            self.expire,
            self.sig.clone(),
            None
        )
    }
}

impl fmt::Display for InviteTicket {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "InviteTicket[channel={}, invitor={}",
            self.channel_id.to_base58(),
            self.inviter.to_base58()
        )?;
        if self.is_public {
            write!(f, ", public")?;
        }

        // TODO: write!(_f, ", expiration={}", self.expire)?;
        if self.is_expired() {
            write!(f, ", expired")?;
        }
        write!(f, "]")?;
        Ok(())
    }
}

pub(crate) fn generate_digest(channel_id: &Id,
    inviter: &Id,
    is_public: bool,
    _expire: SystemTime,
    invitee: &Id
) -> Vec<u8> {
    let invitee_bytes = if is_public {
        id::MAX_ID.as_bytes()
    } else {
        invitee.as_bytes()
    };

    let mut sha256 = Sha256::new();
    sha256.update(channel_id.as_bytes());
    sha256.update(inviter.as_bytes());
    sha256.update(invitee_bytes);
    // TODO: expire

    sha256.finalize().to_vec()
}
