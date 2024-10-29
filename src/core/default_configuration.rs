use std::env;
use std::fmt;
use std::fs;
use std::net::{
    IpAddr,
    Ipv4Addr,
    Ipv6Addr,
    SocketAddr
};
use serde::Deserialize;
use log::LevelFilter;

use crate::{
    local_addr,
    Id,
    NodeInfo,
    config,
    Config,
    Error,
    error::Result
};

use crate::core::{
    constants,
    logger,
};

#[derive(Deserialize)]
struct CfgNode {
    id: String,
    address: String,
    port: u16,
}

#[derive(Deserialize)]
#[allow(non_snake_case)]
struct Logger {
    level: String,
    logFile: Option<String>,
    // pattern: String
}

#[derive(Deserialize)]
#[allow(non_snake_case)]
struct ActiveProxyItem {
    serverPeerId: String,
    peerPrivateKey: Option<String>,
    domainName: Option<String>,
    upstreamHost: String,
    upstreamPort: u16
}

#[derive(Deserialize)]
#[allow(non_snake_case)]
struct Cfg {
    ipv4: bool,
    ipv6: bool,
    port: u16,
    dataDir: String,
    logger: Option<Logger>,
    bootstraps: Vec<CfgNode>,
    activeproxy: Option<ActiveProxyItem>
}

pub struct Builder<'a> {
    auto_ipv4:      bool,
    auto_ipv6:      bool,
    ipv4_str:       Option<&'a str>,
    ipv6_str:       Option<&'a str>,

    ipv4_addr:      Option<IpAddr>,
    ipv6_addr:      Option<IpAddr>,

    port:           u16,
    data_dir:       String,

    log_level:      LevelFilter,
    log_file:       Option<String>,

    bootstrap_nodes:Vec<NodeInfo>,
    activeproxy: Option<ActiveProxyItem>,
}

impl<'a> Builder<'a> {
    pub fn new() -> Builder<'a> {
        Self {
            auto_ipv4:      false,
            auto_ipv6:      false,
            ipv4_str:       None,
            ipv6_str:       None,
            ipv4_addr:      None,
            ipv6_addr:      None,
            port:           constants::DEFAULT_DHT_PORT,
            data_dir:       env::var("HOME").unwrap_or_else(|_| String::from(".")),
            log_level:      LevelFilter::Info,
            log_file:       None,
            activeproxy:    None,
            bootstrap_nodes:Vec::new(),
        }
    }

    pub fn with_auto_ipv4(&mut self) -> &mut Self {
        self.auto_ipv4 = true;
        self.ipv4_str = None;
        self
    }

    pub fn with_auto_ipv6(&mut self) -> &mut Self {
        self.auto_ipv6 = true;
        self.ipv6_str = None;
        self
    }

    pub fn with_ipv4(&mut self, input: &'a str) -> &mut Self {
        self.auto_ipv4 = false;
        self.ipv4_str = Some(input);
        self
    }

    pub fn with_ipv6(&mut self, input: &'a str) -> &mut Self {
        self.auto_ipv6 = false;
        self.ipv6_str = Some(input);
        self
    }

    pub fn with_listening_port(&mut self, port: u16) -> &mut Self {
        self.port = port;
        self
    }

    pub fn with_storage_path(&mut self, input: &str) -> &mut Self {
        if input.starts_with("~") {
            self.data_dir += &input[1..];
        } else {
            self.data_dir = input.to_string();
        }
        self
    }

    pub fn add_bootstrap_node(&mut self, node: &NodeInfo) -> &mut Self {
        self.bootstrap_nodes.push(node.clone());
        self
    }

    pub fn add_bootstrap_nodes(&mut self, nodes: &[NodeInfo]) ->&mut Self {
        for item in nodes.iter() {
            self.bootstrap_nodes.push(item.clone())
        }
        self
    }

    pub fn load(&mut self, input: &str) -> Result<&mut Self> {
        let data = match fs::read_to_string(input) {
            Ok(v) => v,
            Err(e) => return Err(Error::Io(
                format!("Reading config error: {}", e))),
        };

        let cfg: Cfg = match serde_json::from_str(&data) {
            Ok(cfg) => cfg,
            Err(e) => return Err(Error::Argument(format!("bad config, error: {}", e)))
        };

        if cfg.port > 0 {
            self.port = cfg.port;
        }
        if cfg.ipv4 {
            self.auto_ipv4 = true;
        }
        if cfg.ipv6 {
            self.auto_ipv6 = true;
        }

        self.data_dir = cfg.dataDir.clone();

        for item in cfg.bootstraps {
            let id = match Id::try_from_base58(&item.id) {
                Ok(id) => id,
                Err(e) => return Err(Error::Argument(format!("bad id {}, error: {}", item.id, e)))
            };
            let ip = match item.address.parse::<IpAddr>() {
                Ok(ip) => ip,
                Err(e) => return Err(Error::Argument(format!("bad address {}, error: {}", item.address, e)))
            };
            self.bootstrap_nodes.push(
                NodeInfo::new(id, SocketAddr::new(ip, item.port))
            )
        }

        if let Some(logger) = cfg.logger {
            self.log_level = logger::convert_loglevel(&logger.level);
            self.log_file = logger.logFile;
        }

        self.activeproxy = cfg.activeproxy;
        Ok(self)
    }

    pub fn build(&mut self) -> Result<Box<dyn Config>> {
        if let Some(addr) = self.ipv4_str.as_ref() {
            self.ipv4_addr = Some(IpAddr::V4(addr.parse::<Ipv4Addr>()?));
        }
        if let Some(addr) = self.ipv6_str.as_ref() {
            self.ipv6_addr = Some(IpAddr::V6(addr.parse::<Ipv6Addr>()?));
        }

        if self.auto_ipv4 {
            self.ipv4_addr = Some(local_addr(true)?);
        }
        if self.auto_ipv6 {
            self.ipv6_addr = Some(local_addr(false)?);
        }

        if self.ipv4_addr.is_none() && self.ipv6_addr.is_none() {
            return Err(Error::Argument(format!(
                "No valid IPv4 or IPv6 address was specified."
            )));
        }

        Ok(Box::new(DefaultConfiguration::new(self)))
    }
}

