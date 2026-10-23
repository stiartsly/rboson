use std::{
    fmt,
    hash::{Hash, Hasher},
    result::Result as StdResult,
    collections::HashMap,
};
use sha2::{Digest, Sha256};
use unicode_normalization::UnicodeNormalization;
use serde::{Deserialize, Serialize};

use crate::core::errors::{
    Error,
    Result,
    StateError,
    ArgumentError,
};
use crate::core::{
    CryptoIdentity,
    Id,
    Identity,
    signature::{self, KeyPair, PrivateKey}
};
use crate::utils::codec;

#[derive(Default)]
pub struct PeerBuilder<'a> {
    identity        : Option<CryptoIdentity>,
    keep_private_key: bool,
    seq             : i32,
    node            : Option<&'a CryptoIdentity>,
    fingerprint     : u64,
    endpoint        : Option<String>,
    extra           : Option<Vec<u8>>,

    for_update      : Option<PeerInfo>,
}

impl<'a> PeerBuilder<'a> {
    pub const MAX_PAYLOAD_BYTES: usize = 768;

    /// Starts a fresh peer builder for `endpoint`.
    pub fn new(endpoint: impl Into<String>) -> Self {
        Self {
            endpoint: Some(endpoint.into()),
            ..Self::default()
        }
    }

    fn for_update(peer: PeerInfo) -> Self {
        let identity = peer
            .private_key()
            .map(|sk| CryptoIdentity::try_from(sk).unwrap());
        let keep_private_key = peer.has_private_key();
        let seq = peer.sequence_number() + 1;
        let fingerprint = peer.fingerprint();
        let endpoint = peer.endpoint().to_string();
        let extra = peer.extra_data().map(|value| value.to_vec());
        Self {
            for_update: Some(peer),
            identity,
            keep_private_key,
            seq,
            node: None,
            fingerprint,
            endpoint: Some(endpoint),
            extra,
        }
    }

    pub fn with_endpoint(mut self, endpoint: impl Into<String>) -> Self {
        self.endpoint = Some(endpoint.into().nfc().collect::<String>());
        self
    }

    pub fn with_extra(mut self, extra: &[u8]) -> Self {
        self.extra = Some(extra.to_vec());
        self
    }

    pub fn with_extra_map(mut self, extra_map: &HashMap<String, String>) -> Self {
        let extra = serde_json::to_vec(extra_map).unwrap_or_default();
        self.extra = Some(extra);
        self
    }

    pub fn with_fingerprint(mut self, fingerprint: u64) -> Self {
        self.fingerprint = fingerprint;
        self
    }

    pub fn with_node(mut self, node: &'a CryptoIdentity) -> Self {
        self.node = Some(node);
        self
    }

    pub fn with_sequence_number(mut self, seq: i32) -> Self {
        self.seq = seq;
        self
    }

    pub fn with_identity(mut self, key: impl Into<CryptoIdentity>) -> Self {
        self.identity = Some(key.into());
        self
    }

    pub fn with_key(mut self, key: impl Into<KeyPair>) -> Self {
        self.identity = Some(CryptoIdentity::from(key.into()));
        self.keep_private_key = true;
        self
    }

    pub fn with_private_key(self, sk: impl Into<PrivateKey>) -> Self {
        self.with_key(KeyPair::from(sk.into()))
    }

    pub fn keep_private_key(mut self) -> Self {
        self.keep_private_key = true;
        self
    }

