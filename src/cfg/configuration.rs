use std::{
    env, fmt, fs,
    net::SocketAddr,
    path::{Path, PathBuf},
};

use log::LevelFilter;
use serde::Deserialize;

use crate::{
    Id,
    NodeInfo,
    activeproxy::{
        Options as ActiveProxyOptions,
        OptionsBuilder as ActiveProxyOptionsBuilder
    },
    dht::{NodeOptions, NodeOptionsBuilder},
    errors::{ArgumentError, IOError, Result},
    signature,
};

const DEFAULT_DHT_PORT: u16 = 19001;

#[derive(Debug, Deserialize)]
struct SerdeConfiguration {
    #[serde(rename = "privateKey")]
    private_key     : String,
    ipv4: Option<bool>,
    ipv6: Option<bool>,
    #[serde(default = "default_port")]
    port            : u16,
    #[serde(rename = "dataDir")]
    data_dir        : Option<String>,
    #[serde(rename = "databaseUri")]
    database_uri    : String,
    #[serde(default)]
    bootstraps      : Vec<SerdeNodeEntry>,
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

    #[serde(rename = "userKey")]
    user_key        : Option<String>,
    #[serde(rename = "deviceKey")]
    device_key      : Option<String>,

    #[serde(default)]
    activeproxy     : Option<SerdeActiveProxy>,
}

#[derive(Debug, Deserialize)]
struct SerdeNodeEntry(Id, String, u16);

#[derive(Debug, Deserialize)]
struct SerdeActiveProxy {
    #[serde(rename = "device_key")]
    device_key      : Option<String>,
    #[serde(rename = "serverPeerId")]
    server_peerid   : Id,
    #[serde(rename = "upstreamHost")]
    upstream_host   : Option<String>,
    #[serde(rename = "upstreamPort")]
    upstream_port   : Option<u16>,
    name_access     : Option<bool>,
    announce_peer   : Option<bool>,
}

impl TryFrom<SerdeNodeEntry> for NodeInfo {
    type Error = crate::Error;

    fn try_from(value: SerdeNodeEntry) -> Result<NodeInfo> {
        let SerdeNodeEntry(id, host, port) = value;
        let addr = format!("{host}:{port}")
            .parse::<SocketAddr>()
            .map_err(|e| ArgumentError::new(format!(
                "Invalid bootstrap node address {host}:{port}: {e}"
            )))?;
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
    private_key     : signature::PrivateKey,
    host4           : Option<String>,
    host6           : Option<String>,
    port            : u16,
    data_dir        : String,
    database_uri    : String,
    bootstrap_nodes : Vec<NodeInfo>,
    log_level       : LevelFilter,
    log_file        : Option<String>,
    log_console     : bool,
    developer_mode  : bool,

    user_key        : Option<signature::PrivateKey>,
    device_key      : Option<signature::PrivateKey>,

    server_peerid   : Option<Id>,
    upstream_host   : Option<String>,
    upstream_port   : Option<u16>,
    name_access     : bool,
    announce_peer   : bool,
}

#[derive(Debug)]
pub struct Builder {
    private_key     : Option<signature::PrivateKey>,
    host4           : Option<String>,
    host6           : Option<String>,
    port            : Option<u16>,
    data_dir        : Option<String>,
    database_uri    : Option<String>,
    bootstrap_nodes : Vec<NodeInfo>,
    log_level       : Option<LevelFilter>,
    log_file        : Option<String>,
    log_console     : bool,
    devmode         : bool,

    server_peerid   : Option<Id>,
    user_key        : Option<signature::PrivateKey>,
    device_key      : Option<signature::PrivateKey>,
    upstream_host   : Option<String>,
    upstream_port   : Option<u16>,
    name_access     : bool,
    announce_peer   : bool,
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

