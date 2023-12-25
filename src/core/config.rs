use std::net::SocketAddr;
use crate::node_info::NodeInfo;

pub trait Config: Send + Sync {
    fn addr4(&self) -> Option<&SocketAddr>;
    fn addr6(&self) -> Option<&SocketAddr>;

    fn listening_port(&self) -> u16;

    fn storage_path(&self) -> &str;
    fn bootstrap_nodes(&self) -> &[NodeInfo];

    #[cfg(feature = "inspect")]
    fn dump(&self);
}
