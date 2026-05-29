use std::time::{SystemTime, UNIX_EPOCH};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::Id;
use crate::messaging::errors::{Error, Result};

/// Default ticket lifetime: 7 days expressed as milliseconds.
const DEFAULT_EXPIRATION_MS: u64 = 7 * 24 * 60 * 60 * 1000;

/// A signed invitation ticket that grants access to a channel.
///
/// CBOR field names match the Java implementation:
/// `c` = channel_id, `i` = inviter, `p` = is_public, `e` = expire, `s` = sig, `sk` = session_key.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InviteTicket {
    /// The channel to which the holder is invited.
    #[serde(rename = "c")]
    channel_id: Id,

    /// The boson `Id` of the user who created this ticket.
    #[serde(rename = "i")]
    inviter: Id,

    /// `true` for bearer tickets (no specific invitee); `false` for named tickets.
    #[serde(rename = "p", skip_serializing_if = "Option::is_none")]
    is_public: Option<bool>,

    /// Expiry timestamp in milliseconds since UNIX epoch.
    #[serde(rename = "e")]
    expire_ms: u64,

    /// Ed25519 signature by the inviter.
    #[serde(rename = "s")]
    sig: Vec<u8>,

    /// Encrypted channel session key, present only in named tickets.
    #[serde(rename = "sk", skip_serializing_if = "Option::is_none")]
    session_key: Option<Vec<u8>>,
}

impl InviteTicket {
    /// The default ticket validity window (7 days in milliseconds).
    pub const DEFAULT_EXPIRATION_MS: u64 = DEFAULT_EXPIRATION_MS;

    /// Construct directly (used by the builder / server-side code).
    pub fn new(
        channel_id:  Id,
        inviter:     Id,
        is_public:   bool,
        expire_ms:   u64,
        sig:         Vec<u8>,
        session_key: Option<Vec<u8>>,
    ) -> Self {
        Self {
            channel_id,
            inviter,
            is_public: Some(is_public),
            expire_ms,
            sig,
            session_key,
        }
    }

    // --- accessors ---

    pub fn channel_id(&self) -> &Id  { &self.channel_id }
    pub fn inviter(&self)    -> &Id  { &self.inviter }

    /// A "named" ticket is addressed to a specific invitee.
    pub fn is_named_ticket(&self)   -> bool { !self.is_bearer_ticket() }

    /// A "bearer" ticket can be used by anyone (no specific invitee).
    pub fn is_bearer_ticket(&self)  -> bool { self.is_public.unwrap_or(false) }

    /// Returns `true` when the current time is past the expiry.
    pub fn is_expired(&self) -> bool {
        let now_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);
        now_ms > self.expire_ms
    }

    /// The encrypted session key bytes, if present (named tickets only).
    pub fn session_key(&self) -> Option<&[u8]> {
        self.session_key.as_deref()
    }

    /// Verify the signature.
    ///
    /// For bearer tickets the invitee is treated as `Id::max()`.
    pub fn is_valid(&self, invitee: &Id) -> bool {
        let digest = Self::digest(
            &self.channel_id,
            &self.inviter,
            invitee,
            self.is_bearer_ticket(),
            self.expire_ms,
        );
        self.inviter
            .to_signature_key()
            .verify(&digest, &self.sig)
            .is_ok()
    }

    /// Return a copy of this ticket without the session key (suitable for public distribution).
    pub fn proof(&self) -> Self {
        Self {
            channel_id:  self.channel_id.clone(),
            inviter:     self.inviter.clone(),
            is_public:   self.is_public,
            expire_ms:   self.expire_ms,
            sig:         self.sig.clone(),
            session_key: None,
        }
    }

    /// Compute the SHA-256 digest that is signed.
    pub fn digest(
        channel_id: &Id,
        inviter:    &Id,
        invitee:    &Id,
        is_public:  bool,
        expire_ms:  u64,
    ) -> Vec<u8> {
        let effective_invitee = if is_public { &Id::max() } else { invitee };
        let mut h = Sha256::new();
        h.update(channel_id.as_bytes());
        h.update(inviter.as_bytes());
        h.update(effective_invitee.as_bytes());
        h.update(expire_ms.to_be_bytes());
        h.finalize().to_vec()
    }

    // --- serialization helpers ---

    /// Encode this ticket as CBOR and return the hex string.
    pub fn to_hex(&self) -> Result<String> {
        let cbor = serde_cbor::to_vec(self)
            .map_err(|e| Error::Encoding(format!("Failed to CBOR-encode invite ticket: {}", e)))?;
        Ok(hex::encode(cbor))
    }

    /// Decode a ticket from a hex-encoded CBOR string.
    pub fn from_hex(hex_str: &str) -> Result<Self> {
        let bytes = hex::decode(hex_str)
            .map_err(|e| Error::Encoding(format!("Invalid hex string: {}", e)))?;
        serde_cbor::from_slice::<InviteTicket>(&bytes)
            .map_err(|e| Error::Encoding(format!("Failed to CBOR-decode invite ticket: {}", e)))
    }

    /// Encode this ticket as CBOR and return the Base58 string.
    pub fn to_base58(&self) -> Result<String> {
        let cbor = serde_cbor::to_vec(self)
            .map_err(|e| Error::Encoding(format!("Failed to CBOR-encode invite ticket: {}", e)))?;
        Ok(bs58::encode(cbor).into_string())
    }

    /// Decode a ticket from a Base58-encoded CBOR string.
    pub fn from_base58(s: &str) -> Result<Self> {
        let bytes = bs58::decode(s)
            .into_vec()
            .map_err(|e| Error::Encoding(format!("Invalid Base58 string: {}", e)))?;
        serde_cbor::from_slice::<InviteTicket>(&bytes)
            .map_err(|e| Error::Encoding(format!("Failed to CBOR-decode invite ticket: {}", e)))
    }
}
