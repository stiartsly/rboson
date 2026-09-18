use std::{
    fmt,
    str::FromStr,
    time::{SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::messaging::errors::{Error, Result};
use crate::{Id, Identity};

/// Default ticket lifetime: seven days in milliseconds.
pub const DEFAULT_EXPIRATION_MS: u64 = 7 * 24 * 60 * 60 * 1000;

/// MIME type used for messages containing an invite ticket.
pub const CONTENT_TYPE: &str = "application/invite-ticket";

/// A signed channel invitation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InviteTicket {
    #[serde(rename = "c")]
    channel_id: Id,
    #[serde(rename = "sid")]
    session_id: Id,
    #[serde(rename = "i")]
    inviter: Id,
    #[serde(rename = "ie", skip_serializing_if = "Option::is_none")]
    invitee: Option<Id>,
    #[serde(rename = "e")]
    expiration_ms: u64,
    #[serde(rename = "sig")]
    #[serde(with = "base64_bytes")]
    signature: Vec<u8>,
    #[serde(rename = "sk")]
    #[serde(with = "base64_bytes")]
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

    pub fn create(
        inviter: &dyn Identity,
        channel_id: Id,
        session_id: Id,
        invitee: Option<Id>,
        expiration_ms: u64,
        session_key: Vec<u8>,
    ) -> Result<Self> {
        let digest = Self::digest(
            &channel_id,
            &session_id,
            inviter.id(),
            invitee.as_ref(),
            expiration_ms,
        );
        let signature = inviter
            .sign_into(&digest)
            .map_err(|error| Error::Auth(error.to_string()))?;
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

    pub fn digest(
        channel_id: &Id,
        session_id: &Id,
        inviter: &Id,
        invitee: Option<&Id>,
        expiration_ms: u64,
    ) -> Vec<u8> {
        let mut sha256 = Sha256::new();
        sha256.update(channel_id.as_bytes());
        sha256.update(session_id.as_bytes());
        sha256.update(inviter.as_bytes());
        if let Some(invitee) = invitee {
            sha256.update(invitee.as_bytes());
        }
        sha256.update(expiration_ms.to_be_bytes());
        sha256.finalize().to_vec()
    }

    pub fn to_bytes(&self) -> Result<Vec<u8>> {
        serde_json::to_vec(self)
            .map_err(|error| Error::Encoding(format!("Failed to encode invite ticket: {error}")))
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        serde_json::from_slice(bytes)
            .map_err(|error| Error::Encoding(format!("Failed to decode invite ticket: {error}")))
    }
}

mod base64_bytes {
    use base64::{engine::general_purpose::STANDARD, Engine};
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S>(bytes: &[u8], serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&STANDARD.encode(bytes))
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Vec<u8>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let encoded = String::deserialize(deserializer)?;
        STANDARD.decode(encoded).map_err(serde::de::Error::custom)
    }
}

impl fmt::Display for InviteTicket {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let json = serde_json::to_string(self).map_err(|_| fmt::Error)?;
        f.write_str(&json)
    }
}

impl FromStr for InviteTicket {
    type Err = Error;

    fn from_str(value: &str) -> Result<Self> {
        Self::from_bytes(value.as_bytes())
    }
}

#[cfg(test)]
mod tests {
    use super::InviteTicket;
    use crate::{CryptoIdentity, Id};
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn named_ticket_is_signed_and_round_trips() {
        let inviter = CryptoIdentity::new();
        let channel_id = Id::random();
        let session_id = Id::random();
        let invitee = Id::random();
        let expiration = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64
            + 60_000;
        let ticket = InviteTicket::create(
            &inviter,
            channel_id,
            session_id,
            Some(invitee),
            expiration,
            vec![7; 64],
        )
        .unwrap();

        assert!(ticket.is_named_ticket());
        assert!(ticket.is_genuine());
        assert!(!ticket.is_expired());

        let decoded = InviteTicket::from_bytes(&ticket.to_bytes().unwrap()).unwrap();
        assert_eq!(decoded.channel_id(), &channel_id);
        assert_eq!(decoded.session_id(), &session_id);
        assert_eq!(decoded.invitee(), Some(&invitee));
        assert!(decoded.is_genuine());
        let json = ticket.to_string();
        assert!(json.contains("\"sig\":\""));
        assert!(json.contains("\"sk\":\""));
    }

    #[test]
    fn revised_ticket_defensively_replaces_session_key() {
        let inviter = CryptoIdentity::new();
        let ticket = InviteTicket::create(
            &inviter,
            Id::random(),
            Id::random(),
            None,
            u64::MAX,
            vec![1; 64],
        )
        .unwrap();

        let revised = ticket.revise(vec![2; 64]).unwrap();
        assert!(revised.is_bearer_ticket());
        assert!(revised.is_genuine());
        assert_eq!(revised.session_key(), &[2; 64]);
    }
}
