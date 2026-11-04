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
    time::{SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use boson_api::errors::{Error, Result};
use boson_api::{utils, Id, Identity};
use super::errors::{AuthenticationError, EncodingError};

/// Default ticket lifetime: seven days in milliseconds.
pub const DEFAULT_EXPIRATION_MS: u64 = 7 * 24 * 60 * 60 * 1000;

/// MIME type used for messages containing an invite ticket.
pub const CONTENT_TYPE: &str = "application/invite-ticket";

/// A signed channel invitation.
#[derive(Debug, Clone, Serialize, Deserialize)]
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
    expiration_ms: u64,

    #[serde(rename = "sig", with = "bytes_in_bas64")]
    signature: Vec<u8>,

    #[serde(rename = "sk", with = "bytes_in_bas64")]
    session_key: Vec<u8>,
}

impl InviteTicket {
    pub const DEFAULT_EXPIRATION_MS: u64 = DEFAULT_EXPIRATION_MS;
    pub const CONTENT_TYPE: &'static str = CONTENT_TYPE;

    pub fn new(
        channel_id: Id,
        session_id: Id,
        inviter: Id,
        invitee: Option<Id>,
        expiration_ms: u64,
        signature: Vec<u8>,
        session_key: Vec<u8>,
    ) -> Result<Self> {
        Ok(Self {
            channel_id,
            session_id,
            inviter,
            invitee,
            expiration_ms,
            signature,
            session_key,
        })
    }

    pub(crate) fn create(
        inviter: &dyn Identity,
        channel_id: Id,
        session_id: Id,
        invitee: Option<Id>,
        expiration_ms: u64,
        session_key: Vec<u8>,
    ) -> Result<Self> {
        let digest = Self::digest(
            &channel_id, &session_id, inviter.id(), invitee.as_ref(), expiration_ms,
        );
        let signature = inviter
            .sign_into(&digest)
            .map_err(|e| AuthenticationError::new(e.to_string()))?;

        Self::new(
            channel_id,
            session_id,
            *inviter.id(),
            invitee,
            expiration_ms,
            signature,
            session_key,
        )
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

    pub fn expiration_ms(&self) -> u64 {
        self.expiration_ms
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

    pub(crate) fn signature(&self) -> &[u8] {
        &self.signature
    }

    pub fn is_expired(&self) -> bool {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_millis() as u64 > self.expiration_ms)
            .unwrap_or(false)
    }

    pub fn is_genuine(&self) -> bool {
        let digest = Self::digest(
            &self.channel_id,
            &self.session_id,
            &self.inviter,
            self.invitee.as_ref(),
            self.expiration_ms,
        );
        self.inviter
            .to_signature_key()
            .verify(&digest, &self.signature)
            .unwrap_or(false)
    }

    pub fn revise(&self, session_key: Vec<u8>) -> Result<Self> {
        Self::new(
            self.channel_id,
            self.session_id,
            self.inviter,
            self.invitee,
            self.expiration_ms,
            self.signature.clone(),
            session_key,
        )
    }

    fn digest(
        channel_id: &Id,
        session_id: &Id,
        inviter: &Id,
        invitee: Option<&Id>,
        expiration_ms: u64,
    ) -> Vec<u8> {
        let mut sha256 = Sha256::new();
        sha256.update(channel_id);
        sha256.update(session_id);
        sha256.update(inviter);
        if let Some(invitee) = invitee {
            sha256.update(invitee);
        }
        sha256.update(expiration_ms.to_be_bytes());
        sha256.finalize().to_vec()
    }
}

impl FromStr for InviteTicket {
    type Err = Error;

    fn from_str(value: &str) -> Result<Self> {
        serde_json::from_slice(value.as_bytes())
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

    pub fn serialize<S>(bytes: &[u8], se: S) -> StdResult<S::Ok, S::Error>
    where
        S: Serializer,
    {
        se.serialize_str(&STANDARD.encode(bytes))
    }

    pub fn deserialize<'de, D>(de: D) -> StdResult<Vec<u8>, D::Error>
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
