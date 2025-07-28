use std::fmt;
use std::mem;
use std::hash::{Hash, Hasher};

use sha2::{Digest, Sha256};
use unicode_normalization::UnicodeNormalization;
use ciborium::Value;

use super::{
    Id,
    signature,
    signature::{KeyPair, PrivateKey}
};

// No signature field here, which would be calculated on creation.
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

    pub fn with_keypair(&mut self, keypair: Option<&'a KeyPair>) -> &mut Self {
        self.keypair = keypair;
        self
    }

    pub fn with_origin(&mut self, origin: Option<&'a Id>) -> &mut Self {
        self.origin = origin;
        self
    }

    pub fn with_port(&mut self, port: u16) -> &mut Self {
        self.port = port;
        self
    }

    pub fn with_alternative_url(&mut self, alternative_url: Option<&'a str>) -> &mut Self {
        self.url = alternative_url;
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
            &peer.digest(),
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

    #[allow(dead_code)]
    pub(crate) fn from_cbor(input: &Value) -> Option<Self> {
        let mut pk: Option<Id> = None;
        let mut nodeid: Option<Id> = None;
        let mut url: Option<String> = None;
        let mut sig: Option<Vec<u8>> = None;
        let mut port = 0;

        let root = input.as_map()?;
        for (k,v) in root {
            let k = k.as_text()?;
            match k {
                "id" => pk = Some(Id::from_cbor(v)?),
                "nodeid" => nodeid = Some(Id::from_cbor(v)?),
                "port" => port = v.as_integer()?.try_into().unwrap(),
                "url" => url = v.as_text().map(|v|v.to_string()),
                "sig" => sig = Some(v.as_bytes()?.to_vec()),
                _ => return None,
            }
        }

        let Some(nodeid) = nodeid else {
            return None;
        };
        let Some(pk) = pk else {
            return None;
        };
        let Some(sig) = sig else {
            return None;
        };

        Some(PackBuilder::new(nodeid)
            .with_peerid(Some(pk))
            .with_port(port)
            .with_url(url.map(|v|v.to_string()))
            .with_sig(Some(sig.to_vec()))
            .build())
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
        self.url.as_deref()
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
            self.digest().as_ref(),
            self.sig.as_slice(),
            &self.pk.to_signature_key()
        ).is_ok()
    }

    pub(crate) fn digest(&self) -> Vec<u8> {
        let mut sha256 = Sha256::new();
        sha256.update(self.pk.as_bytes());
        sha256.update(self.nodeid.as_bytes());
        if let Some(origin) = self.origin.as_ref() {
            sha256.update(origin.as_bytes())
        }
        sha256.update(self.port.to_be_bytes().as_ref());
        if let Some(url) = self.url.as_ref() {
            sha256.update(url.nfc().collect::<String>().as_bytes());
        };
        sha256.finalize().to_vec()
    }

    #[allow(unused)]
    pub(crate) fn to_cbor(&self) -> Value {
        Value::Map(vec![
            (
                Value::Text(String::from("id")),
                self.id().to_cbor(),
            ),
            (
                Value::Text(String::from("nodeid")),
                self.nodeid().to_cbor(),
            ),
            (
                Value::Text(String::from("port")),
                Value::Integer(self.port().into()),
            ),
            (
                Value::Text(String::from("url")),
                self.alternative_url().map_or(
                    Value::Null,
                    |url| Value::Text(url.to_string())
                ),
            ),
            (
                Value::Text(String::from("sig")),
                Value::Bytes(self.signature().to_vec())
            )
        ])
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
