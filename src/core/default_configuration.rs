use std::env;
use std::fmt;
use std::fs;
use std::net::{
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
    Error,
    core::{
        config::Config,
        config::UserConfig,
        config::MessagingConfig,
        config::ActiveProxyConfig,
        Result
    },
    dht::DEFAULT_DHT_PORT,
};

#[derive(Clone, Deserialize)]
struct NodeItem {
    #[serde(rename = "id")]
    #[serde(deserialize_with = "Id::deserialize")]
    id      :Id,
    #[serde(rename = "address")]
    addr    :String,
    #[serde(rename = "port")]
    port    :u16,
}

impl fmt::Display for NodeItem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.id, self.addr)
    }
}

#[derive(Clone, Deserialize)]
struct LogCfg {
    #[serde(rename = "level")]
    level   : String,
    #[serde(rename = "logFile")]
    file    : Option<String>,

    #[serde(skip)]
    deserde_level: Option<LevelFilter>,
}

#[derive(Clone, Deserialize)]
struct UserCfg {
    #[serde(rename = "name")]
    name    :   Option<String>,
    #[serde(rename = "password")]
    password: Option<String>,
    #[serde(rename = "privateKey")]
    sk      : String
}

#[derive(Clone, Deserialize)]
struct ActiveProxyCfg {
    #[serde(rename = "serverPeerId")]
    server_peerid   : String,

    #[serde(rename = "peerPrivateKey")]
    peer_sk         : Option<String>,
    #[serde(rename = "domainName")]
    domain          : Option<String>,
    #[serde(rename = "upstreamHost")]
    upstream_host   : String,
    #[serde(rename = "upstreamPort")]
    upstream_port   : u16
}

#[derive(Clone, Deserialize)]
struct MessagingCfg {
    #[serde(rename = "serverPeerId")]
    server_peerid: String,
}

#[derive(Clone, Deserialize)]
struct Configuration {
    #[serde(rename = "ipv4")]
    ipv4        : bool,
    #[serde(rename = "ipv6")]
    ipv6        : bool,
    #[serde(rename = "port")]
    port        : u16,
    #[serde(rename = "dataDir")]
    data_dir    : String,

    #[serde(rename = "logger")]
    logger      : Option<LogCfg>,

    #[serde(rename = "user")]
    user        : Option<UserCfg>,

    #[serde(rename = "bootstraps")]
    bootstraps  : Vec<NodeItem>,

    #[serde(rename = "activeproxy")]
    activeproxy : Option<ActiveProxyCfg>,
    #[serde(rename = "messaging")]
    messaging   : Option<MessagingCfg>,


    #[serde(skip)]
    deserde_addr4   : Option<SocketAddr>,
    #[serde(skip)]
    deserde_addr6   : Option<SocketAddr>,
    #[serde(skip)]
    deserde_nodes   : Option<Vec<NodeInfo>>,
}

pub struct Builder<'a> {
    auto_ipv4   : bool,
    auto_ipv6   : bool,
    ipv4_str    : Option<&'a str>,
    ipv6_str    : Option<&'a str>,

    port        : u16,
    data_dir    : Option<String>,

    log_level   : Option<LevelFilter>,
    log_file    : Option<&'a str>,

    bootstraps  : Vec<NodeInfo>,
    cfg         : Option<Configuration>,
}

