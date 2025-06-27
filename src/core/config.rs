use std::net::SocketAddr;
use log::LevelFilter;

use crate::NodeInfo;

pub trait UserConfig: Send + Sync {
    fn name(&self) -> Option<&str> { None }
    fn password(&self) -> Option<&str> { None }
    fn private_key(&self) -> &str;
}

pub trait ActiveProxyConfig: Send + Sync {
    fn server_peerid(&self) -> &str;
    fn peer_private_key(&self) -> Option<&str>;
    fn domain_name(&self) -> Option<&str>;
    fn upstream_host(&self) -> &str;
    fn upstream_port(&self) -> u16;
}

pub trait MessagingConfig: Send + Sync {
    fn server_peerid(&self) -> &str;
}

pub trait Config: Send + Sync {
    fn addr4(&self) -> Option<&SocketAddr> { None }
    fn addr6(&self) -> Option<&SocketAddr> { None }

    fn listening_port(&self) -> u16;
    fn storage_path(&self) -> &str;
    fn bootstrap_nodes(&self) -> &[NodeInfo];

    fn log_level(&self) -> LevelFilter { LevelFilter::Info }
    fn log_file(&self) -> Option<&str> { None }

    fn user(&self) -> Option<&Box<dyn UserConfig>> { None }
    fn activeproxy(&self) -> Option<&Box<dyn ActiveProxyConfig>> { None }
    fn messaging(&self) -> Option<&Box<dyn MessagingConfig>> { None }

    #[cfg(feature = "inspect")]
    fn dump(&self);
}
