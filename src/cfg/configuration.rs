use std::{
    fmt, fs, env, mem,
    net::SocketAddr,
    path::{Path, PathBuf},
};
use log::LevelFilter;
use serde::Deserialize;

use crate::{
    Id,
    NodeInfo,
    signature,
    errors::{Result, IOError, ArgumentError},
    cfg::{
        NodeConfig,
        ActiveProxyConfig,
        config::DEFAULT_DHT_PORT
    },
};

#[derive(Debug, Deserialize)]
struct YamlNodeConfig {
    ipv4            : Option<bool>,
    ipv6            : Option<bool>,
    #[serde(default = "default_port")]
    port            : u16,
    #[serde(rename = "privateKey")]
    private_key     : String,
    #[serde(rename = "dataDir")]
    data_dir        : Option<String>,
    #[serde(rename = "databaseUri")]
    database_uri    : String,
    #[serde(default)]
    bootstraps      : Vec<YamlNodeEntry>,
    #[serde(rename = "logLevel")]
    log_level       : Option<String>,
    #[serde(rename = "logFile")]
    log_file        : Option<String>,
    #[serde(
        rename = "logConsole",
        default = "default_log_console"
    )]
    log_console     : bool,
    #[serde(rename = "enableDeveloperMode", default)]
    devmode         : bool,
    #[serde(default)]
    activeproxy     : Option<YamlActiveProxyConfig>,
}

#[derive(Debug, Deserialize)]
struct YamlActiveProxyConfig {
    #[serde(rename = "serverPeerId")]
    server_peerid    : Id,
    #[serde(rename = "peerPrivateKey")]
    peer_private_key : Option<String>,
    #[serde(rename = "upstreamHost")]
    upstream_host    : Option<String>,
    #[serde(rename = "upstreamPort")]
    upstream_port    : Option<u16>,
}

#[derive(Debug, Deserialize)]
struct YamlNodeEntry(Id, String, u16);

impl TryFrom<YamlNodeEntry> for NodeInfo {
    type Error = crate::Error;

    fn try_from(value: YamlNodeEntry) -> Result<NodeInfo> {
        let YamlNodeEntry(id, host, port) = value;
        let addr = format!("{host}:{port}")
            .parse::<SocketAddr>()
            .map_err(|e|ArgumentError::new(
                format!("Invalid bootstrap node address {host}:{port}: {e}"))
        )?;
        Ok(NodeInfo::new(id, addr))
    }
}

fn default_port() -> u16 {
    DEFAULT_DHT_PORT
}

fn default_log_console() -> bool {
    true
}

#[derive(Debug, Clone)]
pub struct Configuration {
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
    devmode         : bool,

    active_proxy    : Option<ActiveProxyCfg>,
}

#[derive(Debug, Clone)]
struct ActiveProxyCfg {
    server_peerid   : Id,
    peer_private_key: Option<signature::PrivateKey>,
    domain_name     : Option<String>,
    upstream_host   : String,
    upstream_port   : u16,
}

#[derive(Debug)]
pub struct Builder {
    host4           : Option<String>,
    host6           : Option<String>,
    port            : Option<u16>,
    private_key     : Option<signature::PrivateKey>,
    data_dir        : Option<String>,
    database_uri    : Option<String>,
    bootstrap_nodes : Vec<NodeInfo>,
    log_level       : Option<LevelFilter>,
    log_file        : Option<String>,
    log_console     : bool,
    devmode         : bool,

    ap_service_peerid   : Option<Id>,
    ap_upstream_peer_private_key : Option<signature::PrivateKey>,
    ap_upstream_domain  : Option<String>,
    ap_upstream_host    : Option<String>,
    ap_upstream_port    : Option<u16>,
}

#[allow(unused)]
impl Builder {
    pub fn new() -> Self {
        Self {
            host4           : None,
            host6           : None,
            port            : Some(DEFAULT_DHT_PORT),
            private_key     : None,
            data_dir        : None,
            database_uri    : None,
            bootstrap_nodes : Vec::new(),
            log_level       : None,
            log_file        : None,
            log_console     : true,
            devmode         : false,

            ap_service_peerid   : None,
            ap_upstream_peer_private_key : None,
            ap_upstream_domain  : None,
            ap_upstream_host    : None,
            ap_upstream_port    : None,
        }
    }