    pub fn build(mut self) -> Result<PeerInfo> {
        let Some(endpoint) = self.endpoint.as_ref() else {
            return Err(StateError::new("Missing endpoint."));
        };
        if endpoint.is_empty() {
            return Err(StateError::new("Endpoint cannot be empty."));
        }
        if self.seq < 0 {
            return Err(ArgumentError::new("Sequence number cannot be negative."));
        }

        if let Some(for_update) = self.for_update.as_ref() {
            if for_update.is_authenticated() || self.node.is_none() {
                return Err(StateError::new("Cannot remove node authentication from an authenticated peer info"));
            }
            if self.seq != for_update.sequence_number() + 1 {
                return Err(StateError::new("Cannot change sequence for an update."));
            }
            if self.fingerprint != for_update.fingerprint() {
                return Err(StateError::new("Cannot change fingerprint for an update."));
            }
            if self.identity.as_ref().map(|id| id.id()) != Some(for_update.id()) {
                return Err(ArgumentError::new("Cannot change identity for an update."));
            }
            if for_update.nodeid().is_some()
                && self.node.map(|node| node.id()) != for_update.nodeid()
            {
                return Err(ArgumentError::new(
                    "Cannot change node identity of an authenticated peer.",
                ));
            }
        }

        if self.identity.is_none() {
            if self.for_update.is_some() {
                return Err(StateError::new("Missing identity for an update."));
            }
            self.identity = Some(CryptoIdentity::random());
            self.keep_private_key = true;
        }

        let fingerprint = if self.fingerprint == 0 {
            rand::random()
        } else {
            self.fingerprint
        };

        if self.seq < 0 {
            return Err(StateError::new("Invalid sequence number"));
        }

        let payload = endpoint.len()
            + self.extra.as_ref().map_or(0, |extra| extra.len());
        if payload > Self::MAX_PAYLOAD_BYTES {
            return Err(StateError::new(format!(
                "Invalid endpoint and extra data: {payload} bytes together, longer than the {} they share",
                Self::MAX_PAYLOAD_BYTES
            )));
        }

        let private_key = if self.keep_private_key {
            self.identity.as_ref().map(|identity| {
                identity
                    .signature_key()
                    .private_key()
                    .as_bytes()
                    .to_vec()
            })
        } else {
            None
        };

        let identity = self
            .identity
            .take()
            .expect("identity is ensured above");
        PeerInfo::new(
            identity,
            private_key,
            self.node.take(),
            self.seq,
            fingerprint,
            endpoint.clone(),
            self.extra.take()
        )
    }
}

#[derive(Serialize, Deserialize)]
#[serde(into = "SerdePeerInfo", try_from = "SerdePeerInfo")]
#[derive(Clone, Debug)]
pub struct PeerInfo {
    pk: Id,
    sk: Option<Vec<u8>>,
    seq: i32,

    nodeid: Option<Id>,
    node_sig: Option<Vec<u8>>,

    sig: Vec<u8>,
    fingerprint: u64,
    endpoint: String,
    extra: Option<Vec<u8>>,
}

impl PeerInfo {
    pub const NONCE_BYTES: usize = 24;

    fn new(
        peer: CryptoIdentity,
        private_key: Option<Vec<u8>>,
        node_identity: Option<&CryptoIdentity>,
        seq: i32,
        fingerprint: u64,
        endpoint: String,
        extra: Option<Vec<u8>>,
    ) -> Result<Self> {

        let pk: Id = peer.id().clone();
        let (node_id, node_sig) = if let Some(identity) = node_identity {
            let nodeid = identity.id().clone();
            let digest = Self::node_digest(pk.as_bytes(), &nodeid, fingerprint, seq);
            let sig = identity.sign_into(digest.as_slice())?;
            (Some(nodeid), Some(sig))
        } else {
            (None, None)
        };

        let digest = Self::digest(
            pk.as_bytes(),
            seq,
            node_id.as_deref(),
            node_sig.as_deref(),
            fingerprint,
            &endpoint,
            extra.as_deref(),
        );

        Ok(Self {
            pk,
            sk: private_key,
            seq,
            nodeid: node_id,
            node_sig: node_sig,
            fingerprint,
            endpoint,
            sig: peer.sign_into(digest.as_slice())?,
            extra,
        })
    }

    pub fn builder<'a>(endpoint: impl Into<String>) -> PeerBuilder<'a> {
        PeerBuilder::new(endpoint)
    }

    pub(crate) fn packed(
        pk: Id,
        seq: i32,
        node_id: Option<Id>,
        node_sig: Option<Vec<u8>>,
        sig: Vec<u8>,
        fingerprint: u64,
        endpoint: String,
        extra: Option<Vec<u8>>,
    ) -> Self {
        Self {
            pk,
            sk: None,
            seq,
            nodeid: node_id,
            node_sig,
            sig,
            fingerprint,
            endpoint,
            extra,
        }
    }

    pub fn id(&self) -> &Id {
        &self.pk
    }

    pub fn has_private_key(&self) -> bool {
        self.sk.is_some()
    }

    pub fn private_key(&self) -> Option<&[u8]> {
        self.sk.as_deref()
    }

    pub fn sequence_number(&self) -> i32 {
        self.seq
    }

    pub fn nodeid(&self) -> Option<&Id> {
        self.nodeid.as_ref()
    }

    pub fn node_signature(&self) -> Option<&[u8]> {
        self.node_sig.as_deref()
    }

    pub fn is_authenticated(&self) -> bool {
        self.nodeid.is_some() && self.node_sig.is_some()
    }

    pub fn signature(&self) -> &[u8] {
        self.sig.as_slice()
    }

    pub fn fingerprint(&self) -> u64 {
        self.fingerprint
    }

