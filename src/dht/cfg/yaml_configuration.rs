use std::env;
use std::fs;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};

use log::LevelFilter;
use serde::Deserialize;

use crate::{
    errors::{IOError, ArgumentError},
    Id,
    NodeInfo,
    Result,
    signature,
};

use super::node_config::{NodeConfig, DEFAULT_DHT_PORT};

#[derive(Debug, Clone)]
pub struct YamlNodeConfiguration {
    host4: Option<String>,
    host6: Option<String>,
    port: u16,
    private_key: signature::PrivateKey,
    data_dir: String,
    bootstrap_nodes: Vec<NodeInfo>,
    log_level: LevelFilter,
    log_file: Option<String>,
    enable_devp: bool,
}

#[derive(Debug, Deserialize)]
struct RawNodeConfig {
    host4: Option<String>,
    host6: Option<String>,
    #[serde(default = "default_port")]
    port: u16,
    #[serde(rename = "privateKey")]
    private_key: String,
    #[serde(rename = "dataDir")]
    data_dir: Option<String>,
    #[serde(default)]
    bootstraps: Vec<BootstrapEntry>,
    logger: Option<RawLoggerConfig>,
    #[serde(rename = "enableDeveloperMode", default)]
    enable_developer_mode: bool,
}

#[derive(Debug, Deserialize)]
struct RawLoggerConfig {
    level: Option<String>,
    #[serde(rename = "logFile")]
    log_file: Option<String>,
}

#[derive(Debug, Deserialize)]
struct BootstrapEntry(Id, String, u16);

fn default_port() -> u16 {
    DEFAULT_DHT_PORT
}

impl YamlNodeConfiguration {
    pub fn from_yaml(input: &str) -> Result<Self> {
        let expanded = expand_env_placeholders(input)?;
        let raw = serde_yaml::from_str::<RawNodeConfig>(&expanded)
            .map_err(|e| ArgumentError::new(format!("Invalid node.yaml content: {e}")))?;
        Self::try_from_raw(raw)
    }

    pub fn load(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        let input = fs::read_to_string(path)
            .map_err(|e| IOError::new(format!("Reading config {} failed: {e}", path.display())))?;
        Self::from_yaml(&input)
    }

    pub fn load_default() -> Result<Self> {
        let candidates = default_config_paths();
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

    fn try_from_raw(raw: RawNodeConfig) -> Result<Self> {
        let private_key = signature::PrivateKey::try_from(raw.private_key.as_str())?;
        let bootstrap_nodes = raw.bootstraps.into_iter()
            .map(|entry| bootstrap_entry_to_node_info(entry))
            .collect::<Result<Vec<_>>>()?;

        Ok(Self {
            host4: raw.host4,
            host6: raw.host6,
            port: raw.port,
            private_key,
            data_dir: expand_data_dir(raw.data_dir),
            bootstrap_nodes,
            log_level: parse_log_level(raw.logger.as_ref().and_then(|logger| logger.level.as_deref())),
            log_file: raw.logger.and_then(|logger| logger.log_file),
            enable_devp: raw.enable_developer_mode,
        })
    }
}

impl NodeConfig for YamlNodeConfiguration {
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
        self.enable_devp
    }
}

fn bootstrap_entry_to_node_info(entry: BootstrapEntry) -> Result<NodeInfo> {
    let BootstrapEntry(id, host, port) = entry;
    let addr = format!("{host}:{port}")
        .parse::<SocketAddr>()
        .map_err(|e| ArgumentError::new(format!("Invalid bootstrap node address {host}:{port}: {e}")))?;
    Ok(NodeInfo::new(id, addr))
}

fn parse_log_level(level: Option<&str>) -> LevelFilter {
    level
        .and_then(|value| value.parse::<LevelFilter>().ok())
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

fn expand_env_placeholders(input: &str) -> Result<String> {
    let mut output = String::with_capacity(input.len());
    let mut cursor = 0;

    while let Some(offset) = input[cursor..].find("${") {
        let start = cursor + offset;
        output.push_str(&input[cursor..start]);

        let var_start = start + 2;
        let Some(end_offset) = input[var_start..].find('}') else {
            return Err(ArgumentError::new("Unclosed environment placeholder in node.yaml".into()));
        };
        let end = var_start + end_offset;
        let name = &input[var_start..end];
        let value = env::var(name)
            .map_err(|_| ArgumentError::new(format!("Environment variable {name} is not set")))?;
        output.push_str(&value);
        cursor = end + 1;
    }

    output.push_str(&input[cursor..]);
    Ok(output)
}

fn default_config_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();

    if let Ok(current_dir) = env::current_dir() {
        paths.push(current_dir.join("node.yaml"));
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
            paths.push(PathBuf::from(home).join(".config").join("boson").join("node.yaml"));
        }
        paths.push(PathBuf::from("/usr/local/etc/boson/node.yaml"));
        paths.push(PathBuf::from("/etc/boson/node.yaml"));
    }

    paths
}