    fn apply(&mut self, yaml: YamlNodeConfig) -> Result<()> {
        self.private_key = Some(signature::PrivateKey::try_from(
            yaml.private_key.as_str()
        )?);
        self.bootstrap_nodes = yaml.bootstraps.into_iter()
            .map(NodeInfo::try_from)
            .collect::<Result<Vec<_>>>()?;

        self.host4 = if yaml.ipv4.unwrap_or(false) {
            use crate::local_addr;
            Some(local_addr(true)?.to_string())
        } else {
            None
        };
        self.host6 = if yaml.ipv6.unwrap_or(false) {
            use crate::local_addr;
            Some(local_addr(false)?.to_string())
        } else {
            None
        };

        self.port           = Some(yaml.port);
        self.data_dir       = Some(expand_datadir(yaml.data_dir));
        self.database_uri   = Some(yaml.database_uri);
        self.log_level      = Some(log_level(yaml.log_level.as_deref()));
        self.log_file       = yaml.log_file;
        self.log_console    = yaml.log_console;
        self.devmode        = yaml.devmode;

        if let Some(ap) = yaml.activeproxy {
            self.ap_service_peerid = Some(ap.server_peerid);
            self.ap_upstream_peer_private_key = ap.peer_private_key
                .as_deref()
                .map(signature::PrivateKey::try_from)
                .transpose()?;
            self.ap_upstream_host = ap.upstream_host;
            self.ap_upstream_port = ap.upstream_port;
        }
        Ok(())
    }

    pub fn with_host4(&mut self, host: impl Into<String>) -> &mut Self {
        self.host4 = Some(host.into());
        self
    }

    pub fn with_host6(&mut self, host: impl Into<String>) -> &mut Self {
        self.host6 = Some(host.into());
        self
    }

    pub fn with_port(&mut self, port: u16) -> &mut Self {
        self.port = Some(port);
        self
    }

    pub fn with_private_key(&mut self, private_key: signature::PrivateKey) -> &mut Self {
        self.private_key = Some(private_key);
        self
    }

    pub fn with_data_dir(&mut self, data_dir: impl Into<String>) -> &mut Self {
        self.data_dir = Some(expand_datadir(Some(data_dir.into())));
        self
    }

    pub fn with_database_uri(&mut self, database_uri: impl Into<String>) -> &mut Self {
        self.database_uri = Some(database_uri.into());
        self
    }

    pub fn with_bootstrap_nodes(&mut self, nodes: Vec<NodeInfo>) -> &mut Self {
        self.bootstrap_nodes = nodes;
        self
    }

    pub fn add_bootstrap_node(&mut self, node: NodeInfo) -> &mut Self {
        self.bootstrap_nodes.push(node);
        self
    }

    pub fn with_log_level(&mut self, log_level: LevelFilter) -> &mut Self {
        self.log_level = Some(log_level);
        self
    }

    pub fn with_log_file(&mut self, log_file: impl Into<String>) -> &mut Self {
        self.log_file = Some(log_file.into());
        self
    }

    pub fn with_log_console(&mut self, enabled: bool) -> &mut Self {
        self.log_console = enabled;
        self
    }

    pub fn with_devmode(&mut self, enabled: bool) -> &mut Self {
        self.devmode = enabled;
        self
    }

    pub fn with_activeproxy_service_peerid(&mut self, peerid: Id) -> &mut Self {
        self.ap_service_peerid = Some(peerid);
        self
    }

    pub fn with_upstream_peer_private_key(&mut self, private_key: signature::PrivateKey) -> &mut Self {
        self.ap_upstream_peer_private_key = Some(private_key);
        self
    }

    pub fn with_upstream_domain(&mut self, domain: impl Into<String>) -> &mut Self {
        self.ap_upstream_domain = Some(domain.into());
        self
    }

    pub fn with_upstream_host(&mut self, host: impl Into<String>) -> &mut Self {
        self.ap_upstream_host = Some(host.into());
        self
    }

    pub fn with_upstream_port(&mut self, port: u16) -> &mut Self {
        self.ap_upstream_port = Some(port);
        self
    }

    fn check_valid(&self) -> Result<()> {
        if self.host4.is_none() && self.host6.is_none() {
            return Err(ArgumentError::new("At least one of host4 or host6 must be set"));
        }
        if self.private_key.is_none() {
            return Err(ArgumentError::new("Private key is missing"));
        }
        if self.database_uri.is_none() {
            return Err(ArgumentError::new("Database URI is missing"));
        }

        if self.ap_service_peerid.is_some() {
            if self.ap_upstream_host.is_none() {
                return Err(ArgumentError::new("ActiveProxy upstream host is missing"));
            }
            if self.ap_upstream_port.is_none() {
                return Err(ArgumentError::new("ActiveProxy upstream port is missing"));
            }
        }
        Ok(())
    }