    pub fn endpoint(&self) -> &str {
        self.endpoint.as_str()
    }

    pub fn has_extra(&self) -> bool {
        self.extra.as_ref().map(|v| !v.is_empty()).unwrap_or(false)
    }

    pub fn extra_data(&self) -> Option<&[u8]> {
        self.extra.as_deref()
    }

    pub fn extra(&self) -> Result<Option<serde_json::Map<String, serde_json::Value>>> {
        let Some(data) = self.extra.as_deref() else {
            return Ok(None);
        };
        if data.is_empty() {
            return Ok(None);
        }
        serde_json::from_slice(data)
            .map(Some)
            .map_err(|e|
                StateError::new(format!("Invalid extra data: {e}")).into()
            )
    }

    pub fn payload_size(&self) -> usize {
        self.endpoint.len() + self.extra.as_ref().map_or(0, |extra| extra.len())
    }

    pub fn without_private_key(&self) -> Self {
        if self.sk.is_none() {
            return self.clone();
        }
        let mut s = self.clone();
        s.sk = None;
        s
    }

    /// Starts an update of this peer, preserving its identity and fingerprint.
    pub fn update(&self) -> PeerBuilder<'_> {
        PeerBuilder::for_update(self.clone())
    }

    /// Compatibility helper for callers that prefer an explicit endpoint/node/extra update.
    pub fn update1(
        &self,
        endpoint: &str,
        node: Option<&CryptoIdentity>,
        extra: Option<Vec<u8>>,
    ) -> Result<Self> {
        let mut builder = self.update().with_endpoint(endpoint);
        if let Some(node) = node {
            builder = builder.with_node(node);
        }
        if let Some(extra) = extra {
            builder = builder.with_extra(&extra);
        }
        builder.build()
    }

    pub fn is_valid(&self) -> bool {
        if self.sig.len() != signature::Signature::BYTES {
            return false;
        }
        if self.seq < 0 {
            return false;
        }
        if self.endpoint.is_empty() {
            return false;
        }
        if self.payload_size() > PeerBuilder::MAX_PAYLOAD_BYTES {
            return false;
        }

        if let Some(nodeid) = self.nodeid.as_ref() {
            if self.node_sig.is_none() {
                return false;
            }
            let Some(node_sig) = self.node_sig.as_ref() else {
                return false;
            };
            if node_sig.len() != signature::Signature::BYTES {
                return false;
            }

            let node_pk = nodeid.to_signature_key();
            let digest = Self::node_digest(
                self.pk.as_bytes(),
                nodeid,
                self.fingerprint,
                self.seq
            );
            if !signature::verify(&digest, &node_sig, &node_pk).unwrap_or(false) {
                return false;
            }
        } else if self.node_sig.is_some() {
            return false;
        }

        let digest = Self::digest(
            self.pk.as_bytes(),
            self.seq,
            self.nodeid.as_ref().map(|n| n.as_bytes()),
            self.node_sig.as_ref().map(|s| s.as_slice()),
            self.fingerprint,
            &self.endpoint,
            self.extra.as_ref().map(|e| e.as_slice())
        );

        signature::verify(&digest, &self.sig, &self.pk.to_signature_key())
            .unwrap_or(false)
    }

    fn digest(
        pk: &[u8],
        seq: i32,
        nodeid: Option<&[u8]>,
        node_sig: Option<&[u8]>,
        fingerprint: u64,
        endpoint: &str,
        extra: Option<&[u8]>
    ) -> Vec<u8> {
        let mut sha = Sha256::new();
        sha.update(pk);
        sha.update(seq.to_be_bytes().as_ref());
        if let Some(nodeid) = nodeid {
            sha.update(nodeid);
            sha.update(node_sig.unwrap());
        }
        sha.update(fingerprint.to_be_bytes().as_ref());
        sha.update(endpoint.as_bytes());
        if let Some(extra) = extra {
            sha.update(extra);
        }
        sha.finalize().to_vec()
    }

    fn node_digest(pk: &[u8], nodeid: &[u8], fingerprint: u64, seq: i32) -> Vec<u8> {
        let mut sha = Sha256::new();
        sha.update(pk);
        sha.update(nodeid);
        sha.update(fingerprint.to_be_bytes());
        sha.update(seq.to_be_bytes());
        sha.finalize().to_vec()
    }
}

impl PartialEq for PeerInfo {
    fn eq(&self, other: &Self) -> bool {
        self.pk == other.pk
            && self.seq == other.seq
            && self.nodeid == other.nodeid
            && self.node_sig == other.node_sig
            && self.sig == other.sig
            && self.fingerprint == other.fingerprint
            && self.endpoint == other.endpoint
            && self.extra == other.extra
    }
}

