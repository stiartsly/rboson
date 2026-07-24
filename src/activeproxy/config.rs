use log::LevelFilter;

use crate::{NodeInfo, signature};
pub const DEFAULT_DHT_PORT: u16 = 19001;

pub trait ActiveProxyConfig: Send + Sync {
    fn host4(&self) -> Option<&str>;
    fn host6(&self) -> Option<&str>;
    fn port(&self) -> u16 { DEFAULT_DHT_PORT}

    fn private_key(&self) -> &signature::PrivateKey;

    fn data_dir(&self) -> &str;
    fn database_uri(&self) -> &str;
    fn bootstrap_nodes(&self) -> &[NodeInfo];

    fn log_level(&self) -> LevelFilter { LevelFilter::Info }
    fn log_file(&self) -> Option<&str> { None }

    fn enable_devp(&self) -> bool { false }

    fn dump(&self);
}