            server_peerid   : None,
            user_key        : None,
            device_key      : None,
            upstream_host   : None,
            upstream_port   : None,
            name_access     : false,
            announce_peer   : false,
        }
    }

    fn apply(&mut self, yaml: SerdeConfiguration) -> Result<()> {
        self.private_key = Some(signature::PrivateKey::try_from(yaml.private_key.as_str())?);
        self.bootstrap_nodes = yaml
            .bootstraps
            .into_iter()
            .map(NodeInfo::try_from)
            .collect::<Result<Vec<_>>>()?;

        self.host4 = if yaml.ipv4.unwrap_or(false) {
            Some(crate::local_addr(true)?.to_string())
        } else {
            None
        };

        self.host6 = if yaml.ipv6.unwrap_or(false) {
            Some(crate::local_addr(false)?.to_string())
        } else {
            None
        };

        self.port = Some(yaml.port);
        self.data_dir = Some(expand_datadir(yaml.data_dir));
        self.database_uri = Some(yaml.database_uri);
        self.log_level = Some(parse_log_level(yaml.log_level.as_deref()));
        self.log_file = yaml.log_file;
        self.log_console = yaml.log_console;
        self.devmode = yaml.devmode;

        self.user_key = if let Some(key) = yaml.user_key {
            Some(signature::PrivateKey::try_from(key.as_str())?)
        } else {
            None
        };

        if let Some(ap) = yaml.activeproxy {
            self.server_peerid = Some(ap.server_peerid);
            self.upstream_host = ap.upstream_host;
            self.upstream_port = ap.upstream_port;
            self.name_access   = ap.name_access.unwrap_or(false);
            self.announce_peer = ap.announce_peer.unwrap_or(false);

            if yaml.device_key.is_none() {
                self.device_key = ap
                    .device_key
                    .as_deref()
                    .map(signature::PrivateKey::try_from)
                    .transpose()?;
            }
        }

        if let Some(key) = yaml.device_key {
            self.device_key = Some(signature::PrivateKey::try_from(key.as_str())?);
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

    pub fn with_user_key(&mut self, user_key: signature::PrivateKey) -> &mut Self {
        self.user_key = Some(user_key);
        self
    }

    pub fn with_device_key(&mut self, device_key: signature::PrivateKey) -> &mut Self {
        self.device_key = Some(device_key);
        self
    }

    pub fn with_activeproxy_service_peerid(&mut self, peerid: Id) -> &mut Self {
        self.server_peerid = Some(peerid);
        self
    }

    pub fn with_upstream_host(&mut self, host: impl Into<String>) -> &mut Self {
        self.upstream_host = Some(host.into());
        self
    }

    pub fn with_upstream_port(&mut self, port: u16) -> &mut Self {
        self.upstream_port = Some(port);
        self
    }

    pub fn enable_name_access(&mut self) -> &mut Self {
        self.name_access = true;
        self
    }

    pub fn enable_announce_peer(&mut self) -> &mut Self {
        self.announce_peer = true;
        self
    }

    pub fn read_from(&mut self, yaml: &str) -> Result<&mut Self> {
        let expanded = expand_env(yaml)?;
        let parsed = serde_yaml::from_str::<SerdeConfiguration>(&expanded)
            .map_err(|e| ArgumentError::new(format!("invalid yaml format: {e}")))?;
        self.apply(parsed)?;
        Ok(self)
    }

    pub fn load_from(&mut self, path: impl AsRef<Path>) -> Result<&mut Self> {
        let path = path.as_ref();
        let input = fs::read_to_string(path)
            .map_err(|e| IOError::new(format!("Reading config {} failed: {e}", path.display())))?;
        self.read_from(&input)
    }

    pub fn load_default(&mut self) -> Result<&mut Self> {
        let paths = config_paths();
        let Some(path) = paths.iter().find(|path| path.exists()) else {
            return Err(ArgumentError::new(format!(
                "Unable to locate node.yaml in any default location: {}",
                paths
                    .iter()
                    .map(|path| path.display().to_string())
                    .collect::<Vec<_>>()
                    .join(", ")
            )));
        };

        self.load_from(path)
    }

    fn check_global_options(&self) -> Result<()> {
        if self.host4.is_none() && self.host6.is_none() {
            return Err(ArgumentError::new("At least one of host4 or host6 must be set"));
        }
        if self.database_uri.is_none() {
            return Err(ArgumentError::new("Database URI is missing"));
        }
        if self.private_key.is_none() {
            return Err(ArgumentError::new("Private key is missing"));
        }
        Ok(())
    }

    fn check_activeproxy_options(&self) -> Result<()> {
        if self.server_peerid.is_none() {
            return Ok(());
        }
        if self.device_key.is_none() {
            return Err(ArgumentError::new("Device key is missing"));
        }
        if self.upstream_host.is_none() {
            return Err(ArgumentError::new("ActiveProxy upstream host is missing"));
        }
        if self.upstream_port.is_none() {
            return Err(ArgumentError::new("ActiveProxy upstream port is missing"));
        }
        Ok(())
    }

    pub fn build(&mut self) -> Result<Configuration> {
        self.check_global_options()?;
        self.check_activeproxy_options()?;

        Ok(Configuration {
            private_key     : self.private_key.take().unwrap(),
            host4           : self.host4.take(),
            host6           : self.host6.take(),
            port            : self.port.take().unwrap_or(DEFAULT_DHT_PORT),
            data_dir        : self.data_dir.take().unwrap_or_else(|| ".".to_string()),
            database_uri    : self.database_uri.take().unwrap(),
            bootstrap_nodes : std::mem::take(&mut self.bootstrap_nodes),
            log_level       : self.log_level.take().unwrap_or(LevelFilter::Info),
            log_file        : self.log_file.take(),
            log_console     : self.log_console,
            developer_mode  : self.devmode,

            user_key        : self.user_key.take(),
            device_key      : self.device_key.take(),

            server_peerid   : self.server_peerid.take(),
            upstream_host   : self.upstream_host.take(),
            upstream_port   : self.upstream_port.take(),
            name_access     : self.name_access,
            announce_peer   : self.announce_peer,
        })
    }
}

impl Configuration {
    pub fn new() -> Builder {
        Builder::new()
    }

    pub fn private_key(&self) -> &signature::PrivateKey {
        &self.private_key
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

    pub fn enable_devp(&self) -> bool {
        self.developer_mode
    }

    pub fn user_key(&self) -> Option<&signature::PrivateKey> {
        self.user_key.as_ref()
    }

    pub fn device_key(&self) -> Option<&signature::PrivateKey> {
        self.device_key.as_ref()
    }

    pub fn server_peerid(&self) -> Option<&Id> {
        self.server_peerid.as_ref()
    }

    pub fn upstream_host(&self) -> Option<&str> {
        self.upstream_host.as_deref()
    }

    pub fn upstream_port(&self) -> Option<u16> {
        self.upstream_port
    }

    pub fn is_name_access_enabled(&self) -> bool {
        self.name_access
    }

    pub fn is_announce_peer_enabled(&self) -> bool {
        self.announce_peer
    }

    pub fn dump(&self) {
        println!("{}", self);
    }

    pub fn build_node_options(&self) -> Result<NodeOptions> {
        let mut builder = NodeOptionsBuilder::new();

        if let Some(v) = self.host4.as_deref() {
            builder = builder.with_host4(v);
        }
        if let Some(v) = self.host6.as_deref() {
            builder = builder.with_host6(v);
        }
        builder = builder.with_port(self.port)
            .with_private_key(self.private_key.clone())
            .with_data_dir(&self.data_dir)
            .with_database_uri(&self.database_uri)
            .with_bootstrap_nodes(self.bootstrap_nodes.clone())
            .with_log_level(self.log_level)
            .with_log_console(self.log_console);

        if let Some(v) = self.log_file.as_deref() {
            builder = builder.with_log_file(v);
        }
        if self.developer_mode {
            builder = builder.enable_developer_mode();
        }

        let options = builder
            .build()
            .map_err(|e| ArgumentError::new(e.to_string()))?;
        Ok(options)
    }

    pub fn build_activeproxy_options(&self) -> Result<Option<ActiveProxyOptions>> {
        let Some(server_peerid) = self.server_peerid.clone() else {
            return Ok(None);
        };

        let mut builder = ActiveProxyOptionsBuilder::new(server_peerid)
            .with_data_dir(&self.data_dir)
            .with_upstream_host(self.upstream_host.as_deref().ok_or_else(|| {
                ArgumentError::new("ActiveProxy upstream host is missing")
            })?)
            .with_upstream_port(self.upstream_port.ok_or_else(|| {
                ArgumentError::new("ActiveProxy upstream port is missing")
            })?)
            .with_device_key(
                signature::KeyPair::from(self.device_key.as_ref().ok_or_else(|| {
                    ArgumentError::new("ActiveProxy device key is missing")
                })?)
            );

        if let Some(user_key) = &self.user_key {
            builder = builder.with_user_key(
                signature::KeyPair::from(user_key)
            );
        }

        let options = builder
            .build()
            .map_err(|e| ArgumentError::new(e.to_string()))?;

        Ok(Some(options))
    }
}

fn parse_log_level(level: Option<&str>) -> LevelFilter {
    level
        .and_then(|v| v.parse::<LevelFilter>().ok())
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
            return Err(ArgumentError::new(
                "Unclosed environment placeholder in node.yaml",
            ));
        };
        let end = var_start + endoff;
        let name = &input[var_start..end];
        let value = env::var(name)
            .map_err(|_| ArgumentError::new(format!("Environment variable {name} is not set")))?;
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
            paths.push(
                PathBuf::from(program_data)
                    .join("boson")
                    .join("node.yaml"),
            );
        }
    }

    #[cfg(not(target_os = "windows"))]
    {
        if let Ok(home) = env::var("HOME") {
            paths.push(
                PathBuf::from(home)
                    .join(".config")
                    .join("boson")
                    .join("node.yaml"),
            );
        }
        paths.push(PathBuf::from("/usr/local/etc/boson/node.yaml"));
        paths.push(PathBuf::from("/etc/boson/node.yaml"));
    }
    paths
}

