use core::convert::TryFrom;
use std::{env, fs, path::Path};
use serde::Deserialize;
use crate::{
    errors::{Error, Result, ArgumentError, IOError},
    signature, Id, PeerInfo,
};

pub const DEFAULT_SCHEME: &'static str = "tcp://";
pub const DEFAULT_PORT: u16 = 9090;

#[derive(Clone, Debug, Deserialize)]
#[serde(try_from = "SerdeOptions")]
pub struct Options {
    server_peerid: Id,
    // when set, skips DHT resolution of `server_peerid`.
    server_peer: Option<PeerInfo>,

    // when set, skips DHT resolution of `server_peerid`.
    service_host: Option<String>,
    service_port: u16,

    // The client identity that authenticates to the service:
    // a user identity and a per-device key.
    user_id: Id,
    user_key: Option<signature::KeyPair>,
    device_key: signature::KeyPair,

    // The upstream information, which is the local service provider that
    // the ActiveProxy will forward requests to.
    upstream_host: String,
    upstream_port: u16,
    upstream_scheme: String,

    name_access: bool,
    announce_peer: bool,
}

impl Options {
    pub fn new(peerid: Id, userid: Id) -> Self {
        Self {
            server_peerid: peerid,
            server_peer: None,
            service_host: None,
            service_port: DEFAULT_PORT,
            user_id: userid,
            user_key: None,
            device_key: signature::KeyPair::random(),
            upstream_host: "127.0.0.1".to_string(),
            upstream_port: 8080,
            upstream_scheme: DEFAULT_SCHEME.to_string(),
            name_access: false,
            announce_peer: false,
        }
    }

    pub fn parse(yaml: impl AsRef<str>) -> Result<Self> {
        let expanded_yaml = expand_environ_vars(yaml.as_ref())?;
        serde_yaml::from_str::<SerdeOptions>(&expanded_yaml)
            .map_err(|e| ArgumentError::new(format!("invalid active proxy YAML format: {e}")))?
            .try_into()
    }

    pub fn read(yaml: impl AsRef<str>) -> Result<Self> {
        Self::parse(yaml)
    }

    pub fn load(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        let yaml = fs::read_to_string(path)
            .map_err(|e| IOError::new(format!("Reading config {} failed: {e}", path.display())))?;
        Self::parse(yaml)
    }

    pub fn with_peerid(mut self, peerid: Id) -> Self {
        self.server_peerid = peerid;
        self
    }

    pub fn with_peer(mut self, peer: PeerInfo) -> Self {
        self.server_peerid = peer.id().clone();
        self.server_peer = Some(peer);
        self
    }

    pub fn with_service_host(mut self, host: impl Into<String>) -> Self {
        self.service_host = Some(host.into());
        self
    }

    pub fn with_service_port(mut self, port: u16) -> Self {
        self.service_port = port;
        self
    }

    pub fn with_userid(mut self, userid: Id) -> Self {
        self.user_id = userid;
        self
    }

    pub fn with_user_key(mut self, user_key: signature::KeyPair) -> Self {
        self.user_id = Id::from(user_key.public_key());
        self.user_key = Some(user_key);
        self
    }

    pub fn with_generated_user_key(self) -> Self {
        self.with_user_key(signature::KeyPair::random())
    }

    pub fn with_device_key(mut self, device_key: signature::KeyPair) -> Self {
        self.device_key = device_key;
        self
    }

    pub fn with_generated_device_key(self) -> Self {
        self.with_device_key(signature::KeyPair::random())
    }

    pub fn with_upstream_host(mut self, host: impl Into<String>) -> Self {
        self.upstream_host = host.into();
        self
    }

    pub fn with_upstream_port(mut self, port: u16) -> Self {
        self.upstream_port = port;
        self
    }

    pub fn with_upstream_scheme(mut self, scheme: impl Into<String>) -> Self {
        self.upstream_scheme = scheme.into();
        self
    }

    pub fn with_name_access(mut self, enabled: bool) -> Self {
        self.name_access = enabled;
        self
    }

    pub fn with_announce_peer(mut self, enabled: bool) -> Self {
        self.announce_peer = enabled;
        self
    }

    pub fn service_peerid(&self) -> &Id {
        &self.server_peerid
    }

    pub fn service_peer(&self) -> Option<&PeerInfo> {
        self.server_peer.as_ref()
    }

    pub fn service_host(&self) -> Option<&str> {
        self.service_host.as_deref()
    }

    pub fn service_port(&self) -> u16 {
        self.service_port
    }

