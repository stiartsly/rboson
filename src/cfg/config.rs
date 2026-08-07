use log::LevelFilter;

use crate::{
    Id,
    NodeInfo,
    signature
};
pub const DEFAULT_DHT_PORT: u16 = 19001;

pub trait NodeConfig {
    fn host4(&self) -> Option<&str>;
    fn host6(&self) -> Option<&str>;
    fn port(&self) -> u16 { DEFAULT_DHT_PORT}

    fn private_key(&self) -> &signature::PrivateKey;

    fn data_dir(&self) -> &str;
    fn database_uri(&self) -> &str;
    fn bootstrap_nodes(&self) -> &[NodeInfo];

    fn log_level(&self) -> LevelFilter { LevelFilter::Info }
    fn log_file(&self) -> Option<&str> { None }
    fn log_console(&self) -> bool { false }

    fn enable_devp(&self) -> bool { false }

    fn dump(&self) {}
}

pub trait ActiveProxyConfig {
    fn server_peerid(&self) -> Option<&Id> {
        None
    }

    fn peer_private_key(&self) -> Option<&signature::PrivateKey> {
        None
    }

    fn active_proxy_host4(&self) -> Option<&str> {
        None
    }

    fn active_proxy_host6(&self) -> Option<&str> {
        None
    }

    fn domain_name(&self) -> Option<&str> {
        None
    }

    fn upstream_host(&self) -> Option<&str> {
        None
    }

    fn upstream_port(&self) -> Option<u16> {
        None
    }

    fn dump(&self) {}
}
