use std::env;
use std::fmt;
use std::net::{
    IpAddr,
    Ipv4Addr,
    Ipv6Addr,
    SocketAddr
};
use std::fs;
use serde::Deserialize;

use crate::{
    local_addr,
    Id,
    NodeInfo,
    Config,
    Error,
    error::Result
};

use crate::core::{
    constants
};

#[derive(Deserialize)]
struct CfgNode {
    id: String,
    address: String,
    port: u16,
}

#[derive(Deserialize)]
#[allow(non_snake_case)]
struct Cfg {
    ipv4: bool,
    ipv6: bool,
    port: u16,
    dataDir: String,
    bootstraps: Vec<CfgNode>,
}

pub struct Builder<'a> {
    auto_ipv4: bool,
    auto_ipv6: bool,
    ipv4: Option<&'a str>,
    ipv6: Option<&'a str>,
    port: u16,
    data_dir: String,
    bootstrap_nodes: Vec<NodeInfo>,
}

impl<'a> Builder<'a> {
    pub fn new() -> Builder<'a> {
        Self {
            auto_ipv4: false,
            auto_ipv6: false,
            ipv4: None,
            ipv6: None,
            port: constants::DEFAULT_DHT_PORT,
            data_dir: env::var("HOME").unwrap_or_else(|_| String::from(".")),
            bootstrap_nodes: Vec::new(),
        }
    }

    pub fn with_auto_ipv4(mut self) -> Self {
        self.auto_ipv4 = true;
        self.ipv4 = None;
        self
    }

    pub fn with_auto_ipv6(mut self) -> Self {
        self.auto_ipv6 = true;
        self.ipv6 = None;
        self
    }

    pub fn with_ipv4(mut self, input: &'a str) -> Self {
        self.auto_ipv4 = false;
        self.ipv4 = Some(input);
        self
    }

    pub fn with_ipv6(mut self, input: &'a str) -> Self {
        self.auto_ipv6 = false;
        self.ipv6 = Some(input);
        self
    }

    pub fn with_listening_port(mut self, port: u16) -> Self {
        self.port = port;
        self
    }

    pub fn with_storage_path(mut self, input: &'a str) -> Self {
        if input.starts_with("~") {
            self.data_dir += &input[1..];
        } else {
            self.data_dir = input.to_string();
        }
        self
    }

    pub fn add_bootstrap_node(mut self, node: &NodeInfo) -> Self {
        self.bootstrap_nodes.push(node.clone());
        self
    }

    pub fn add_bootstrap_nodes(mut self, nodes: &[NodeInfo]) -> Self {
        for item in nodes.iter() {
            self.bootstrap_nodes.push(item.clone())
        }
        self
    }

    pub fn load(mut self, input: &str) -> Result<Self> {
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
        Ok(self)
    }

    pub(crate) fn check_valid(&self) -> Result<()> {
        if self.port == 0 {
            return Err(Error::Argument(format!("error: port can't be 0")));
        }

        self.ipv4.as_ref().map(|addr| {
            addr.parse::<Ipv4Addr>().map_err(|e| {
                return Error::Argument(format!("error: {}", e));
            }).ok();
        });
        self.ipv6.as_ref().map(|addr| {
            addr.parse::<Ipv4Addr>().map_err(|e| {
                return Error::Argument(format!("error: {}", e));
            }).ok();
        });

        if self.ipv4.is_none() && self.ipv6.is_none() &&
            !self.auto_ipv4 && !self.auto_ipv6 {
            return Err(Error::Argument(format!(
                "No valid IPv4 or IPv6 address was specified."
            )));
        }

        Ok(())
    }

    pub fn build(&self) -> Result<Box<dyn Config>> {
        self.check_valid()?;

        Ok(Box::new(DefaultConfiguration::new(self)))
    }
}

pub struct DefaultConfiguration {
    addr4: Option<SocketAddr>,
    addr6: Option<SocketAddr>,

    port: u16,

    storage_path: String,
    bootstrap_nodes: Vec<NodeInfo>,
}

impl DefaultConfiguration {
    fn new(b: &Builder) -> Self {
        let addr4 = if b.auto_ipv4 {
            Some(SocketAddr::new(
                local_addr(true).unwrap(),
                b.port
            ))
        } else {
            b.ipv4.as_ref().map(|addr| {
                SocketAddr::new(
                    IpAddr::V4(addr.parse::<Ipv4Addr>().unwrap()),
                    b.port
                )
            })
        };

        let addr6 = if b.auto_ipv6 {
            Some(SocketAddr::new(
                local_addr(false).unwrap(),
                b.port
            ))
        } else {
            b.ipv6.as_ref().map(|addr| {
                SocketAddr::new(
                    IpAddr::V6(addr.parse::<Ipv6Addr>().unwrap()),
                    b.port
                )
            })
        };

        Self {
            addr4,
            addr6,
            port: b.port,
            storage_path: b.data_dir.to_string(),
            bootstrap_nodes: b.bootstrap_nodes.clone(),
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
