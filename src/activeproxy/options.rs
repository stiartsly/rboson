use crate::{
    signature,
    Id,
};

pub struct ActiveProxyOptions {
    data_dir        : String,
    server_peerid   : Id,

    upstream_peer_private_key: Option<signature::PrivateKey>,
    upstream_domain : Option<String>,
    upstream_host   : String,
    upstream_port   : u16,
}

impl ActiveProxyOptions {
    pub fn data_dir(&self) -> &str {
        &self.data_dir
    }

    pub fn server_peerid(&self) -> &Id {
        &self.server_peerid
    }

    pub fn upstream_peer_private_key(&self) -> Option<&signature::PrivateKey> {
        self.upstream_peer_private_key.as_ref()
    }

    pub fn upstream_domain(&self) -> Option<&str> {
        self.upstream_domain.as_deref()
    }

    pub fn upstream_host(&self) -> &str {
        &self.upstream_host
    }

    pub fn upstream_port(&self) -> u16 {
        self.upstream_port
    }
}

#[derive(Debug)]
pub struct ActiveProxyOptionsBuilder {
    data_dir        : Option<String>,
    server_peerid   : Id,

    upstream_peer_private_key: Option<signature::PrivateKey>,
    upstream_domain : Option<String>,
    upstream_host   : Option<String>,
    upstream_port   : Option<u16>,
}

impl ActiveProxyOptionsBuilder {
    pub fn new(peerid: Id) -> Self {
        Self {
            data_dir: None,
            server_peerid: peerid,
            upstream_peer_private_key: None,
            upstream_domain: None,
            upstream_host: None,
            upstream_port: None,
        }
    }

    pub fn with_data_dir(mut self, data_dir: &str) -> Self {
        self.data_dir = Some(data_dir.to_string());
        self
    }

    pub fn with_upstream_peer_private_key(mut self, private_key: signature::PrivateKey) -> Self {
        self.upstream_peer_private_key = Some(private_key);
        self
    }

    pub fn with_upstream_domain(mut self, domain: &str) -> Self {
        self.upstream_domain = Some(domain.to_string());
        self
    }

    pub fn with_upstream_host(mut self, host: &str) -> Self {
        self.upstream_host = Some(host.to_string());
        self
    }

    pub fn with_upstream_port(mut self, port: u16) -> Self {
        self.upstream_port = Some(port);
        self
    }

    pub fn build(self) -> Result<ActiveProxyOptions, String> {
        let data_dir = self.data_dir.ok_or("data_dir is required")?;
        let upstream_host = self.upstream_host.ok_or("upstream_host is required")?;
        let upstream_port = self.upstream_port.ok_or("upstream_port is required")?;

        Ok(ActiveProxyOptions {
            data_dir,
            server_peerid: self.server_peerid,
            upstream_peer_private_key: self.upstream_peer_private_key,
            upstream_domain: self.upstream_domain,
            upstream_host,
            upstream_port,
        })
    }
}
