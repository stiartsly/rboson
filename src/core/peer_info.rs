use std::{
    fmt,
    result::Result as SResult,
    hash::{Hash, Hasher},
    sync::{Arc, Mutex}
};
use serde::{Serialize, Deserialize};
use sha2::{Digest, Sha256};
use unicode_normalization::UnicodeNormalization;

use super::{
    Id,
    Identity,
    signature,
    Result,
    errors::{Error, StateError},
    signature::{KeyPair, PrivateKey},
    utils,
};

pub struct PeerBuilder {
    keypair: Option<KeyPair>,
    seq: i32,
    node: Option<Arc<Mutex<dyn Identity>>>,
    fingerprint: u64,
    endpoint: String,
    extra: Option<Vec<u8>>,
}

impl PeerBuilder {
    pub fn new(endpoint: &str) -> Self {
        Self {
            keypair: None,
            seq: 0,
            node: None,
            fingerprint: 0,
            endpoint: endpoint.nfc().collect::<String>(),
            extra: None,
        }
    }

    pub fn with_extra(mut self, extra: &[u8]) -> Self {
        self.extra = Some(extra.to_vec());
        self
    }

    pub fn with_node(mut self, node: Arc<Mutex<dyn Identity>>) -> Self {
        self.node = Some(node);
        self
    }

    pub fn with_fingerprint(mut self, fingerprint: u64) -> Self {
        self.fingerprint = fingerprint;
        self
    }

    pub fn with_sequence_number(mut self, seq: i32) -> Self {
        self.seq = seq;
        self
    }

    pub fn with_key(mut self, kp: KeyPair) -> Self {
        self.keypair = Some(kp);
        self
    }

    pub fn with_private_key(mut self, sk: &[u8]) -> Result<Self> {
        self.keypair = Some(KeyPair::try_from(sk)?);
        Ok(self)
    }

    pub fn build(self) -> Result<PeerInfo> {
        if self.endpoint.is_empty() {
            return Err(StateError::new("Missing endpoint."));
        }
        if self.seq < 0 {
            return Err(StateError::new("Invalid sequence number"));
        }

        PeerInfo::new(
            self.keypair.as_ref(),
            self.node.clone(),
            self.seq,
            self.fingerprint,
            self.endpoint,
            self.extra
        )
    }
}