impl<'a> Builder<'a> {
    pub fn new() -> Builder<'a> {
        Self {
            auto_ipv4   : false,
            auto_ipv6   : false,
            ipv4_str    : None,
            ipv6_str    : None,
            port        : DEFAULT_DHT_PORT,
            data_dir    : None, //env::var("HOME").unwrap_or_else(|_| ".".into()),
            log_level   : None,
            log_file    : None,
            bootstraps  : Vec::new(),
            cfg         : None,
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

    pub fn with_ipv4(&mut self, ipv4: &'a str) -> &mut Self {
        self.auto_ipv4 = false;
        self.ipv4_str = Some(ipv4);
        self
    }

    pub fn with_ipv6(&mut self, ipv6: &'a str) -> &mut Self {
        self.auto_ipv6 = false;
        self.ipv6_str = Some(ipv6);
        self
    }

    pub fn with_port(&mut self, port: u16) -> &mut Self {
        self.port = port;
        self
    }

    pub fn with_data_dir(&mut self, input: &str) -> &mut Self {
        let mut data_dir = String::new();
        if input.starts_with("~") {
            data_dir += &input[1..];
        } else {
            data_dir += input;
        }
        self.data_dir = Some(data_dir);
        self
    }

    pub fn with_logger(&mut self, level: LevelFilter, file: Option<&'a str>) -> &mut Self {
        self.log_level = Some(level);
        self.log_file = file;
        self
    }

    pub fn with_bootstrap_node(&mut self, node: &NodeInfo) -> &mut Self {
        self.bootstraps.push(node.clone());
        self
    }

    pub fn with_bootstrap_nodes(&mut self, nodes: Vec<NodeInfo>) ->&mut Self {
        self.bootstraps.extend(nodes);
        self
    }

    pub fn load(&mut self, input: &str) -> Result<&mut Self> {
        let data = fs::read_to_string(input).map_err(|e| {
            Error::Io(format!("Reading config error: {}", e))
        })?;

        let cfg = serde_json::from_str::<Configuration>(&data).map_err(|e| {
            Error::Argument(format!("bad config, error: {}", e))
        })?;

        self.cfg = Some(cfg);
        Ok(self)
    }

    pub fn build(&mut self) -> Result<Box<dyn Config>> {
        Ok(Box::new(Configuration::new(self)?))
    }
}

impl Configuration {
    fn new(b: &Builder) -> Result<Self>{
        let mut cfg = match b.cfg.as_ref() {
            Some(cfg) => cfg.clone(),
            None => Self {
                ipv4            : true,
                ipv6            : false,
                port            : DEFAULT_DHT_PORT,
                data_dir        : env::var("HOME").unwrap_or_else(|_| ".".into()),
                logger          : None,
                bootstraps      : Vec::new(),
                activeproxy     : None,
                messaging       : None,
                user            : None,
                deserde_addr4   : None,
                deserde_addr6   : None,
                deserde_nodes   : None,

            }
        };

        if b.port != DEFAULT_DHT_PORT && b.port != cfg.port {
            cfg.port = b.port
        };

        if cfg.ipv4 {
            let ipv4 = if b.auto_ipv4 {
                Some(local_addr(true)?)
            } else if let Some(addr) = b.ipv4_str {
                Some(addr.parse::<Ipv4Addr>()?.into())
            } else {
                Some(local_addr(true)?)
            };

            cfg.deserde_addr4 = ipv4.map(|addr| {
                SocketAddr::new(addr, b.port)
            });
        }

        if cfg.ipv6 {
            let ipv6 = if b.auto_ipv6 {
                Some(local_addr(false)?)
            } else if let Some(addr) = b.ipv6_str {
                Some(addr.parse::<Ipv6Addr>()?.into())
            } else {
                None
            };

            cfg.deserde_addr6 = ipv6.map(|addr| {
                SocketAddr::new(addr, b.port)
            });
        }

        if let Some(dir) = b.data_dir.as_ref() {
            cfg.data_dir = dir.to_string();
        }

        cfg.deserde_nodes = Some(b.bootstraps.iter()
            .map(|v| v.clone())
            .collect::<Vec<NodeInfo>>());

        cfg.deserde_nodes.as_mut().unwrap().extend(
            cfg.bootstraps.iter().filter_map(|v| {
                let saddr = format!("{}:{}", v.addr, v.port).parse().ok()?;
                Some(NodeInfo::new(v.id.clone(), saddr))
            })
        );

        if let Some(ref mut logger) = cfg.logger {
            if let Some(level) = logger.level.parse::<LevelFilter>().ok() {
                logger.deserde_level = Some(level);
            } else {
                logger.deserde_level = Some(LevelFilter::Info);
            }
        } else {
            cfg.logger = Some(LogCfg {
                level: b.log_level.unwrap_or(LevelFilter::Info).to_string(),
                file: b.log_file.map(|f| f.to_string()),
                deserde_level: Some(b.log_level.unwrap_or(LevelFilter::Info)),
            });
        }

        Ok(cfg)
    }
}

impl Config for Configuration {
    fn addr4(&self) -> Option<&SocketAddr> {
        self.deserde_addr4.as_ref()
    }

