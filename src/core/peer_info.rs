use std::fmt;
use std::hash::{Hash, Hasher};
use std::sync::{Arc, Mutex};
use serde::{
    Serialize, Deserialize, Serializer, Deserializer,
    ser::SerializeTuple,
    de::{self, Visitor, SeqAccess}
};

use sha2::{Digest, Sha256};
use unicode_normalization::UnicodeNormalization;
use rand::RngCore;

use super::{
    Id,
    error::{Error, Result},
    Identity,
    signature,
    signature::{KeyPair, PrivateKey},
};

pub struct PeerBuilder {
    keypair: Option<KeyPair>,
    nonce: Option<Vec<u8>>,
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
            nonce: None,
            seq: 0,
            node: None,
            fingerprint: 0,
            endpoint: endpoint.nfc().collect::<String>(),
            extra: None,
        }
    }

    pub fn with_nonce(mut self, nonce: &[u8]) -> Self {
        self.nonce = Some(nonce.to_vec());
        self
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
            return Err(Error::State("Missing endpoint.".into()));
        }
        if self.seq < 0 {
            return Err(Error::State("Invalid sequence number".into()));
        }
        if let Some(nonce) = self.nonce.as_ref() {
            if nonce.len() != PeerInfo::NONCE_BYTES {
                return Err(Error::State(format!("Invalid nonce length {}, expected {}", nonce.len(), PeerInfo::NONCE_BYTES)));
            }
        }


        PeerInfo::new(
            self.keypair.as_ref(),
            self.node.clone(),
            self.nonce.as_ref().map(|v| v.as_slice()),
            self.seq,
            self.fingerprint,
            self.endpoint,
            self.extra
        )
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PeerInfo {
    pk: Id,
    sk: Option<PrivateKey>,
    nonce: Vec<u8>,
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
        node: Option<Arc<Mutex<dyn Identity>>>,
        nounce: Option<&[u8]>,
        seq: i32,
        fingerprint: u64,
        endpoint: String,
        extra: Option<Vec<u8>>,
    ) -> Result<Self> {
        let kp = match keypair_opt {
            Some(k) => k.clone(),
            None => KeyPair::random(),
        };

        let pk = Id::from(kp.public_key());
        let nonce = if let Some(nonce) = nounce {
            if nonce.len() != Self::NONCE_BYTES {
                return Err(Error::State(format!("Invalid nonce length {}, expected {}", nonce.len(), Self::NONCE_BYTES)));
            }
            nonce.to_vec()
        } else {
            let mut nonce = vec![0u8; Self::NONCE_BYTES];
            rand::thread_rng().fill_bytes(&mut nonce);
            nonce
        };

        let mut nodeid: Option<Id> = None;
        let mut node_sig: Option<Vec<u8>> = None;

        if let Some(node_arc) = node.as_ref() {
            let id = node_arc.lock().unwrap().id().clone();

            let mut sha = Sha256::new();
            sha.update(pk.as_bytes());
            sha.update(id.as_bytes());
            sha.update(nonce.as_slice());
            let digest = sha.finalize().to_vec();

            let sig = node_arc.lock().unwrap().sign_into(digest.as_slice())?;

            nodeid = Some(id);
            node_sig = Some(sig);
        }

        let mut peer = PeerInfo {
            pk: pk.clone(),
            sk: Some(kp.to_private_key()),
            nonce: nonce.clone(),
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

    #[allow(unused)]
    pub(crate) fn packed(
        pk: Id,
        nonce: Vec<u8>,
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
            nonce,
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

    pub fn nonce(&self) -> &[u8] {
        self.nonce.as_slice()
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
        if self.sk.is_none() {
            return Err(Error::State("Not the owner of the peer info".into()));
        }
        if endpoint.is_empty() {
            return Err(Error::State("Invalid endpoint".into()));
        }

        let endpoint_nfc = endpoint.nfc().collect::<String>();
        let extra_bytes = extra.filter(|v| !v.is_empty());

        if endpoint_nfc == self.endpoint &&
            self.nodeid.is_none() == node.is_none() &&
            self.extra == extra_bytes {
            return Ok(self.clone());
        }

        // If current has an authenticating node, validate replacement
        if self.nodeid.is_some() {
            if node.is_none() {
                return Err(Error::State("Cannot authenticate peer info without owner node".into()));
            }
            let node_id = node.as_ref().unwrap().lock().unwrap().id().clone();
            if node_id != self.nodeid.clone().unwrap() {
                return Err(Error::State("Cannot authenticate peer info with a different node".into()));
            }
        }

        let sequence_number = self.seq + 1;
        let sk = self.sk.as_ref().unwrap();
        let kp = KeyPair::from(sk);

        PeerInfo::new(
            Some(&kp),
            node,
            None,
            sequence_number,
            self.fingerprint,
            endpoint_nfc,
            extra_bytes
        )
    }

    pub fn is_valid(&self) -> bool {
        if self.sig.len() != signature::Signature::BYTES {
            return false;
        }
        if self.nonce.len() != Self::NONCE_BYTES {
            return false;
        }

        if let Some(nodeid) = self.nodeid.as_ref() {
            if self.node_sig.is_none() {
                return false;
            }
            let mut sha = Sha256::new();
            sha.update(self.pk.as_bytes());
            sha.update(nodeid.as_bytes());
            sha.update(self.nonce.as_slice());
            let digest = sha.finalize().to_vec();

            return signature::verify(
                digest.as_slice(),
                self.node_sig.as_ref().unwrap().as_slice(),
                &nodeid.to_signature_key()
            ).is_ok()
        } else if self.node_sig.is_some() {
            return false;
        }

        signature::verify(
            self.digest().as_slice(),
            self.sig.as_slice(),
            &self.pk.to_signature_key()
        ).is_ok()
    }

    fn digest(&self) -> Vec<u8> {
        let mut sha = Sha256::new();
        sha.update(self.pk.as_bytes());
        sha.update(self.nonce.as_slice());
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
        self.nonce.hash(state);
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
        write!(f, "id:{}", self.pk)?;
        write!(f, ",endpoint:{}", self.endpoint)?;
        if self.fingerprint != 0 { write!(f, ",sn:{}", self.fingerprint)?; }
        if self.seq > 0 { write!(f, ",seq:{}", self.seq)?; }
        if let Some(nodeid) = self.nodeid.as_ref() {
            write!(f, ",nodeId:{}", nodeid.to_base58())?;
        }
        if let Some(node_sig) = self.node_sig.as_ref() {
            write!(f, ",nodeSig:{}", hex::encode(node_sig))?;
        }
        write!(f, ",sig:{}", hex::encode(&self.sig))?;
        Ok(())
    }
}

impl Serialize for PeerInfo {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut s = serializer.serialize_tuple(9)?;
        s.serialize_element(&self.pk)?;
        s.serialize_element(&self.nonce)?;
        if self.seq >= 0 {
            s.serialize_element(&self.seq)?;
        }
        if let Some(nodeid) = self.nodeid.as_ref() {
            s.serialize_element(nodeid)?;
            s.serialize_element(self.node_sig.as_ref().unwrap())?;
        }
        s.serialize_element(&self.sig)?;
        s.serialize_element(&self.fingerprint)?;
        s.serialize_element(&self.endpoint)?;
        if let Some(extra) = self.extra.as_ref() {
            s.serialize_element(extra)?;
        }
        s.end()
    }
}

impl<'de> Deserialize<'de> for PeerInfo {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct PeerVisitor;

        impl<'de> Visitor<'de> for PeerVisitor {
            type Value = PeerInfo;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("peer info tuple")
            }

            fn visit_seq<A>(self, mut seq: A) -> std::result::Result<Self::Value, A::Error>
            where
                A: SeqAccess<'de>,
            {
                let pk: Id = seq.next_element()?
                    .ok_or_else(|| de::Error::invalid_length(0, &"9 elements"))?;
                let nonce: Vec<u8> = seq.next_element()?
                    .ok_or_else(|| de::Error::invalid_length(1, &"9 elements"))?;
                let seqno: i32 = seq.next_element()?
                    .ok_or_else(|| de::Error::invalid_length(2, &"9 elements"))?;
                let nodeid: Option<Id> = seq.next_element()?;
                let node_sig: Option<Vec<u8>> = seq.next_element()?
                    .ok_or_else(|| de::Error::invalid_length(4, &"9 elements"))?;
                let sig: Vec<u8> = seq.next_element()?
                    .ok_or_else(|| de::Error::invalid_length(5, &"9 elements"))?;
                let fingerprint: u64 = seq.next_element()?
                    .ok_or_else(|| de::Error::invalid_length(6, &"9 elements"))?;
                let endpoint: String = seq.next_element()?
                    .ok_or_else(|| de::Error::invalid_length(7, &"9 elements"))?;
                let extra: Option<Vec<u8>> = seq.next_element()?
                    .ok_or_else(|| de::Error::invalid_length(8, &"9 elements"))?;

                Ok(PeerInfo::packed(
                    pk,
                    nonce,
                    seqno,
                    nodeid,
                    node_sig,
                    sig,
                    fingerprint,
                    endpoint,
                    extra
                ))
            }
        }
        deserializer.deserialize_tuple(9, PeerVisitor)
    }
}
