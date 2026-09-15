use std::fmt;
use std::time::{Duration, SystemTime};
use serde::Deserialize;
use crate::Id;

#[derive(Clone, Debug, Deserialize)]
pub struct NodeStatus {
    #[serde(rename = "nodeId")]
    node_id: Id,

    #[serde(default)]
    software: Option<String>,
    #[serde(default)]
    version: Option<String>,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    logo: Option<String>,
    #[serde(default)]
    website: Option<String>,
    #[serde(default)]
    contact: Option<String>,

    #[serde(rename = "startedAt")]
    started_at: u64,

    #[serde(default)]
    running: bool,

    #[serde(default)]
    services: Vec<Service>,
}

impl NodeStatus {
    pub fn node_id(&self) -> &Id {
        &self.node_id
    }

    pub fn software(&self) -> Option<&str> {
        self.software.as_deref()
    }

    pub fn version(&self) -> Option<&str> {
        self.version.as_deref()
    }

    pub fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    pub fn logo(&self) -> Option<&str> {
        self.logo.as_deref()
    }

    pub fn website(&self) -> Option<&str> {
        self.website.as_deref()
    }

    pub fn contact(&self) -> Option<&str> {
        self.contact.as_deref()
    }

    pub fn started_at(&self) -> SystemTime {
        SystemTime::UNIX_EPOCH + Duration::from_secs(self.started_at)
    }

    pub fn is_running(&self) -> bool {
        self.running
    }

    pub fn services(&self) -> &[Service] {
        &self.services
    }
}

impl fmt::Display for NodeStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "NodeStatus {{ node_id: {}, name: {:?}, version: {:?}, running: {}, services: {:?} }}",
            self.node_id,
            self.name,
            self.version,
            self.running,
            self.services
        )?;
        write!(f, "software: {:?}", self.software())?;
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize)]
pub struct Service {
    #[serde(rename = "serviceId")]
    service_id: String,
    #[serde(rename = "serviceName", default)]
    service_name: Option<String>,
    #[serde(rename = "peerId")]
    pub peer_id: Id,
    #[serde(rename = "endpoint", default)]
    pub endpoint: Option<String>,
}

#[allow(dead_code)]
impl Service {
    pub fn service_id(&self) -> &str {
        &self.service_id
    }

    pub fn service_name(&self) -> Option<&str> {
        self.service_name.as_deref()
    }

    pub fn peer_id(&self) -> &Id {
        &self.peer_id
    }

    pub fn endpoint(&self) -> Option<&str> {
        self.endpoint.as_deref()
    }
}

impl fmt::Display for Service {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Service {{ service_id: {}, service_name: {:?}, peer_id: {}, endpoint: {:?} }}",
            self.service_id,
            self.service_name,
            self.peer_id,
            self.endpoint
        )
    }
}