pub(crate) struct ActiveProxyConfiguration {
    server_peerid: String,
    peer_sk: Option<String>,
    domain_name: Option<String>,
    upstream_host: String,
    upstream_port: u16
}

impl config::ActiveProxyConfig for ActiveProxyConfiguration {
    fn server_peerid(&self) -> &str {
        &self.server_peerid
    }

    fn peer_private_key(&self) -> Option<&str> {
        self.peer_sk.as_ref().map(|v|v.as_str())
    }

    fn domain_name(&self) -> Option<&str> {
        self.domain_name.as_ref().map(|v|v.as_str())
    }

    fn upstream_host(&self) -> &str {
        &self.upstream_host
    }

    fn upstream_port(&self) -> u16 {
        self.upstream_port
    }
}

impl ActiveProxyConfiguration {
    fn new(b: &ActiveProxyItem) -> Self {
        Self {
            server_peerid:  b.serverPeerId.clone(),
            peer_sk:        b.peerPrivateKey.clone(),
            domain_name:    b.domainName.clone(),
            upstream_host:  b.upstreamHost.clone(),
            upstream_port:  b.upstreamPort
        }
    }
}

pub(crate) struct DefaultConfiguration {
    addr4: Option<SocketAddr>,
    addr6: Option<SocketAddr>,

    port: u16,

    log_level: LevelFilter,
    log_file: Option<String>,

    storage_path: String,
    bootstrap_nodes: Vec<NodeInfo>,

    activeproxy: Option<Box<dyn config::ActiveProxyConfig>>,
}

impl DefaultConfiguration {
    fn new(b: &Builder) -> Self {
        let addr4 = b.ipv4_addr.as_ref().map(|ip| {
            SocketAddr::new(ip.clone(), b.port)
        });

        let addr6 = b.ipv6_addr.as_ref().map(|ip| {
            SocketAddr::new(ip.clone(), b.port)
        });

        let activeproxy = match b.activeproxy.as_ref() {
            Some(ap) => Some(Box::new(ActiveProxyConfiguration::new(ap))),
            None => None
        };

        Self {
            addr4,
            addr6,
            port: b.port,
            log_level: b.log_level,
            log_file: b.log_file.clone(),
            storage_path: b.data_dir.to_string(),
            bootstrap_nodes: b.bootstrap_nodes.clone(),
            activeproxy: activeproxy.map(|v| v as Box<dyn config::ActiveProxyConfig>)
        }
    }
}

impl Config for DefaultConfiguration {
    fn addr4(&self) -> Option<&SocketAddr> {
        self.addr4.as_ref()
    }

    fn addr6(&self) -> Option<&SocketAddr> {
        self.addr6.as_ref()
    }

    fn listening_port(&self) -> u16 {
        self.port
    }

    fn storage_path(&self) -> &str {
        self.storage_path.as_str()
    }

    fn bootstrap_nodes(&self) -> &[NodeInfo] {
        &self.bootstrap_nodes
    }

    fn log_level(&self) -> LevelFilter {
        self.log_level
    }

    fn log_file(&self) -> Option<&str> {
        self.log_file.as_ref().map(|v|v.as_str())
    }

    fn activeproxy(&self) -> Option<&Box<dyn config::ActiveProxyConfig>> {
        self.activeproxy.as_ref()
    }

    #[cfg(feature = "inspect")]
    fn dump(&self) {
        println!("config: {}", self);
    }
}

impl fmt::Display for DefaultConfiguration {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.addr4.as_ref().map(|addr| {
            write!(f, "ipv4:{},", addr).ok();
        });
        self.addr6.as_ref().map(|addr| {
            write!(f, "ipv6:{},", addr).ok();
        });

        write!(f, "\tstorage:{},", self.storage_path)?;
        write!(f, "\tbootstraps: [")?;
        for item in self.bootstrap_nodes.iter() {
            write!(f, "\t{}, ", item)?;
        }
        write!(f, "]")?;
        Ok(())
    }
}
