use std::{
    env, fmt, fs,
    future::Future,
    path::Path,
    pin::Pin,
    sync::Arc
};
use serde::Deserialize;
use crate::{
    errors::{ArgumentError, Error, IOError, Result},
    signature::{KeyPair, PrivateKey},
    Id, PeerInfo,
};

pub const DEFAULT_SCHEME: &'static str = "tcp://";
pub const DEFAULT_PORT: u16 = 9090;

type AnnouncePeerFuture = Pin<Box<dyn Future<Output = Result<()>> + Send + 'static>>;
#[derive(Clone)]
pub struct AnnouncePeerHandler(Arc<dyn Fn(PeerInfo) -> AnnouncePeerFuture + Send + Sync>);

impl AnnouncePeerHandler {
    pub fn new<F, Fut>(handler: F) -> Self
    where
        F: Fn(PeerInfo) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<()>> + Send + 'static,
    {
        Self(Arc::new(move |peer| Box::pin(handler(peer))))
    }

    pub(crate) async fn announce(&self, peer: PeerInfo) -> Result<()> {
        (self.0)(peer).await
    }
}

impl fmt::Debug for AnnouncePeerHandler {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("AnnouncePeerHandler(..)")
    }
}

#[derive(Clone, Debug)]
pub struct OptionsBuilder {
    server_peerid: Id,
    service_host: Option<String>,
    service_port: u16,
    user_id: Option<Id>,
    user_key: Option<KeyPair>,
    device_id: Option<Id>,
    device_key: Option<KeyPair>,
    upstream_host: Option<String>,
    upstream_port: u16,
    upstream_scheme: String,
    name_access: bool,
    announce_peer: bool,

    log_level: log::LevelFilter,
    log_file: Option<String>,
    log_console: bool,
    announce_peer_handler: Option<AnnouncePeerHandler>,
}

impl OptionsBuilder {
    pub fn new(peerid: Id) -> Self {
        Self {
            server_peerid: peerid,
            service_host: None,
            service_port: DEFAULT_PORT,
            user_id: None,
            user_key: None,
            device_id: None,
            device_key: None,
            upstream_host: None,
            upstream_port: 8080,
            upstream_scheme: DEFAULT_SCHEME.to_string(),
            name_access: false,
            announce_peer: false,

            log_level: log::LevelFilter::Info,
            log_file: None,
            log_console: true,
            announce_peer_handler: None,
        }
    }

    pub fn with_service_host(&mut self, host: impl Into<String>) -> &mut Self {
        self.service_host = Some(host.into());
        self
    }

    pub fn with_service_port(&mut self, port: u16) -> &mut Self {
        self.service_port = port;
        self
    }

    pub fn with_userid(&mut self, userid: Id) -> &mut Self {
        self.user_id = Some(userid);
        self
    }

    pub fn with_user_private_key(&mut self, private_key: PrivateKey) -> &mut Self {
        self.with_user_keypair(KeyPair::from(private_key))
    }

    pub fn with_user_keypair(&mut self, user_key: KeyPair) -> &mut Self {
        self.user_id = Some(Id::from(user_key.public_key()));
        self.user_key = Some(user_key);
        self
    }

    pub fn with_generated_user_key(&mut self) -> &mut Self {
        self.with_user_keypair(KeyPair::random())
    }

    pub fn with_device_private_key(&mut self, private_key: PrivateKey) -> &mut Self {
        self.with_device_keypair(KeyPair::from(private_key))
    }

    pub fn with_device_keypair(&mut self, device_key: KeyPair) -> &mut Self {
        self.device_id = Some(Id::from(device_key.public_key()));
        self.device_key = Some(device_key);
        self
    }

    pub fn with_generated_device_key(&mut self) -> &mut Self {
        self.with_device_keypair(KeyPair::random())
    }

    pub fn with_upstream_host(&mut self, host: impl Into<String>) -> &mut Self {
        self.upstream_host = Some(host.into());
        self
    }

    pub fn with_upstream_port(&mut self, port: u16) -> &mut Self {
        self.upstream_port = port;
        self
    }

    pub fn with_upstream_scheme(&mut self, scheme: impl Into<String>) -> &mut Self {
        self.upstream_scheme = scheme.into();
        self
    }

    pub fn with_name_access(&mut self, enabled: bool) -> &mut Self {
        self.name_access = enabled;
        self
    }

    pub fn with_announce_peer(&mut self, enabled: bool) -> &mut Self {
        self.announce_peer = enabled;
        self
    }

    pub fn with_announce_peer_handler(&mut self, handler: AnnouncePeerHandler) -> &mut Self {
        self.announce_peer_handler = Some(handler);
        self
    }

    pub fn with_log_level(&mut self, level: log::LevelFilter) -> &mut Self {
        self.log_level = level;
        self
    }

    pub fn with_log_file(&mut self, path: impl Into<String>) -> &mut Self {
        self.log_file = Some(path.into());
        self
    }

    pub fn with_log_console(&mut self, enabled: bool) -> &mut Self {
        self.log_console = enabled;
        self
    }

