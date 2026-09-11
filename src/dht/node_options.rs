use std::{
    fs,
    net::SocketAddr,
    path::Path,
};

use log::LevelFilter;
use serde::Deserialize;
use crate::{
    NodeInfo,
    signature,
    errors::{Result, ArgumentError, IOError},
};

const DEFAULT_DHT_PORT: u16 = 39001;
const DATA_DIR: &str = ".";
const DATABASE_URI: &str = "storage.db";

#[derive(Debug, Deserialize)]
struct SerdeNodeOptions {
    #[serde(rename = "privateKey")]
    private_key: String,
    ipv4: Option<bool>,
    ipv6: Option<bool>,
    #[serde(default = "default_port")]
    port: u16,
    #[serde(rename = "dataDir")]
    data_dir: Option<String>,
    #[serde(rename = "databaseUri")]
    database_uri: String,
    #[serde(default)]
    bootstraps: Vec<SerdeNodeEntry>,
    #[serde(rename = "logLevel")]
    log_level: Option<String>,
    #[serde(rename = "logFile")]
    log_file: Option<String>,
    #[serde(rename = "logConsole", default = "default_log_console")]
    log_console: bool,
    #[serde(rename = "enableDeveloperMode", default)]
    developer_mode: bool,
}

#[derive(Debug, Deserialize)]
struct SerdeNodeEntry(crate::Id, String, u16);

impl TryFrom<SerdeNodeEntry> for NodeInfo {
    type Error = crate::Error;

    fn try_from(value: SerdeNodeEntry) -> Result<Self> {
        let SerdeNodeEntry(id, host, port) = value;
        let address = format!("{host}:{port}").parse::<SocketAddr>().map_err(|e| {
            ArgumentError::new(format!("Invalid bootstrap node address {host}:{port}: {e}"))
        })?;
        Ok(NodeInfo::new(id, address))
    }
}

fn default_port() -> u16 {
    DEFAULT_DHT_PORT
}

fn default_log_console() -> bool {
    true
}

#[derive(Debug, Clone)]
pub struct NodeOptions {
    host4           : Option<String>,
    host6           : Option<String>,
    port            : u16,

    private_key     : signature::PrivateKey,

    data_dir        : String,
    database_uri    : String,
    bootstrap_nodes : Vec<NodeInfo>,

    log_level       : LevelFilter,
    log_file        : Option<String>,
    log_console     : bool,

    developer_mode  : bool,
}

#[allow(unused)]
impl NodeOptions {
    fn new() -> Self {
        NodeOptions {
            host4           : None,
            host6           : None,
            port            : DEFAULT_DHT_PORT,
            private_key     : signature::KeyPair::random().to_private_key(),
            data_dir        : DATA_DIR.to_string(),
            database_uri    : DATABASE_URI.to_string(),
            bootstrap_nodes : Vec::new(),
            log_level       : LevelFilter::Info,
            log_file        : None,
            log_console     : true,
            developer_mode  : false,
        }
    }
    pub fn builder() -> NodeOptionsBuilder {
        NodeOptionsBuilder::new()
    }

    pub fn host4(&self) -> Option<&str> {
        self.host4.as_deref()
    }

    pub fn host6(&self) -> Option<&str> {
        self.host6.as_deref()
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    pub fn private_key(&self) -> &signature::PrivateKey {
        &self.private_key
    }

    pub fn data_dir(&self) -> &str {
        &self.data_dir
    }

    pub fn database_uri(&self) -> &str {
        &self.database_uri
    }

    pub fn bootstrap_nodes(&self) -> &[NodeInfo] {
        &self.bootstrap_nodes
    }

    pub fn log_level(&self) -> LevelFilter {
        self.log_level
    }

    pub fn log_file(&self) -> Option<&str> {
        self.log_file.as_deref()
    }

    pub fn log_console(&self) -> bool {
        self.log_console
    }

    pub fn developer_mode(&self) -> bool {
        self.developer_mode
    }
}

pub struct NodeOptionsBuilder {
    options: NodeOptions,
}

impl NodeOptionsBuilder {
    pub fn new() -> Self {
        Self {
            options: NodeOptions::new(),
        }
    }

    pub fn read_from(mut self, yaml: &str) -> Result<Self> {
        let parsed = serde_yaml::from_str::<SerdeNodeOptions>(yaml)
            .map_err(|e| ArgumentError::new(format!("invalid node YAML format: {e}")))?;
        self.options.private_key = signature::PrivateKey::try_from(parsed.private_key.as_str())?;
        self.options.host4 = parsed
            .ipv4
            .unwrap_or(false)
            .then(|| crate::local_addr(true).map(|address| address.to_string()))
            .transpose()?;
        self.options.host6 = parsed
            .ipv6
            .unwrap_or(false)
            .then(|| crate::local_addr(false).map(|address| address.to_string()))
            .transpose()?;
        self.options.port = parsed.port;
        self.options.data_dir = parsed.data_dir.unwrap_or_else(|| DATA_DIR.to_string());
        self.options.database_uri = parsed.database_uri;
        self.options.bootstrap_nodes = parsed
            .bootstraps
            .into_iter()
            .map(NodeInfo::try_from)
            .collect::<Result<Vec<_>>>()?;
        self.options.log_level = parsed
            .log_level
            .as_deref()
            .and_then(|level| level.parse().ok())
            .unwrap_or(LevelFilter::Info);
        self.options.log_file = parsed.log_file;
        self.options.log_console = parsed.log_console;
        self.options.developer_mode = parsed.developer_mode;
        Ok(self)
    }

    pub fn load_from(self, path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        let yaml = fs::read_to_string(path)
            .map_err(|e| IOError::new(format!("Reading config {} failed: {e}", path.display())))?;
        self.read_from(&yaml)
    }

    pub fn with_host4(mut self, host: &str) -> Self {
        self.options.host4 = Some(host.to_string());
        self
    }

    pub fn with_host6(mut self, host: &str) -> Self {
        self.options.host6 = Some(host.to_string());
        self
    }

    pub fn with_port(mut self, port: u16) -> Self {
        self.options.port = port;
        self
    }

    pub fn with_private_key(mut self, private_key: signature::PrivateKey) -> Self {
        self.options.private_key = private_key;
        self
    }

    pub fn with_data_dir(mut self, data_dir: &str) -> Self {
        self.options.data_dir = data_dir.to_string();
        self
    }

    pub fn with_database_uri(mut self, database_uri: &str) -> Self {
        self.options.database_uri = database_uri.to_string();
        self
    }

    pub fn with_bootstrap_nodes(mut self, nodes: Vec<NodeInfo>) -> Self {
        self.options.bootstrap_nodes = nodes;
        self
    }

    pub fn with_log_level(mut self, level: LevelFilter) -> Self {
        self.options.log_level = level;
        self
    }

    pub fn with_log_file(mut self, log_file: &str) -> Self {
        self.options.log_file = Some(log_file.to_string());
        self
    }

    pub fn with_log_console(mut self, log_console: bool) -> Self {
        self.options.log_console = log_console;
        self
    }

    pub fn enable_developer_mode(mut self) -> Self {
        self.options.developer_mode = true;
        self
    }

    pub fn build(self) -> Result<NodeOptions> {
        if self.options.host4.is_none() && self.options.host6.is_none() {
            return Err(ArgumentError::new("Either host4 or host6 must be specified"));
        }
        Ok(self.options)
    }
}
