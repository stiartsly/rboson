use std::net::SocketAddr;
use log::LevelFilter;
use crate::core::node_info::NodeInfo;

pub trait ActiveProxyConfig: Send + Sync {
    fn server_peerid(&self) -> &str;
    fn peer_private_key(&self) -> Option<&str>;
    fn domain_name(&self) -> Option<&str>;
    fn upstream_host(&self) -> &str;
    fn upstream_port(&self) -> u16;
}

pub trait Config: Send + Sync {
    fn addr4(&self) -> Option<&SocketAddr>;
    fn addr6(&self) -> Option<&SocketAddr>;

    fn listening_port(&self) -> u16;

    fn storage_path(&self) -> &str;
    fn bootstrap_nodes(&self) -> &[NodeInfo];

    fn log_level(&self) -> LevelFilter;
    fn log_file(&self) -> Option<&str>;

    fn activeproxy(&self) -> Option<&Box<dyn ActiveProxyConfig>>;

    #[cfg(feature = "inspect")]
    fn dump(&self);
}