impl Eq for PeerInfo {}

impl Hash for PeerInfo {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.pk.hash(state);
        self.seq.hash(state);
        if let Some(v) = self.nodeid.as_ref() {
            v.hash(state);
        }
        if let Some(v) = self.node_sig.as_ref() {
            v.hash(state);
        }
        self.sig.hash(state);
        self.fingerprint.hash(state);
        self.endpoint.hash(state);

        if let Some(v) = self.extra.as_ref() {
            v.hash(state);
        }
    }
}

impl fmt::Display for PeerInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{{")?;
        write!(f, "id:{}", self.pk)?;
        write!(f, ",endpoint:{}", self.endpoint)?;
        if self.fingerprint != 0 {
            write!(f, ",sn:{}", self.fingerprint)?;
        }
        if self.seq > 0 {
            write!(f, ",seq:{}", self.seq)?;
        }
        if let Some(nodeid) = self.nodeid.as_ref() {
            write!(f, ",nodeId:{}", nodeid.to_base58())?;
        }
        if let Some(node_sig) = self.node_sig.as_ref() {
            write!(f, ",nodeSig:0x{}", hex::encode(node_sig))?;
        }
        write!(f, ",sig:0x{}", hex::encode(&self.sig))?;
        write!(f, "}}")?;
        Ok(())
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SerdePeerInfo {
    #[serde(
        rename = "id",
        serialize_with = "codec::id::serialize",
        deserialize_with = "codec::id::deserialize"
    )]
    pk: Id,

    #[serde(
        rename = "seq",
        default,
        deserialize_with = "codec::seq::deserialize",
        skip_serializing_if = "crate::utils::is_default"
    )]
    seq: i32,

    #[serde(
        rename = "o",
        default,
        serialize_with = "codec::id_opt::serialize",
        deserialize_with = "codec::id_opt::deserialize",
        skip_serializing_if = "Option::is_none"
    )]
    nodeid: Option<Id>,

    #[serde(
        rename = "os",
        default,
        serialize_with = "codec::bytes_opt::serialize",
        deserialize_with = "codec::bytes_opt::deserialize",
        skip_serializing_if = "codec::is_default"
    )]
    node_sig: Option<Vec<u8>>,

    #[serde(
        rename = "sig",
        serialize_with = "codec::bytes::serialize",
        deserialize_with = "codec::bytes::deserialize"
    )]
    sig: Vec<u8>,

    #[serde(
        rename = "f",
        default,
        skip_serializing_if = "codec::is_default"
    )]
    fingerprint: u64,

    #[serde(rename = "e", skip_serializing_if = "codec::is_default")]
    endpoint: String,

    #[serde(
        rename = "ex",
        default,
        serialize_with = "codec::bytes_opt::serialize",
        deserialize_with = "codec::bytes_opt::deserialize",
        skip_serializing_if = "codec::is_default"
    )]
    extra: Option<Vec<u8>>,
}

impl From<PeerInfo> for SerdePeerInfo {
    fn from(peer: PeerInfo) -> Self {
        Self {
            pk: peer.pk,
            seq: peer.seq,
            nodeid: peer.nodeid,
            node_sig: peer.node_sig,
            sig: peer.sig,
            fingerprint: peer.fingerprint,
            endpoint: peer.endpoint,
            extra: peer.extra,
        }
    }
}

impl TryFrom<SerdePeerInfo> for PeerInfo {
    type Error = Error;

    fn try_from(sp: SerdePeerInfo) -> StdResult<Self, Self::Error> {
        if sp.endpoint.is_empty() {
            return Err(StateError::new("invalid peer: missing endpoint"));
        }
        if sp.sig.len() != signature::Signature::BYTES {
            return Err(StateError::new(format!(
                "invalid peer: invalid signature length {}, expected {}",
                sp.sig.len(),
                signature::Signature::BYTES
            )));
        }
        if sp.nodeid.is_some() != sp.node_sig.is_some() {
            return Err(StateError::new(
                "invalid peer: nodeid and node signature must both be present or absent",
            ));
        }

        let peer = PeerInfo::packed(
            sp.pk,
            sp.seq,
            sp.nodeid,
            sp.node_sig,
            sp.sig,
            sp.fingerprint,
            sp.endpoint,
            sp.extra,
        );
        if !peer.is_valid() {
            return Err(StateError::new(
                "invalid peer: signature verification failed",
            ));
        }
        Ok(peer)
    }
}

#[cfg(test)]
#[path = "unitests/test_peer_info.rs"]
mod test_peer_info;
