use crate::{
    PeerInfo,
    signature,
    Id,
    Result,
    errors::ArgumentError,
};

pub struct Options {
    server_peerid   : Id,
    // when set, skips DHT resolution of `server_peerid`.
    server_peer     : Option<PeerInfo>,

    // when set, skips DHT resolution of `server_peerid`.
    service_host    : Option<String>,
    service_port    : u16,

    // The client identity that authenticates to the service:
    // a user identity and a per-device key.
    user_id         : Id,
    user_key        : Option<signature::KeyPair>,
    device_key      : signature::KeyPair,

    // The upstream information, which is the local service provider that
    // the ActiveProxy will forward requests to.
    upstream_host   : String,
    upstream_port   : u16,
    upstream_scheme : String,

    name_access     : bool,
    announce_peer   : bool,

    // The file path to cache the peer information for the service peer
    data_dir       : String,
}

impl Options {
    const DEFAULT_SCHEME: &'static str = "tcp://";
    const DEFAULT_PORT: u16 = 9090;

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

    pub fn data_dir(&self) -> &str {
        &self.data_dir
    }
}

pub struct OptionsBuilder {
    data_dir        : Option<String>,
    server_peerid   : Id,
    server_peer     : Option<PeerInfo>,
    service_host    : Option<String>,
    service_port    : Option<u16>,

    user_id         : Option<Id>,
    user_key        : Option<signature::KeyPair>,
    device_key      : Option<signature::KeyPair>,

    upstream_host   : Option<String>,
    upstream_port   : Option<u16>,
    upstream_scheme : String,

    name_access     : bool,
    announce_peer   : bool,
}

impl OptionsBuilder {
    pub fn new(peerid: Id) -> Self {
        Self {
            data_dir        : None,
            server_peerid   : peerid,
            server_peer     : None,
            service_host    : None,
            service_port    : None,

            user_id         : None,
            user_key        : None,
            device_key      : None,

            upstream_host   : None,
            upstream_port   : None,
            upstream_scheme : Options::DEFAULT_SCHEME.to_string(),

            name_access     : false,
            announce_peer   : false,
        }
    }

    pub fn with_data_dir(mut self, data_dir: impl Into<String>) -> Self {
        self.data_dir = Some(data_dir.into());
        self
    }

    pub fn with_service_peerid(mut self, peerid: Id) -> Self {
        self.server_peerid = peerid;
        self.server_peer = None;
        self
    }

    pub fn with_service_peer(mut self, peer: PeerInfo) -> Self {
        self.server_peerid = peer.id().clone();
        self.server_peer = Some(peer);
        self
    }

    pub fn with_service(mut self, peerid: Id, host: impl Into<String>, port: u16) -> Self {
        self.server_peerid = peerid;
        self.server_peer = None;
        self.service_host = Some(host.into());
        self.service_port = Some(port);
        self
    }

    pub fn with_service_host(mut self, host: impl Into<String>) -> Self {
        self.service_host = Some(host.into());
        self
    }

    pub fn with_service_port(mut self, port: u16) -> Self {
        self.service_port = Some(port);
        self
    }

    // Sets the client identity by user id only; clears any previously set user key.
    pub fn with_user_id(mut self, user_id: Id) -> Self {
        self.user_id = Some(user_id);
        self.user_key = None;
        self
    }

    // Sets the client identity by user key; the user id is derived from it.
    pub fn with_user_key(mut self, user_key: signature::KeyPair) -> Self {
        self.user_id = Some(Id::from(user_key.public_key()));
        self.user_key = Some(user_key);
        self
    }

    pub fn with_generated_user_key(self) -> Self {
        let user_key = signature::KeyPair::random();
        self.with_user_key(user_key)
    }

    pub fn with_device_key(mut self, device_key: signature::KeyPair) -> Self {
        self.device_key = Some(device_key);
        self
    }

    pub fn with_upstream_host(mut self, host: impl Into<String>) -> Self {
        self.upstream_host = Some(host.into());
        self
    }

    pub fn with_upstream_port(mut self, port: u16) -> Self {
        self.upstream_port = Some(port);
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

    pub fn build(mut self) -> Result<Options> {
        let Some(upstream_host) = self.upstream_host.take() else {
            return Err(ArgumentError::new("upstream_host is required"));
        };
        let Some(upstream_port) = self.upstream_port.take() else {
            return Err(ArgumentError::new("upstream_port is required"));
        };
        let Some(user_id) = self.user_id.take() else {
            return Err(ArgumentError::new("user_id (or user_key) is required"));
        };
        let Some(device_key) = self.device_key.take() else {
            return Err(ArgumentError::new("device_key is required"));
        };

        Ok(Options {
            data_dir        : self.data_dir.take().unwrap_or_else(|| ".".into()),
            server_peerid   : self.server_peerid,
            server_peer     : self.server_peer.take(),
            service_host    : self.service_host.take(),
            service_port    : self.service_port.unwrap_or(Options::DEFAULT_PORT),
            user_id,
            user_key        : self.user_key.take(),
            device_key,
            upstream_host,
            upstream_port,
            upstream_scheme : self.upstream_scheme,
            name_access     : self.name_access,
            announce_peer   : self.announce_peer,
        })
    }
}

