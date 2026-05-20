use std::{
    fmt,
    env,
    fs,
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
    dht::cfg::node_config::{
        NodeConfig,
        DEFAULT_DHT_PORT
    }
};

#[derive(Debug, Clone)]
pub struct NodeConfiguration {
    host4: Option<String>,
    host6: Option<String>,
    port: u16,
    private_key: signature::PrivateKey,
    data_dir: String,
    bootstrap_nodes: Vec<NodeInfo>,
    log_level: LevelFilter,
    log_file: Option<String>,
    devp: bool,
}

#[derive(Debug, Deserialize)]
struct YamlNodeConfig {
    ipv4: Option<bool>,
    ipv6: Option<bool>,
    #[serde(default = "default_port")]
    port: u16,
    #[serde(rename = "privateKey")]
    private_key: String,
    #[serde(rename = "dataDir")]
    data_dir: Option<String>,
    #[serde(default)]
    bootstraps: Vec<YamlNodeEntry>,
    logger: Option<YamlLoggerConfig>,
    #[serde(rename = "enableDeveloperMode", default)]
    devp: bool,
}

#[derive(Debug, Deserialize)]
struct YamlLoggerConfig {
    #[serde(rename = "logLevel")]
    level: Option<String>,
    #[serde(rename = "logFile")]
    log_file: Option<String>,
}

#[derive(Debug, Deserialize)]
struct YamlNodeEntry(Id, String, u16);

fn default_port() -> u16 {
    DEFAULT_DHT_PORT
}

impl NodeConfiguration {
    pub fn from_yaml(input: &str) -> Result<Self> {
        let expanded = expand_env(input)?;
        let config = serde_yaml::from_str::<YamlNodeConfig>(&expanded)
            .map_err(|e| ArgumentError::new(format!("Invalid node.yaml content: {e}")))?;
        Self::try_from(config)
    }

    pub fn load(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        let input = fs::read_to_string(path).map_err(|e|
            IOError::new(format!("Reading config {} failed: {e}", path.display()))
        )?;
        Self::from_yaml(&input)
    }

    pub fn load_default() -> Result<Self> {
        let candidates = config_paths();
        let Some(path) = candidates.iter().find(|path| path.exists()) else {
            return Err(ArgumentError::new(format!(
                "Unable to locate node.yaml in any default location: {}",
                candidates
                    .iter()
                    .map(|path| path.display().to_string())
                    .collect::<Vec<_>>()
                    .join(", ")
            )));
        };

        Self::load(path)
    }
}

impl TryFrom<YamlNodeConfig> for NodeConfiguration {
    type Error = crate::Error;

    fn try_from(yaml: YamlNodeConfig) -> Result<Self> {
        let sk = signature::PrivateKey::try_from(yaml.private_key.as_str())?;
        let bootstrap_nodes = yaml.bootstraps.into_iter()
            .map(|entry| NodeInfo::try_from(entry))
            .collect::<Result<Vec<_>>>()?;

        let addr4 = if yaml.ipv4 .unwrap_or(false) {
            use crate::local_addr;
            Some(local_addr(true)?.to_string())
        } else {
            None
        };
        let addr6 = if yaml.ipv6.unwrap_or(false) {
            use crate::local_addr;
            Some(local_addr(false)?.to_string())
        } else {
            None
        };

        let logger = yaml.logger.as_ref();

        Ok(Self {
            host4   : addr4,
            host6   : addr6,
            port    : yaml.port,
            private_key: sk,
            data_dir: expand_data_dir(yaml.data_dir),
            bootstrap_nodes,
            log_level: log_level(logger.and_then(|logger| logger.level.as_deref())),
            log_file: logger.and_then(|logger| logger.log_file.clone()),
            devp    : yaml.devp,
        })
    }
}

impl NodeConfig for NodeConfiguration {
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

    fn bootstrap_nodes(&self) -> &[NodeInfo] {
        &self.bootstrap_nodes
    }

    fn log_level(&self) -> LevelFilter {
        self.log_level
    }

    fn log_file(&self) -> Option<&str> {
        self.log_file.as_deref()
    }

    fn enable_devp(&self) -> bool {
        self.devp
    }

    fn dump(&self) {
        println!("{}", self);
    }
}

impl TryFrom<YamlNodeEntry> for NodeInfo {
    type Error = crate::Error;

    fn try_from(value: YamlNodeEntry) -> Result<NodeInfo> {
        let YamlNodeEntry(id, host, port) = value;
        let addr = format!("{host}:{port}")
            .parse::<SocketAddr>()
            .map_err(|e|
                ArgumentError::new(format!("Invalid bootstrap node address {host}:{port}: {e}"))
        )?;
        Ok(NodeInfo::new(id, addr))
    }
}

fn log_level(level: Option<&str>) -> LevelFilter {
    level.and_then(|v| v.parse::<LevelFilter>().ok())
        .unwrap_or(LevelFilter::Info)
}

fn expand_data_dir(data_dir: Option<String>) -> String {
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
            return Err(ArgumentError::new("Unclosed environment placeholder in node.yaml".into()));
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

impl fmt::Display for NodeConfiguration {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "node config:")?;
        write!(f, "\n\thost4: {}", self.host4.as_deref().unwrap_or("<none>"))?;
        write!(f, "\n\thost6: {}", self.host6.as_deref().unwrap_or("<none>"))?;
        write!(f, "\n\tport: {}", self.port)?;
        write!(f, "\n\tprivateKey: {}", self.private_key)?;
        write!(f, "\n\tataDir: {}", self.data_dir)?;
        write!(f, "\n\tlogLevel: {:?}", self.log_level)?;
        write!(f, "\n\tlogFile: {}", self.log_file.as_deref().unwrap_or("<none>"))?;
        write!(f, "\n\tenableDeveloperMode: {}", self.devp)?;

        if self.bootstrap_nodes.is_empty() {
            write!(f, "\n\tbootstraps: []")?;
        } else {
            write!(f, "\n\tbootstraps:")?;
            for node in &self.bootstrap_nodes {
                write!(f, "\n\t- {} {} {}", node.id(), node.host(), node.port())?;
            }
        }
        Ok(())
    }
}