    pub fn from(&mut self, yaml: &str) -> Result<&mut Self> {
        let expanded = expand_env(yaml)?;
        let parsed = serde_yaml::from_str::<YamlNodeConfig>(&expanded)
            .map_err(|e| ArgumentError::new(format!("invalid yaml format: {e}")))?;
        self.apply(parsed)?;
        Ok(self)
    }

    pub fn load(&mut self, path: impl AsRef<Path>) -> Result<&mut Self> {
        let path  = path.as_ref();
        let input = fs::read_to_string(path).map_err(|e|
            IOError::new(format!("Reading config {} failed: {e}", path.display()))
        )?;
        self.from(&input)
    }

    pub fn load_default(&mut self) -> Result<&mut Self> {
        let paths = config_paths();
        let Some(path) = paths.iter().find(|path| path.exists()) else {
            return Err(ArgumentError::new(format!(
                "Unable to locate node.yaml in any default location: {}",
                paths.iter().map(|path| path.display().to_string())
                    .collect::<Vec<_>>()
                    .join(", ")
            )));
        };

        self.load(path)
    }

    pub fn build(&mut self) -> Result<Configuration> {
        self.check_valid()?;

        Ok(Configuration {
            host4           : self.host4.take(),
            host6           : self.host6.take(),
            port            : self.port.take().unwrap_or_else(default_port),
            private_key     : self.private_key.take().unwrap(),
            data_dir        : self.data_dir.take().unwrap_or_else(|| ".".to_string()),
            database_uri    : self.database_uri.take().unwrap(),
            bootstrap_nodes : mem::take(&mut self.bootstrap_nodes),
            log_level       : self.log_level.take().unwrap_or(LevelFilter::Info),
            log_file        : self.log_file.take(),
            log_console     : self.log_console,
            devmode         : self.devmode,

            active_proxy    : self.ap_service_peerid.take().map(|v| ActiveProxyCfg {
                server_peerid   : v,
                peer_private_key: self.ap_upstream_peer_private_key.take(),
                domain_name     : self.ap_upstream_domain.take(),
                upstream_host   : self.ap_upstream_host.take().unwrap(),
                upstream_port   : self.ap_upstream_port.take().unwrap(),
            }),
        })
    }
}

impl NodeConfig for Configuration {
    fn host4(&self) -> Option<&str> {
        self.host4.as_deref()
    }

    fn host6(&self) -> Option<&str> {
        self.host6.as_deref()
    }

    fn port(&self) -> u16 {
        self.port
    }

    fn private_key(&self) -> &signature::PrivateKey {
        &self.private_key
    }

    fn data_dir(&self) -> &str {
        &self.data_dir
    }

    fn database_uri(&self) -> &str {
        &self.database_uri
    }

    fn bootstrap_nodes(&self) -> &[NodeInfo] {
        &self.bootstrap_nodes
    }

    fn log_level(&self) -> LevelFilter {
        self.log_level
    }

    fn log_file(&self) -> Option<&str> {
        self.log_file.as_deref()
    }

    fn log_console(&self) -> bool {
        self.log_console
    }

    fn enable_devp(&self) -> bool {
        self.devmode
    }

    fn dump(&self) {
        println!("{}", self);
    }
}

impl ActiveProxyConfig for Configuration {
    fn server_peerid(&self) -> Option<&Id> {
        self.active_proxy.as_ref().map(|cfg|
            &cfg.server_peerid
        )
    }

    fn peer_private_key(&self) -> Option<&signature::PrivateKey> {
        self.active_proxy.as_ref().and_then(|cfg|
            cfg.peer_private_key.as_ref()
        )
    }

    fn domain_name(&self) -> Option<&str> {
        self.active_proxy.as_ref().and_then(|cfg|
            cfg.domain_name.as_deref()
        )
    }

    fn upstream_host(&self) -> Option<&str> {
        self.active_proxy.as_ref().map(|cfg|
            cfg.upstream_host.as_str()
        )
    }

    fn upstream_port(&self) -> Option<u16> {
        self.active_proxy.as_ref().map(|cfg|
            cfg.upstream_port
        )
    }

