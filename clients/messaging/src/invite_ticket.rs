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
    fmt,
    str::FromStr,
    time::SystemTime
};
use serde::{Serialize, Serializer, Deserialize, ser::SerializeMap};
use base64::{engine::general_purpose::STANDARD, Engine};
use sha2::{Digest,Sha256};

use boson_api::{
    Error,
    Result,
    utils,
    Id,
    Identity,
};
use super::errors::{
    AuthenticationError,
    EncodingError,
};

/// Default ticket lifetime: seven days in milliseconds.
pub const DEFAULT_EXPIRATION_MS: u64 = 7 * 24 * 60 * 60 * 1000;

/// MIME type used for messages containing an invite ticket.
pub const CONTENT_TYPE: &str = "application/invite-ticket";

/// A signed channel invitation.
#[derive(Debug, Clone, Deserialize)]
pub struct InviteTicket {
    #[serde(rename = "c", with = "utils::id")]
    channel_id: Id,
    #[serde(rename = "sid", with = "utils::id")]
    session_id: Id,
    #[serde(rename = "i", with = "utils::id")]
    inviter: Id,
    #[serde(rename = "ie", with = "utils::id_opt")]
    invitee: Option<Id>,
    #[serde(rename = "e")]
    expiration: i64,
    #[serde(rename = "sig", with = "bytes_in_bas64")]
    signature: Vec<u8>,
    #[serde(rename = "sk", with = "bytes_in_bas64")]
    session_key: Vec<u8>,
}

impl Serialize for InviteTicket {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        struct Bytes<'a>(&'a [u8]);

        impl Serialize for Bytes<'_> {
            fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
            where
                S: Serializer,
            {
                if serializer.is_human_readable() {
                    serializer.serialize_str(&STANDARD.encode(self.0))
                } else {
                    serializer.serialize_bytes(self.0)
                }
            }
        }

        let mut map = serializer.serialize_map(Some(7))?;
        map.serialize_entry("c", &self.channel_id)?;
        map.serialize_entry("sid", &self.session_id)?;
        map.serialize_entry("i", &self.inviter)?;
        map.serialize_entry("ie", &self.invitee)?;
        map.serialize_entry("e", &self.expiration)?;
        map.serialize_entry("sig", &Bytes(&self.signature))?;
        map.serialize_entry("sk", &Bytes(&self.session_key))?;
        map.end()
    }
}

impl InviteTicket {
    pub const DEFAULT_EXPIRATION_MS: u64 = DEFAULT_EXPIRATION_MS;
    pub const CONTENT_TYPE: &'static str = CONTENT_TYPE;

    pub fn new(
        channel_id: Id,
        session_id: Id,
        inviter: Id,
        invitee: Option<Id>,
        expiration: i64,
        signature: Vec<u8>,
        session_key: Vec<u8>,
    ) -> Result<Self> {
        Ok(Self {
            channel_id,
            session_id,
            inviter,
            invitee,
            expiration,
            signature,
            session_key,
        })
    }

    pub(crate) fn create(
        inviter: &dyn Identity,
        channel_id: Id,
        session_id: Id,
        invitee: Option<Id>,
        expiration: i64,
        session_key: Vec<u8>,
    ) -> Result<Self> {
        let digest = Self::generate_digest(
            &channel_id, &session_id, inviter.id(), invitee.as_ref(), expiration,
        );
        let signature = inviter
            .sign_into(&digest)
            .map_err(|e| AuthenticationError::new(e.to_string()))?;

        Ok(Self {
            channel_id,
            session_id,
            inviter: *inviter.id(),
            invitee,
            expiration,
            signature,
            session_key,
        })
    }

    pub fn revise(&self, session_key: Vec<u8>) -> Self {
        Self {
            channel_id: self.channel_id,
            session_id: self.session_id,
            inviter: self.inviter,
            invitee: self.invitee,
            expiration: self.expiration,
            signature: self.signature.clone(),
            session_key,
        }
    }

    pub fn channel_id(&self) -> &Id {
        &self.channel_id
    }

    pub fn session_id(&self) -> &Id {
        &self.session_id
    }

    pub fn inviter(&self) -> &Id {
        &self.inviter
    }

    pub fn invitee(&self) -> Option<&Id> {
        self.invitee.as_ref()
    }

    pub fn is_named_ticket(&self) -> bool {
        self.invitee.is_some()
    }

    pub fn is_bearer_ticket(&self) -> bool {
        self.invitee.is_none()
    }

    pub fn session_key(&self) -> &[u8] {
        &self.session_key
    }

    pub fn is_expired(&self) -> bool {
        self.expiration < boson_api::as_ms!(SystemTime::now()) as i64
    }

    pub fn is_genuine(&self) -> bool {
        let digest = Self::generate_digest(
            &self.channel_id,
            &self.session_id,
            &self.inviter,
            self.invitee.as_ref(),
            self.expiration,
        );
        self.inviter
            .to_signature_key()
            .verify(&digest, &self.signature)
            .unwrap_or(false)
    }

    fn generate_digest(
        channel_id: &Id,
        session_id: &Id,
        inviter: &Id,
        invitee: Option<&Id>,
        expiration: i64,
    ) -> Vec<u8> {
        let mut sha = Sha256::new();
        sha.update(channel_id);
        sha.update(session_id);
        sha.update(inviter);
        invitee.map(|v| sha.update(v));
        sha.update(expiration.to_be_bytes());
        sha.finalize().to_vec()
    }
}

impl TryFrom<&[u8]> for InviteTicket {
    type Error = Error;

    fn try_from(value: &[u8]) -> Result<Self> {
        serde_json::from_slice(value).map_err(|e| {
            EncodingError::new(format!("Decoding invite ticket error: {e}")).into()
        })
    }
}

impl FromStr for InviteTicket {
    type Err = Error;

    fn from_str(value: &str) -> Result<Self> {
        serde_json::from_str(value)
            .map_err(|e| {
                EncodingError::new(format!("Decoding invite ticket error: {e}")).into()
            })
    }
}

impl fmt::Display for InviteTicket {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let json = serde_json::to_string(self).map_err(|_| fmt::Error)?;
        f.write_str(&json)
    }
}

mod bytes_in_bas64 {
    use std::result::Result as StdResult;
    use base64::{engine::general_purpose::STANDARD, Engine};
    use serde::{Deserialize, Deserializer, Serializer};

    pub(super) fn serialize<S>(bytes: &[u8], se: S) -> StdResult<S::Ok, S::Error>
    where
        S: Serializer,
    {
        se.serialize_str(&STANDARD.encode(bytes))
    }

    pub(super) fn deserialize<'de, D>(de: D) -> StdResult<Vec<u8>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let encoded = String::deserialize(de)?;
        STANDARD.decode(encoded).map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
#[path = "unitests/test_invite_ticket.rs"]
mod test_invite_ticket;
