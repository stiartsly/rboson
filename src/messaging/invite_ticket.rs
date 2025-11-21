
use std::fmt;
use std::time::SystemTime;
use serde::{ Serialize, Deserialize};
use sha2::{Digest, Sha256};

use crate::{
    as_secs,
    Id,
    Error, core::Result,
};

const DEFAULT_EXPIRATION: u64 = 7 * 24 * 60 * 60 * 1000; // 7 days

#[derive(Debug, Clone, Deserialize, Serialize, Hash)]
pub struct InviteTicket {
    #[serde(rename = "c")]
    channel_id: Id,

    #[serde(rename = "i")]
    inviter:    Id,

    #[serde(rename = "p", skip_serializing_if = "crate::is_empty")]
    is_public:  bool,

    #[serde(rename = "e", skip_serializing_if = "crate::is_empty")]
    expire:     u64,

    #[serde(rename = "s")]
    sig:       Vec<u8>,

    #[serde(rename = "sk", skip_serializing_if = "crate::is_none_or_empty")]
    session_key: Option<Vec<u8>>
}

#[allow(unused)]
impl InviteTicket {
    pub const EXPIRATION: u64 = DEFAULT_EXPIRATION;

    pub(crate) fn new(channel_id: Id,
        inviter: Id,
        is_public: bool,
        expire: u64,
        sig: Vec<u8>,
        session_key: Option<Vec<u8>>
    ) -> Self {
        Self {
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
        self.expire < as_secs!(SystemTime::now())
    }

    pub fn session_key(&self) -> Option<&[u8]> {
        self.session_key.as_deref()
    }

    pub fn is_valid(&self, invitee: &Id) -> bool {
        let digest = {
            let invitee = match self.is_public {
                true => &Id::max(),
                false => invitee
            };

            let mut v = Sha256::new();
            v.update(self.channel_id.as_bytes());
            v.update(self.inviter.as_bytes());
            v.update(invitee.as_bytes());
            v.update(&self.expire.to_le_bytes());
            v.finalize().to_vec()
        };

        self.inviter
            .to_signature_key()
            .verify(&digest, &self.sig)
            .is_ok()
    }

    pub fn proof(&self) -> Self {
        Self::new(
            self.channel_id.clone(),
            self.inviter.clone(),
            self.is_public,
            self.expire,
            self.sig.clone(),
            None
        )
    }

    #[cfg(test)]
    pub(crate) fn digest(channel_id: &Id,
        inviter: &Id,
        is_public: bool,
        expire: u64,
        invitee: &Id
    ) -> Vec<u8> {
        let invitee = if is_public {
            &Id::max()
        } else {
            invitee
        };

        let mut v = Sha256::new();
        v.update(channel_id.as_bytes());
        v.update(inviter.as_bytes());
        v.update(invitee.as_bytes());
        v.update(&expire.to_le_bytes());
        v.finalize().to_vec()
    }
}

impl fmt::Display for InviteTicket {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        serde_json::to_string(self)
            .map_err(|_| fmt::Error)?
            .fmt(f)
    }
}

impl TryFrom<&str> for InviteTicket {
    type Error = Error;

    fn try_from(data: &str) -> Result<Self> {
        serde_json::from_str(data).map_err(|e|
            Error::Argument(format!("Failed to parse InviteTicket from string: {}", e))
        )
    }
}

/*
impl fmt::Display for InviteTicket {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "InviteTicket[channel={}, invitor={}",
            self.channel_id,
            self.inviter
        )?;
        if self.is_public {
            write!(f, ", public")?;
        }

        write!(f, ", expiration={}", self.expire)?;
        if self.is_expired() {
            write!(f, ", expired")?;
        }
        write!(f, "]")?;
        Ok(())
    }
}
*/
