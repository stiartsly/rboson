use std::net::SocketAddr;
use log::LevelFilter;
use crate::core::node_info::NodeInfo;

pub trait Config: Send + Sync {
    fn addr4(&self) -> Option<&SocketAddr>;
    fn addr6(&self) -> Option<&SocketAddr>;

    fn listening_port(&self) -> u16;

    fn storage_path(&self) -> &str;
    fn bootstrap_nodes(&self) -> &[NodeInfo];

    fn log_level(&self) -> LevelFilter;
    fn log_file(&self) -> Option<&str>;

    #[cfg(feature = "inspect")]
    fn dump(&self);
}
