use std::fmt;
use std::mem;
use std::hash::{Hash, Hasher};
use unicode_normalization::UnicodeNormalization;

use crate::{
    Id,
    id::ID_BYTES,
    signature,
    signature::{KeyPair, PrivateKey}
};

// No signature field here, which would be calculated during creation.
#[derive(Clone)]
pub struct PeerBuilder<'a> {
    keypair: Option<&'a KeyPair>,
    nodeid: &'a Id,
    origin: Option<&'a Id>,
    port: u16,
    url: Option<&'a str>,
}

impl<'a> PeerBuilder<'a> {
    pub fn new(nodeid: &'a Id) -> Self {
        Self {
            keypair: None,
            nodeid,
            origin: None,
            port: 0,
            url: None,
        }
    }

    pub fn with_keypair(&mut self, keypair: &'a KeyPair) -> &mut Self {
        self.keypair = Some(keypair);
        self
    }

    pub fn with_nodeid(&mut self, nodeid: &'a Id) -> &mut Self {
        self.nodeid = nodeid;
        self
    }

    pub fn with_origin(&mut self, origin: &'a Id) -> &mut Self {
        self.origin = Some(origin);
        self
    }

    pub fn with_port(&mut self, port: u16) -> &mut Self {
        self.port = port;
        self
    }

    pub fn with_alternative_url(&mut self, alternative_url: &'a str) -> &mut Self {
        self.url = Some(alternative_url);
        self
    }

    pub fn build(&self) -> PeerInfo {
        PeerInfo::new(self)
    }
}

pub(crate) struct PackBuilder {
    pk: Option<Id>,
    nodeid: Id,
    origin: Option<Id>,
    sk: Option<PrivateKey>,
    port: u16,
    url: Option<String>,
    sig: Option<Vec<u8>>,
}

impl PackBuilder {
    pub(crate) fn new(nodeid: Id) -> Self {
        Self {
            pk: None,
            nodeid,
            origin: None,
            sk: None,
            port: 0,
            url: None,
            sig: None,
        }
    }

    pub(crate) fn with_peerid(mut self, id: Option<Id>) -> Self {
        self.pk = id;
        self
    }

    pub(crate) fn with_origin(mut self, origin: Option<Id>) -> Self {
        self.origin = origin;
        self
    }

    pub(crate) fn with_sk(mut self, sk: Option<PrivateKey>) -> Self {
        self.sk = sk;
        self
    }

    pub(crate) fn with_port(mut self, port: u16) -> Self {
        self.port = port;
        self
    }

    pub(crate) fn with_url(mut self, url: Option<String>) -> Self {
        self.url = url;
        self
    }

    pub(crate) fn with_sig(mut self, sig: Option<Vec<u8>>) -> Self {
        self.sig = sig;
        self
    }

    pub fn build(self) -> PeerInfo {
        assert!(self.pk.is_some());
        assert!(self.sig.is_some());
        PeerInfo::packed(self)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PeerInfo {
    pk: Id,
    sk: Option<PrivateKey>,
    nodeid: Id,
    origin: Option<Id>,
    port: u16,
    url: Option<String>,
    sig: Vec<u8>,
}

impl PeerInfo {
    fn new(b: &PeerBuilder) -> Self {
        let kp = match b.keypair.as_ref() {
            Some(v) => v,
            None => &KeyPair::random()
        };

        let mut peer = PeerInfo {
            pk: Id::from(kp.to_public_key()),
            sk: Some(kp.to_private_key()),
            nodeid: b.nodeid.clone(),
            origin: b.origin.map(|v|v.clone()),
            port: b.port,
            url: b.url.map(|v| v.nfc().collect::<String>()),
            sig: Vec::new(),
        };

        peer.sig = signature::sign_into(
            &peer.serialize_signature_data(),
            peer.sk.as_ref().unwrap()
        ).unwrap();
        peer
    }

    fn packed(mut b: PackBuilder) -> Self {
        PeerInfo {
            pk: b.pk.take().unwrap(),
            sk: b.sk.take(),
            nodeid: mem::take(&mut b.nodeid),
            origin: b.origin.take(),
            port: b.port,
            url: b.url.take().map(|v|v.nfc().collect::<String>()),
            sig: b.sig.take().unwrap(),
        }
    }

    pub const fn id(&self) -> &Id {
        &self.pk
    }

    pub const fn has_private_key(&self) -> bool {
        self.sk.is_some()
    }

    pub const fn private_key(&self) -> Option<&PrivateKey> {
        self.sk.as_ref()
    }

    pub const fn nodeid(&self) -> &Id {
        &self.nodeid
    }

    pub fn origin(&self) -> &Id {
        self.origin.as_ref().unwrap_or_else(|| self.nodeid())
    }

    pub const fn port(&self) -> u16 {
        self.port
    }

    pub const fn has_alternative_url(&self) -> bool {
        self.url.is_some()
    }

    pub fn alternative_url(&self) -> Option<&str> {
        self.url.as_ref().map(|v| v.as_str())
    }

    pub fn signature(&self) -> &[u8] {
        &self.sig
    }

    pub fn is_delegated(&self) -> bool {
        self.origin.is_some()
    }

    pub fn is_valid(&self) -> bool {
        assert_eq!(
            self.sig.len(),
            signature::Signature::BYTES,
            "Invalid signature data length {}, should be {}",
            self.sig.len(),
            signature::Signature::BYTES
        );

        signature::verify(
            self.serialize_signature_data().as_ref(),
            self.sig.as_slice(),
            &self.pk.to_signature_key()
        ).is_ok()
    }

    pub(crate) fn serialize_signature_data(&self) -> Vec<u8> {
        let len = {
            let mut sz = 0;
            sz += ID_BYTES * 2;            // nodeid and origin.
            sz += mem::size_of::<u16>();   // padding port
            sz += self.url.as_ref().map_or(0, |v|v.len());
            sz
        };

        let mut data = vec![0u8; len];
        data.extend_from_slice(self.nodeid.as_bytes());
        data.extend_from_slice(self.origin().as_bytes());
        data.extend_from_slice(self.port.to_be_bytes().as_ref());

        if let Some(url) = self.url.as_ref() {
            data.extend_from_slice(url.as_ref());
        }
        data
    }
}

impl Hash for PeerInfo {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.pk.hash(state);
        self.nodeid.hash(state);
        self.origin().hash(state);
        self.port.hash(state);
        self.url.hash(state);
        self.sig.hash(state);
    }
}

impl fmt::Display for PeerInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{},{},", self.pk, self.nodeid)?;
        if self.is_delegated() {
            write!(f, "{},", self.origin())?;
        }
        write!(f, "{}", self.port)?;
        if let Some(url) = self.url.as_ref() {
            write!(f, ",{}", url)?;
        }
        Ok(())
    }
}