    fn addr6(&self) -> Option<&SocketAddr> {
        self.deserde_addr6.as_ref()
    }

    fn port(&self) -> u16 {
        self.port
    }

    fn access_control_dir(&self) -> Option<&str> {
        unimplemented!()
    }

    fn data_dir(&self) -> &str {
        &self.data_dir
    }

    fn bootstrap_nodes(&self) -> Vec<NodeInfo> {
        self.bootstraps.iter().filter_map(|v| {
            let saddr = format!("{}:{}", v.addr, v.port).parse().ok()?;
            Some(NodeInfo::new(v.id.clone(), saddr))
        }).collect::<Vec<NodeInfo>>()
    }

    fn log_level(&self) -> LevelFilter {
        self.logger.as_ref()
            .and_then(|v| v.deserde_level)
            .unwrap_or(LevelFilter::Info)
    }

    fn log_file(&self) -> Option<String> {
        self.logger.as_ref().and_then(|v| v.file.clone())
    }

    fn activeproxy(&self) -> Option<Box<dyn ActiveProxyConfig>> {
        self.activeproxy.as_ref().map(|v|
            Box::new(v.clone()) as Box<dyn ActiveProxyConfig>
        )
    }

    fn user(&self) -> Option<Box<dyn UserConfig>> {
        self.user.as_ref().map(|v|
            Box::new(v.clone()) as Box<dyn UserConfig>
        )
    }

    fn messaging(&self) -> Option<Box<dyn MessagingConfig>> {
        self.messaging.as_ref().map(|v|
            Box::new(v.clone()) as Box<dyn MessagingConfig>
        )
    }

    #[cfg(feature = "inspect")]
    fn dump(&self) {
        println!("config: {}", self);
    }
}

impl ActiveProxyConfig for ActiveProxyCfg {
    fn server_peerid(&self) -> &str {
        &self.server_peerid
    }

    fn peer_private_key(&self) -> Option<&str> {
        self.peer_sk.as_deref()
    }

    fn domain_name(&self) -> Option<&str> {
        self.domain.as_deref()
    }

    fn upstream_host(&self) -> &str {
        &self.upstream_host
    }

    fn upstream_port(&self) -> u16 {
        self.upstream_port
    }
}

impl UserConfig for UserCfg {
    fn private_key(&self) -> &str {
        &self.sk
    }

    fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    fn password(&self) -> Option<&str> {
        self.password.as_deref()
    }
}

impl MessagingConfig for MessagingCfg {
    fn server_peerid(&self) -> &str {
        &self.server_peerid
    }
}

impl fmt::Display for Configuration {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.deserde_addr4.as_ref().map(|addr| {
            write!(f, "ipv4:{},", addr).ok();
        });
        self.deserde_addr6.as_ref().map(|addr| {
            write!(f, "ipv6:{},", addr).ok();
        });

        write!(f, "\tstore:{},", self.data_dir)?;
        write!(f, "\tbootstraps: [")?;
        for item in self.bootstraps.iter() {
            write!(f, "\t{}, ", item)?;
        }
        // TODO:
        write!(f, "]")?;
        Ok(())
    }
}
