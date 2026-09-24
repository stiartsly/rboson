use std::fmt;
use url::Url;

use crate::{
    errors::{ArgumentError, Result},
    signature::{KeyPair, PrivateKey},
    Id,
};

#[derive(Clone)]
pub struct DirectorOptions {
    director_url: Url,
    insecure: bool,

    node_id: Option<Id>,
    user_id: Option<Id>,
    user_key: Option<KeyPair>,
    device_id: Option<Id>,
    device_key: Option<KeyPair>,
}

impl DirectorOptions {
    pub fn new(url: impl AsRef<str>) -> Result<Self> {
        Ok(Self {
            director_url: parse_director_url(url.as_ref())?,
            node_id: None,
            user_key: None,
            user_id: None,
            device_id: None,
            device_key: None,
            insecure: false,
        })
    }

    pub fn with_node_id(mut self, node_id: Id) -> Self {
        self.node_id = Some(node_id);
        self
    }

    pub fn with_user_id(mut self, user_id: Id) -> Self {
        self.user_id = Some(user_id);
        self.user_key = None;
        self
    }

    pub fn with_user_private_key(self, sk: PrivateKey) -> Self {
        self.with_user_keypair(KeyPair::from(sk))
    }

    pub fn with_user_keypair(mut self, key: KeyPair) -> Self {
        self.user_id = Some(Id::from(key.public_key()));
        self.user_key = Some(key);
        self
    }

    pub fn with_device_private_key(self, sk: PrivateKey) -> Self {
        self.with_device_keypair(KeyPair::from(sk))
    }

    pub fn with_device_keypair(mut self, key: KeyPair) -> Self {
        self.device_id = Some(Id::from(key.public_key()));
        self.device_key = Some(key);
        self
    }

    pub fn with_insecure(mut self, insecure: bool) -> Self {
        self.insecure = insecure;
        self
    }

    pub fn director_url(&self) -> &Url {
        &self.director_url
    }

    pub fn node_id(&self) -> Option<&Id> {
        self.node_id.as_ref()
    }

    pub fn user_id(&self) -> Option<&Id> {
        self.user_id.as_ref()
    }

    pub fn user_private_key(&self) -> Option<&PrivateKey> {
        self.user_key.as_ref().map(|kp| kp.private_key())
    }

    pub fn user_key(&self) -> Option<&KeyPair> {
        self.user_key.as_ref()
    }

    pub fn has_user_private_key(&self) -> bool {
        self.user_key.is_some()
    }

    pub fn device_id(&self) -> Option<&Id> {
        self.device_id.as_ref()
    }

    pub fn device_private_key(&self) -> Option<&PrivateKey> {
        self.device_key.as_ref().map(|kp| kp.private_key())
    }

    pub fn device_key(&self) -> Option<&KeyPair> {
        self.device_key.as_ref()
    }

    pub fn has_device_private_key(&self) -> bool {
        self.device_key.is_some()
    }

    pub fn is_insecure(&self) -> bool {
        self.insecure
    }

    pub(crate) fn check_completeness(&self) -> Result<()> {
        if self.user_id.is_none() && self.device_key.is_some() {
            return Err(ArgumentError::new(
                "A device key requires a user key or user ID",
            ));
        }
        if self.user_id.is_some() && self.user_key.is_none() && self.device_key.is_none() {
            return Err(ArgumentError::new("A user ID requires a device key"));
        }
        Ok(())
    }
}

impl fmt::Display for DirectorOptions {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let idstr = |id: &Option<Id>| {
            id.as_ref()
                .map(|v| v.to_base58())
                .unwrap_or("N/A".to_string())
        };
        write!(
            f,
            "DirectorOptions {{url:{}, node_id:{}, insecure:{}",
            self.director_url,
            idstr(&self.node_id),
            self.is_insecure()
        )?;

        write!(f, ", user_id: {}", idstr(&self.user_id))?;
        write!(f, ", has_user_private_key:{}", self.user_key.is_some())?;
        write!(f, ", has_device_private_key:{}", self.device_key.is_some())?;

        write!(f, "}}")
    }
}

fn parse_director_url(url: &str) -> Result<Url> {
    let url = Url::parse(url).map_err(|e|
        ArgumentError::new(e.to_string())
    )?;
    if !matches!(url.scheme(), "http" | "https") {
        return Err(ArgumentError::new("Director URL must use http or https"));
    }
    Ok(url)
}
