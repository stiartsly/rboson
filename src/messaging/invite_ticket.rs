
use std::fmt;
use std::time::SystemTime;
use serde::{ Serialize, Deserialize};
use sha2::{Digest, Sha256};

use crate::{
    Id,
    Error, core::Result,
};

const DEFAULT_EXPIRATION: u64 = 7 * 24 * 60 * 60 * 1000; // 7 days

#[derive(Debug, Clone, Deserialize, Serialize, Hash)]
pub struct InviteTicket {
    #[serde(rename = "c", with = "crate::serde_id_as_bytes")]
    channel_id: Id,

    #[serde(rename = "i", with = "crate::serde_id_as_bytes")]
    inviter: Id,

    #[serde(rename = "p", skip_serializing_if = "crate::is_none_or_empty")]
    is_public: Option<bool>,

    #[serde(rename = "e", skip_serializing_if = "crate::is_empty")]
    expire: u64,

    #[serde(rename = "s")]
    sig: Vec<u8>,

    #[serde(rename = "sk", skip_serializing_if="crate::is_none_or_empty")]
    session_key: Option<Vec<u8>>
}

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
            is_public: Some(is_public),
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
        self.is_public.unwrap_or(false)
    }

    pub fn is_expired(&self) -> bool {
        self.expire < crate::as_secs!(SystemTime::now())
    }

    pub fn session_key(&self) -> Option<&[u8]> {
        self.session_key.as_deref()
    }

    pub fn is_valid(&self, invitee: &Id) -> bool {
        let digest = Self::digest(
            &self.channel_id,
            &self.inviter,
            invitee,
            self.is_public.unwrap_or(false),
            self.expire
        );

        self.inviter
            .to_signature_key()
            .verify(&digest, &self.sig)
            .is_ok()
    }

    pub fn proof(&self) -> Self {
        Self {
            channel_id: self.channel_id.clone(),
            inviter: self.inviter.clone(),
            is_public: self.is_public,
            expire: self.expire,
            sig: self.sig.clone(),
            session_key: None
        }
    }

    pub(crate) fn digest(channel_id: &Id,
        inviter: &Id,
        invitee: &Id,
        is_public: bool,
        expire: u64
    ) -> Vec<u8> {
        let invitee = if is_public {
            &Id::max()
        } else {
            invitee
        };

        let mut v = Sha256::new();
        v.update(channel_id);
        v.update(inviter);
        v.update(invitee);
        v.update(&expire.to_be_bytes());
        v.finalize().to_vec()
    }

    #[allow(unused)]
    pub fn to_hex(&self) -> Result<String> {
        let cbor = serde_cbor::to_vec(self).map_err(|e|
            Error::Argument(format!("Error serializing invite ticket to hex: {}", e))
        )?;

        use hex::ToHex;
        Ok(cbor.encode_hex::<String>())
    }

    #[allow(unused)]
    pub fn from_hex(hex:&str) -> Result<Self> {
        use hex::FromHex;
        let bytes = Vec::from_hex(hex).map_err(|e| {
            Error::Argument(format!("Error decoding hex string: {}", e))
        })?;

        let ticket = serde_cbor::from_slice::<InviteTicket>(&bytes).map_err(|e| {
            Error::Argument(format!(
                "Error deserializing invite ticket from cbor: {}",
                e
            ))
        })?;

        Ok(ticket)
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

impl fmt::Display for InviteTicket {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "InviteTicket[channel={}, invitor={}",
            self.channel_id,
            self.inviter
        )?;
        if self.is_public.unwrap_or(false) {
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