    fn dump(&self) {
        unimplemented!()
    }
}

fn log_level(level: Option<&str>) -> LevelFilter {
    level.and_then(|v| v.parse::<LevelFilter>().ok())
        .unwrap_or(LevelFilter::Info)
}

fn expand_datadir(data_dir: Option<String>) -> String {
    let Some(data_dir) = data_dir else {
        return ".".to_string();
    };

    if data_dir == "~" {
        return env::var("HOME").unwrap_or(data_dir);
    }

    if let Some(suffix) = data_dir.strip_prefix("~/") {
        return env::var("HOME")
            .map(|home| format!("{home}/{suffix}"))
            .unwrap_or(data_dir);
    }

    data_dir
}

fn expand_env(input: &str) -> Result<String> {
    let mut expanded = String::with_capacity(input.len());
    let mut cursor = 0;

    while let Some(offset) = input[cursor..].find("${") {
        let start = cursor + offset;
        expanded.push_str(&input[cursor..start]);

        let var_start = start + 2;
        let Some(endoff) = input[var_start..].find('}') else {
            return Err(ArgumentError::new("Unclosed environment placeholder in node.yaml"));
        };
        let end = var_start + endoff;
        let name = &input[var_start..end];
        let value = env::var(name).map_err(|_|
            ArgumentError::new(format!("Environment variable {name} is not set"))
        )?;
        expanded.push_str(&value);
        cursor = end + 1;
    }

    expanded.push_str(&input[cursor..]);
    Ok(expanded)
}

fn config_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();

    if let Ok(dir) = env::current_dir() {
        paths.push(dir.join("node.yaml"));
    }

    #[cfg(target_os = "windows")]
    {
        if let Ok(appdata) = env::var("APPDATA") {
            paths.push(PathBuf::from(appdata).join("boson").join("node.yaml"));
        }
        if let Ok(program_data) = env::var("ProgramData") {
            paths.push(PathBuf::from(program_data).join("boson").join("node.yaml"));
        }
    }

    #[cfg(not(target_os = "windows"))]
    {
        if let Ok(home) = env::var("HOME") {
            paths.push(
                PathBuf::from(home)
                    .join(".config")
                    .join("boson")
                    .join("node.yaml")
            );
        }
        paths.push(PathBuf::from("/usr/local/etc/boson/node.yaml"));
        paths.push(PathBuf::from("/etc/boson/node.yaml"));
    }
    paths
}

impl fmt::Display for Configuration {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "node config:")?;
        write!(f, "\n\thost4\t:{}", self.host4.as_deref().unwrap_or("<none>"))?;
        write!(f, "\n\thost6\t:{}", self.host6.as_deref().unwrap_or("<none>"))?;
        write!(f, "\n\tport\t:{}", self.port)?;
        write!(f, "\n\tsk\t:{}", self.private_key)?;
        write!(f, "\n\tataDir\t:{}", self.data_dir)?;
        write!(f, "\n\tlogLevel\t:{:?}", self.log_level)?;
        write!(f, "\n\tlogFile\t:{}", self.log_file.as_deref().unwrap_or("<none>"))?;
        write!(f, "\n\tlogConsole\t:{}", self.log_console)?;
        write!(f, "\n\tenableDeveloperMode\t:{}", self.devmode)?;

        if self.bootstrap_nodes.is_empty() {
            write!(f, "\n\tbootstraps\t:[]")?;
        } else {
            write!(f, "\n\tbootstraps\t:")?;
            for node in &self.bootstrap_nodes {
                write!(f, "\n\t- {} {} {}", node.id(), node.host(), node.port())?;
            }
        }

        if let Some(ap) = &self.active_proxy {
            write!(f, "\n\tactiveProxy\t:")?;
            write!(f, "\n\t- serverPeerId\t:{}", ap.server_peerid)?;
            write!(f, "\n\t- peerPrivateKey\t:{}", ap.peer_private_key.as_ref().map(|k| k.to_string()).unwrap_or("<none>".to_string()))?;
            write!(f, "\n\t- domainName\t:{}", ap.domain_name.as_deref().unwrap_or("<none>"))?;
            write!(f, "\n\t- upstreamHost\t:{}", ap.upstream_host)?;
            write!(f, "\n\t- upstreamPort\t:{}", ap.upstream_port)?;
        } else {
            write!(f, "\n\tactiveProxy\t:<none>")?;
        }

        Ok(())
    }
}