impl fmt::Display for Configuration {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "configuration:")?;
        if let Some(ref host4) = self.host4.as_ref() {
            write!(f, "\n\thost4\t\t:{}", host4)?;
        }
        if let Some(ref host6) = self.host6.as_ref() {
            write!(f, "\n\thost6\t\t:{}", host6)?;
        }
        write!(f, "\n\tport\t\t:{}", self.port)?;
        // write!(f, "\n\tsk\t\t:{}", self.private_key)?;
        write!(f, "\n\tdataDir\t\t:{}", self.data_dir)?;
        write!(f, "\n\tlogLevel\t:{:?}", self.log_level)?;
        if let Some(ref logfile) = self.log_file.as_ref() {
            write!(f, "\n\tlogFile\t\t:{}", logfile)?;
        }
        write!(f, "\n\tlogConsole\t:{}", self.log_console)?;
        write!(f, "\n\tdev mode\t:{}", self.developer_mode)?;

        if self.bootstrap_nodes.is_empty() {
            write!(f, "\n\tbootstraps\t:[]")?;
        } else {
            write!(f, "\n\tbootstraps\t:")?;
            for node in &self.bootstrap_nodes {
                write!(f, "\n\t\t- {}@{}:{}", node.id(), node.host(), node.port())?;
            }
        }

        if let Some(server_peerid) = &self.server_peerid {
            write!(f, "\n\tactiveProxy\t:")?;
            write!(f, "\n\t- serverPeerId\t:{}", server_peerid)?;
            write!(
                f,
                "\n\t- deviceKey\t:{}",
                self.device_key
                    .as_ref()
                    .map(ToString::to_string)
                    .unwrap_or_else(|| "<none>".to_string())
            )?;
            write!(
                f,
                "\n\t- upstreamHost\t:{}",
                self.upstream_host.as_deref().unwrap_or("<none>")
            )?;
            write!(
                f,
                "\n\t- upstreamPort\t:{}",
                self.upstream_port
                    .map(|v| v.to_string())
                    .unwrap_or_else(|| "<none>".to_string())
            )?;
        }
        write!(f, "\n")?;
        Ok(())
    }
}