    pub fn build(&self) -> Result<Options> {
        let service_host = self
            .service_host
            .as_deref()
            .filter(|host| !host.trim().is_empty())
            .ok_or_else(|| ArgumentError::new("service_host is required"))?;

        let user_id = self
            .user_id
            .as_ref()
            .ok_or_else(|| ArgumentError::new("user_id is required"))?;
        let device_id = self
            .device_id
            .as_ref()
            .ok_or_else(|| ArgumentError::new("device_id is required"))?;
        let device_key = self
            .device_key
            .as_ref()
            .ok_or_else(|| ArgumentError::new("device_key is required"))?;

        let upstream_host = self
            .upstream_host
            .as_deref()
            .filter(|host| !host.trim().is_empty())
            .ok_or_else(|| ArgumentError::new("upstream_host is required"))?;
        if self.upstream_port == 0 {
            return Err(ArgumentError::new("upstream_port must not be zero"));
        }
        if self.upstream_scheme.trim().is_empty() {
            return Err(ArgumentError::new("upstream_scheme is required"));
        }

        Ok(Options {
            server_peerid: self.server_peerid.clone(),
            service_host: service_host.to_string(),
            service_port: self.service_port,
            user_id: user_id.clone(),
            device_id: device_id.clone(),
            device_key: device_key.clone(),
            upstream_host: upstream_host.to_string(),
            upstream_port: self.upstream_port,
            upstream_scheme: self.upstream_scheme.clone(),
            name_access: self.name_access,
            announce_peer: self.announce_peer,
            log_level: self.log_level,
            log_file: self.log_file.clone(),
            log_console: self.log_console,
            announce_peer_handler: self.announce_peer_handler.clone(),
        })
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(try_from = "SerdeOptions")]
pub struct Options {
    server_peerid: Id,

    // when set, skips DHT resolution of `server_peerid`.
    service_host: String,
    service_port: u16,

    // The client identity that authenticates to the service:
    // a user identity and a per-device key.
    user_id: Id,
    device_id: Id,
    device_key: KeyPair,

    // The upstream information, which is the local service provider that
    // the ActiveProxy will forward requests to.
    upstream_host: String,
    upstream_port: u16,
    upstream_scheme: String,

    name_access: bool,
    announce_peer: bool,
    log_level: log::LevelFilter,
    log_file: Option<String>,
    log_console: bool,
    announce_peer_handler: Option<AnnouncePeerHandler>,
}

impl Options {
    pub fn builder(peerid: Id) -> OptionsBuilder {
        OptionsBuilder::new(peerid)
    }

    pub fn with_announce_peer_handler(mut self, handler: AnnouncePeerHandler) -> Self {
        self.announce_peer_handler = Some(handler);
        self
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

    pub fn service_peerid(&self) -> &Id {
        &self.server_peerid
    }

    pub fn service_host(&self) -> &str {
        &self.service_host
    }

    pub fn service_port(&self) -> u16 {
        self.service_port
    }

    pub fn user_id(&self) -> &Id {
        &self.user_id
    }

    pub fn device_id(&self) -> &Id {
        &self.device_id
    }

    pub fn device_private_key(&self) -> &PrivateKey {
        self.device_key.private_key()
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

    pub fn log_level(&self) -> log::LevelFilter {
        self.log_level
    }

    pub fn log_file(&self) -> Option<&str> {
        self.log_file.as_deref()
    }

    pub fn log_console(&self) -> bool {
        self.log_console
    }

    pub(crate) fn announce_peer_handler(&self) -> Option<AnnouncePeerHandler> {
        self.announce_peer_handler.clone()
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
    #[serde(rename = "logLevel", default)]
    log_level: Option<String>,
    #[serde(rename = "logFile", default)]
    log_file: Option<String>,
    #[serde(rename = "logConsole", default)]
    log_console: Option<bool>,
}

impl TryFrom<SerdeOptions> for Options {
    type Error = Error;

    fn try_from(sopts: SerdeOptions) -> Result<Self> {
        let mut b = Options::builder(sopts.service.peer_id);
        b.with_userid(sopts.client.user_id);

        let user_key = sopts
            .client
            .user_private_key
            .as_deref()
            .map(PrivateKey::try_from)
            .transpose()?
            .map(KeyPair::from);

        if let Some(key) = user_key {
            if sopts.client.user_id != Id::from(key.public_key()) {
                return Err(ArgumentError::new("userId does not match userPrivateKey"));
            }
            b.with_user_keypair(key);
        }

        let device_sk = sopts
            .client
            .device_private_key
            .as_deref()
            .map(PrivateKey::try_from)
            .transpose()?
            .map(KeyPair::from);

        if let Some(key) = device_sk {
            b.with_device_keypair(key);
        }

        let service_host = sopts
            .service
            .host
            .ok_or_else(|| ArgumentError::new("service.host is required"))?;
        b.with_service_host(service_host);
        b.with_service_port(sopts.service.port.unwrap_or(DEFAULT_PORT));

        b.with_upstream_host(sopts.upstream.host);
        b.with_upstream_port(sopts.upstream.port);
        b.with_upstream_scheme(
            sopts
                .upstream
                .scheme
                .unwrap_or_else(|| DEFAULT_SCHEME.to_string()),
        );
        b.with_name_access(sopts.name_access);
        b.with_announce_peer(sopts.announce_peer);

        let log_level = sopts.log_level.unwrap_or("info".to_string())
            .parse::<log::LevelFilter>()
            .map_err(|e| ArgumentError::new(format!("invalid logLevel: {}", e)))?;
        b.with_log_level(log_level);

        if let Some(log_file) = sopts.log_file {
            b.with_log_file(log_file);
        }
        let log_console = sopts.log_console.unwrap_or(true);
        b.with_log_console(log_console);
        b.build()
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
    #[serde(rename = "userPrivateKey", default)]
    user_private_key: Option<String>,
    #[serde(rename = "devicePrivateKey", default)]
    device_private_key: Option<String>,
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

        let end = after
            .find('}')
            .ok_or_else(|| ArgumentError::new("unterminated environment variable reference"))?;
        let name = &after[2..end];
        if name.is_empty() {
            return Err(ArgumentError::new("empty environment variable name"));
        }
        let value = env::var(name)
            .map_err(|_| ArgumentError::new(format!("environment variable `{name}` is not set")))?;
        expanded.push_str(&value);
        remaining = &after[end + 1..];
    }

    expanded.push_str(remaining);
    Ok(expanded)
}