#[derive(Serialize, Deserialize)]
#[serde(into = "SerdePeerInfo", try_from = "SerdePeerInfo")]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PeerInfo {
    pk: Id,
    sk: Option<PrivateKey>,
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
        keypair_opt: Option<&KeyPair>,
        node_identity: Option<Arc<Mutex<dyn Identity>>>,
        seq: i32,
        fingerprint: u64,
        endpoint: String,
        extra: Option<Vec<u8>>,
    ) -> Result<Self> {
        let kp = match keypair_opt {
            Some(k) => k.clone(),
            _ => KeyPair::random(),
        };
        let pk = Id::from(kp.public_key());

        let mut nodeid: Option<Id> = None;
        let mut node_sig: Option<Vec<u8>> = None;

        if let Some(identity) = node_identity.as_ref() {
            let id = identity.lock().unwrap().id().clone();

            let mut sha = Sha256::new();
            sha.update(pk.as_bytes());
            sha.update(id.as_bytes());
            let digest = sha.finalize().to_vec();

            let sig = identity.lock().unwrap().sign_into(&digest)?;

            nodeid = Some(id);
            node_sig = Some(sig);
        }

        let mut peer = Self {
            pk,
            sk: Some(kp.to_private_key()),
            seq,
            nodeid,
            node_sig,
            fingerprint,
            endpoint,
            extra,
            sig: Vec::new(),
        };

        peer.sig = signature::sign_into(peer.digest().as_slice(), kp.private_key())?;
        Ok(peer)
    }

    pub fn builder(endpoint: &str) -> PeerBuilder {
        PeerBuilder::new(endpoint)
    }

    pub(crate) fn packed(
        pk: Id,
        seq: i32,
        nodeid: Option<Id>,
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
            nodeid,
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

    pub fn private_key(&self) -> Option<&PrivateKey> {
        self.sk.as_ref()
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

    pub fn without_private_key(&self) -> Self {
        if self.sk.is_none() {
            return self.clone();
        }
        let mut s = self.clone();
        s.sk = None;
        s
    }

    pub fn update(&self,
        endpoint: &str,
        node: Option<Arc<Mutex<dyn Identity>>>,
        extra: Option<Vec<u8>>
    ) -> Result<Self> {
        let Some(sk) = self.sk.clone() else {
            return Err(StateError::new("Not the owner of the peer info"));
        };
        if endpoint.is_empty() {
            return Err(StateError::new("Invalid endpoint"));
        }

        let endpoint_nfc = endpoint.nfc().collect::<String>();
        let extra_bytes = extra.filter(|v| !v.is_empty());

        if endpoint_nfc == self.endpoint &&
            self.nodeid.is_none() == node.is_none() &&
            self.extra == extra_bytes {
            return Ok(self.clone());
        }

        // If current has an authenticating node, validate replacement
        if let Some(nodeid) = self.nodeid.as_ref() {
            let Some(node) = node.as_ref() else {
                return Err(StateError::new("Cannot authenticate peer info without owner node"));
            };
            let borrowed = node.lock().unwrap();
            if nodeid != borrowed.id() {
                return Err(StateError::new("Cannot authenticate peer info with a different node"));
            }
        }

        let kp = KeyPair::from(sk);
        Self::new(
            Some(&kp),
            node,
            self.seq + 1,
            self.fingerprint,
            endpoint_nfc,
            extra_bytes
        )
    }

    pub fn is_valid(&self) -> bool {
        if self.sig.len() != signature::Signature::BYTES {
            return false;
        }

        if let Some(nodeid) = self.nodeid.as_ref() {
            if self.node_sig.is_none() {
                return false;
            }
            let node_sig = self.node_sig.as_ref().unwrap();
            if node_sig.len() != signature::Signature::BYTES {
                return false;
            }
            let mut sha = Sha256::new();
            sha.update(self.pk.as_bytes());
            sha.update(nodeid.as_bytes());
            let digest = sha.finalize().to_vec();

            return signature::verify(
                digest.as_slice(),
                node_sig.as_slice(),
                &nodeid.to_signature_key()
            ).unwrap_or(false)
        } else if self.node_sig.is_some() {
            return false;
        }

        signature::verify(
            self.digest().as_slice(),
            self.sig.as_slice(),
            &self.pk.to_signature_key()
        ).unwrap_or(false)
    }

    fn digest(&self) -> Vec<u8> {
        let mut sha = Sha256::new();
        sha.update(self.pk.as_bytes());
        sha.update(self.seq.to_be_bytes().as_ref());
        if let Some(nodeid) = self.nodeid.as_ref() {
            sha.update(nodeid.as_bytes());
            sha.update(self.node_sig.as_ref().unwrap().as_slice());
        }
        sha.update(self.fingerprint.to_be_bytes().as_ref());
        sha.update(self.endpoint.as_bytes());
        if let Some(extra) = self.extra.as_ref() {
            sha.update(extra.as_slice());
        }
        sha.finalize().to_vec()
    }
}

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


#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SerdePeerInfo {
    #[serde(
        rename = "id",
        serialize_with = "utils::serialize_id",
        deserialize_with = "utils::deserialize_id"
    )]
    pk: Id,

    #[serde(
        rename = "seq",
        default = "utils::default_seq",
        serialize_with = "utils::serialize_seq",
        deserialize_with = "utils::deserialize_seq",
        skip_serializing_if = "utils::is_default_seq"
    )]
    seq: i32,

    #[serde(
        rename = "o",
        default,
        serialize_with = "utils::serialize_id_opt",
        deserialize_with = "utils::deserialize_id_opt",
        skip_serializing_if = "utils::is_default"
    )]
    nodeid: Option<Id>,

    #[serde(
        rename = "os",
        default,
        serialize_with = "utils::serialize_sig_opt",
        deserialize_with = "utils::deserialize_sig_opt",
        skip_serializing_if = "utils::is_default"
    )]
    node_sig: Option<Vec<u8>>,

    #[serde(
        rename = "sig",
        serialize_with = "utils::serialize_sig",
        deserialize_with = "utils::deserialize_sig"
    )]
    sig: Vec<u8>,

     #[serde(
        rename = "f",
        default,
        skip_serializing_if = "utils::is_default"
    )]
    fingerprint: u64,

     #[serde(
        rename = "e",
        skip_serializing_if = "utils::is_default"
    )]
    endpoint: String,

     #[serde(
        rename = "ex",
        default,
        serialize_with = "utils::serialize_bytes_opt",
        deserialize_with = "utils::deserialize_bytes_opt",
        skip_serializing_if = "utils::is_default"
    )]
    extra: Option<Vec<u8>>,
}

impl fmt::Display for PeerInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
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
        Ok(())
    }
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

    fn try_from(sp: SerdePeerInfo) -> SResult<Self, Self::Error> {
        if sp.endpoint.is_empty() {
            return Err(StateError::new("invalid peer info: missing endpoint"));
        }
        if sp.sig.len() != signature::Signature::BYTES {
            return Err(StateError::new(format!(
                "invalid peer info: invalid signature length {}, expected {}",
                sp.sig.len(),
                signature::Signature::BYTES
            )));
        }
        if sp.nodeid.is_some() != sp.node_sig.is_some() {
            return Err(StateError::new(
                "invalid peer info: nodeid and node signature must both be present or absent"
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
                "invalid peer info: signature verification failed"
            ));
        }
        Ok(peer)
    }
}