    pub fn user_id(&self) -> &Id {
        &self.user_id
    }

    pub fn user_key(&self) -> Option<&signature::KeyPair> {
        self.user_key.as_ref()
    }

    pub fn device_key(&self) -> &signature::KeyPair {
        &self.device_key
    }

    pub fn upstream_host(&self) -> &str {
        &self.upstream_host
    }

    pub fn upstream_port(&self) -> u16 {
        self.upstream_port
    }

    pub fn upstream_scheme(&self) -> &str {
        &self.upstream_scheme
    }

    pub fn is_name_access_enabled(&self) -> bool {
        self.name_access
    }

    pub fn is_announce_peer_enabled(&self) -> bool {
        self.announce_peer
    }

    pub fn check_valid(&self) -> Result<()> {
        if self.upstream_host.is_empty() {
            return Err(ArgumentError::new("upstream_host is empty"));
        }
        if self.upstream_port == 0 {
            return Err(ArgumentError::new("upstream_port is not set"));
        }
        if self.upstream_scheme.is_empty() {
            return Err(ArgumentError::new("upstream_scheme is empty"));
        }
        Ok(())
    }
}

impl TryFrom<&str> for Options {
    type Error = Error;

    fn try_from(yaml: &str) -> Result<Self> {
        Self::parse(yaml)
    }
}

#[derive(Debug, Deserialize)]
struct SerdeOptions {
    service: SerdeService,
    client: SerdeClient,
    upstream: SerdeUpstream,
    #[serde(rename = "nameAccess", default)]
    name_access: bool,
    #[serde(rename = "announcePeer", default)]
    announce_peer: bool,
}

impl TryFrom<SerdeOptions> for Options {
    type Error = Error;

    fn try_from(options: SerdeOptions) -> Result<Self> {
        let user_key = options.client.user_private_key.as_deref().map(
            signature::PrivateKey::try_from
        ).transpose()?
        .map(signature::KeyPair::from);

        let device_sk = signature::PrivateKey::try_from(
            options.client.device_private_key.as_str()
        ).map_err(|e| {
            ArgumentError::new(&format!("failed to parse device private key: {}", e))
        })?;
        let device_key = signature::KeyPair::from(&device_sk);

        let userid_matched = user_key.as_ref().map(|k|
            options.client.user_id == Id::from(k.public_key())
        ).unwrap_or(true);

        if !userid_matched {
            return Err(ArgumentError::new("userId does not match userPrivateKey"));
        }

        Ok(Self {
            server_peerid: options.service.peer_id,
            server_peer: None,
            service_host: options.service.host,
            service_port: options.service.port.unwrap_or(DEFAULT_PORT),
            user_id: options.client.user_id,
            user_key,
            device_key,
            upstream_host: options.upstream.host,
            upstream_port: options.upstream.port,
            upstream_scheme: options
                .upstream
                .scheme
                .unwrap_or_else(|| DEFAULT_SCHEME.to_string()),
            name_access: options.name_access,
            announce_peer: options.announce_peer,
        })
    }
}

#[derive(Debug, Deserialize)]
struct SerdeService {
    #[serde(rename = "peerId")]
    peer_id: Id,
    host: Option<String>,
    port: Option<u16>,
}

#[derive(Debug, Deserialize)]
struct SerdeClient {
    #[serde(rename = "userId")]
    user_id: Id,
    #[serde(rename = "userPrivateKey")]
    user_private_key: Option<String>,
    #[serde(rename = "devicePrivateKey")]
    device_private_key: String,
}

#[derive(Debug, Deserialize)]
struct SerdeUpstream {
    host: String,
    port: u16,
    #[serde(default)]
    scheme: Option<String>,
}

fn expand_environ_vars(input: &str) -> Result<String> {
    let mut expanded = String::with_capacity(input.len());
    let mut remaining = input;

    while let Some(start) = remaining.find("${") {
        let (prefix, after) = remaining.split_at(start);
        expanded.push_str(prefix);

        let end = after.find('}').ok_or_else(|| {
            ArgumentError::new("unterminated environment variable reference")
        })?;
        let name = &after[2..end];
        if name.is_empty() {
            return Err(ArgumentError::new("empty environment variable name"));
        }
        let value = env::var(name).map_err(|_| {
            ArgumentError::new(format!(
                "environment variable `{name}` is not set"
            ))
        })?;
        expanded.push_str(&value);
        remaining = &after[end + 1..];
    }

    expanded.push_str(remaining);
    Ok(expanded)
}
