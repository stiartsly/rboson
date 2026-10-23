use crate::{
    errors::{ArgumentError, Error, IOError, Result},
    signature::{KeyPair, PrivateKey},
    NodeInfo,
};
use log::LevelFilter;
use serde::Deserialize;
use std::path::{Path, PathBuf};
use std::{env, fmt, fs, net::SocketAddr};

pub const DEFAULT_DHT_PORT: u16 = 39001;

const DEFAULT_DATA_DIR: &str = ".";
const DEFAULT_DATABASE_URI: &str = "jdbc:sqlite:node.db";

#[derive(Debug, Clone, Deserialize)]
#[serde(try_from = "SerdeNodeOptions")]
pub struct NodeOptions {
    host4: Option<String>,
    host6: Option<String>,
    port: u16,

    keypair: KeyPair,

    data_dir: PathBuf,
    database_uri: String,
    bootstrap_nodes: Vec<NodeInfo>,

    log_level: LevelFilter,
    log_file: Option<String>,
    log_console: bool,

    developer_mode: bool,
}

impl NodeOptions {
    pub fn new(sk: PrivateKey) -> Self {
        Self {
            host4: None,
            host6: None,
            port: DEFAULT_DHT_PORT,
            keypair: KeyPair::from(sk),
            data_dir: DEFAULT_DATA_DIR.into(),
            database_uri: DEFAULT_DATABASE_URI.into(),
            bootstrap_nodes: Vec::new(),
            log_level: LevelFilter::Info,
            log_file: None,
            log_console: true,
            developer_mode: false,
        }
    }

    pub fn parse(yaml: impl AsRef<str>) -> Result<Self> {
        let expanded_yaml = expand_environ_vars(yaml.as_ref())?;
        serde_yaml::from_str::<SerdeNodeOptions>(&expanded_yaml)
            .map_err(|e| ArgumentError::new(format!("invalid YAML format: {e}")))?
            .try_into()
    }

    pub fn read(yaml: impl AsRef<str>) -> Result<Self> {
        Self::parse(yaml)
    }

    pub fn load(path: impl AsRef<Path>) -> Result<Self> {
        let yaml = fs::read_to_string(path.as_ref()).map_err(|e| {
            IOError::new(format!(
                "Reading config {} failed: {e}",
                path.as_ref().display()
            ))
        })?;
        Self::parse(&yaml)
    }

    pub fn with_host4(mut self, host: impl Into<String>) -> Self {
        self.host4 = Some(host.into());
        self
    }

    pub fn enable_host4(self) -> Result<Self> {
        let host = crate::local_addr(true).map(|v| v.to_string())?;
        Ok(self.with_host4(host))
    }

    pub fn with_host6(mut self, host: impl Into<String>) -> Self {
        self.host6 = Some(host.into());
        self
    }

    pub fn enable_host6(self) -> Result<Self> {
        let host = crate::local_addr(false).map(|v| v.to_string())?;
        Ok(self.with_host6(host))
    }

    pub fn with_port(mut self, port: u16) -> Self {
        self.port = port;
        self
    }

    pub fn with_private_key(mut self, private_key: PrivateKey) -> Self {
        self.keypair = KeyPair::from(private_key);
        self
    }

    pub fn with_keypair(mut self, keypair: KeyPair) -> Self {
        self.keypair = keypair;
        self
    }

    pub fn with_data_dir(mut self, data_dir: impl AsRef<Path>) -> Self {
        self.data_dir = PathBuf::from(data_dir.as_ref());
        self
    }

    pub fn with_bootstrap_nodes(mut self, nodes: Vec<NodeInfo>) -> Self {
        self.bootstrap_nodes = nodes;
        self
    }

    pub fn with_log_level(mut self, level: LevelFilter) -> Self {
        self.log_level = level;
        self
    }

    pub fn with_log_file(mut self, log_file: impl Into<String>) -> Self {
        self.log_file = Some(log_file.into());
        self
    }

    pub fn enable_log_console(self) -> Self {
        self.with_log_console(true)
    }

    pub fn with_log_console(mut self, log_console: bool) -> Self {
        self.log_console = log_console;
        self
    }

    pub fn enable_developer_mode(self) -> Self {
        self.with_developer_mode(true)
    }

    pub fn with_developer_mode(mut self, developer_mode: bool) -> Self {
        self.developer_mode = developer_mode;
        self
    }

    pub fn check_completeness(&self) -> Result<()> {
        if self.host4.is_none() && self.host6.is_none() {
            return Err(IOError::new("At least one of host4 or host6 must be set"));
        }
        Ok(())
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

    pub fn private_key(&self) -> &PrivateKey {
        self.keypair.private_key()
    }

    pub fn data_dir(&self) -> &str {
        self.data_dir.to_str().unwrap_or(DEFAULT_DATA_DIR)
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

    pub fn log_console_enabled(&self) -> bool {
        self.log_console
    }

    pub fn log_console(&self) -> bool {
        self.log_console
    }

    pub fn developer_mode(&self) -> bool {
        self.developer_mode
    }
}

impl TryFrom<&str> for NodeOptions {
    type Error = Error;

    fn try_from(yaml: &str) -> Result<Self> {
        Self::read(yaml)
    }
}

impl fmt::Display for NodeOptions {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Node Options: {{ host4: {:?}, host6: {:?}, port: {}, data_dir: {}, database_uri: {}, bootstrap_nodes: {:?}, log_level: {:?}, log_file: {:?}, log_console: {}, developer_mode: {} }}",
            self.host4,
            self.host6,
            self.port,
            self.data_dir(),
            self.database_uri,
            self.bootstrap_nodes,
            self.log_level,
            self.log_file,
            self.log_console,
            self.developer_mode
        )
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
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
    _database_uri: Option<String>,
    #[serde(default)]
    bootstraps: Vec<SerdeNodeEntry>,
    #[serde(rename = "logLevel")]
    log_level: Option<String>,
    #[serde(rename = "logFile")]
    log_file: Option<String>,
    #[serde(rename = "logConsole")]
    log_console: Option<bool>,
    #[serde(rename = "enableDeveloperMode", default)]
    developer_mode: bool,
}

impl TryFrom<SerdeNodeOptions> for NodeOptions {
    type Error = Error;

    fn try_from(sopts: SerdeNodeOptions) -> Result<Self> {
        let sk = PrivateKey::try_from(sopts.private_key.as_str())?;
        let mut opts = Self::new(sk);
        if sopts.ipv4.unwrap_or(false) {
            opts = opts.enable_host4()?;
        }
        if sopts.ipv6.unwrap_or(false) {
            opts = opts.enable_host6()?;
        }
        opts = opts.with_port(sopts.port);

        let data_dir =
            expand_relative_path(&sopts.data_dir.unwrap_or(DEFAULT_DATA_DIR.to_string()))?;
        opts = opts.with_data_dir(data_dir);

        let log_level = sopts
            .log_level
            .as_deref()
            .and_then(|level| level.parse().ok())
            .unwrap_or(LevelFilter::Info);
        opts = opts.with_log_level(log_level);
        opts = opts.with_log_console(sopts.log_console.unwrap_or(true));

        if let Some(log_file) = sopts.log_file {
            opts = opts.with_log_file(log_file);
        }

        let bootstrap_nodes = sopts
            .bootstraps
            .into_iter()
            .map(NodeInfo::try_from)
            .collect::<Result<Vec<_>>>()?;
        opts = opts.with_bootstrap_nodes(bootstrap_nodes);
        opts = opts.with_developer_mode(sopts.developer_mode);
        Ok(opts)
    }
}

#[derive(Debug, Deserialize)]
struct SerdeNodeEntry(crate::Id, String, u16);

impl TryFrom<SerdeNodeEntry> for NodeInfo {
    type Error = crate::Error;

    fn try_from(value: SerdeNodeEntry) -> Result<Self> {
        let SerdeNodeEntry(id, host, port) = value;
        let address = format!("{host}:{port}")
            .parse::<SocketAddr>()
            .map_err(|e| {
                ArgumentError::new(format!("Invalid bootstrap node address {host}:{port}: {e}"))
            })?;
        Ok(NodeInfo::new(id, address))
    }
}

fn default_port() -> u16 {
    DEFAULT_DHT_PORT
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

fn expand_relative_path(input: &str) -> Result<PathBuf> {
    let data_dir = input;
    let home_dir = env::var_os("HOME")
        .or_else(|| env::var_os("USERPROFILE"))
        .map(PathBuf::from);

    let relative = if data_dir == "~" {
        Some("")
    } else {
        data_dir.strip_prefix("~/")
    };

    match relative {
        Some(relative) => {
            let home_dir = home_dir.ok_or_else(|| {
                ArgumentError::new(format!(
                "Data path {data_dir} can not be expanded because the home directory is unavailable"
            ))
            })?;
            let path = home_dir.join(relative);
            Ok(path)
        }
        _ => Ok(PathBuf::from(data_dir)),
    }
}
