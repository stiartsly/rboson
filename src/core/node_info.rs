use std::fmt;
use std::net::{
    SocketAddr,
    IpAddr,
    Ipv4Addr,
    Ipv6Addr
};
use ciborium::Value;

use crate::Id;
use crate::core::version;

pub(crate) trait Reachable {
    fn reachable(&self) -> bool;
    fn unreachable(&self) -> bool;
    fn set_reachable(&mut self, _: bool);
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NodeInfo {
    id: Id,
    addr: SocketAddr,
    ver: i32,
}

impl NodeInfo {
    pub fn new(id: Id, addr: SocketAddr) -> Self {
        Self {id, addr, ver: 0}
    }

    pub fn with_version(id: Id, addr: SocketAddr, ver: i32) -> Self {
        Self {id, addr, ver}
    }

    pub(crate) fn from_cbor(input: &Value) -> Option<Self> {
        let map = input.as_array()?;
        let id  = Id::from_cbor(map.get(0)?)?;
        let ip  = map.get(1)?.as_bytes()?;
        let port: u16 = map.get(2)?.as_integer()?.try_into().unwrap();
        let addr = match ip.len() {
            4 => {
                let ip: [u8; 4] = ip.as_slice().try_into().unwrap();
                IpAddr::V4(Ipv4Addr::from(ip))
            },
            16 => {
                let ip: [u8; 16] = ip.as_slice().try_into().unwrap();
                IpAddr::V6(Ipv6Addr::from(ip))
            },
            _ => return None,
        };
        let addr = SocketAddr::new(addr, port);

        Some(Self {id, addr, ver: 0})
    }

    pub const fn ip(&self) -> IpAddr {
        self.addr.ip()
    }

    pub const fn port(&self) -> u16 {
        self.addr.port()
    }

    pub const fn socket_addr(&self) -> &SocketAddr {
        &self.addr
    }

    pub const fn id(&self) -> &Id {
        &self.id
    }

    pub const fn version(&self) -> i32 {
        self.ver
    }

    pub fn version_str(&self) -> String {
        version::normailized_version(self.ver)
    }

    pub fn is_ipv4(&self) -> bool {
        self.addr.ip().is_ipv4()
    }

    pub fn is_ipv6(&self) -> bool {
        self.addr.ip().is_ipv6()
    }

    pub fn matches(&self, other: &NodeInfo) -> bool {
        self.id == other.id || self.addr == other.addr
    }

    pub(crate) fn to_cbor(&self) -> Value {
        let addr = match self.addr.ip() {
            IpAddr::V4(addr4) => addr4.octets().to_vec(),
            IpAddr::V6(addr6) => addr6.octets().to_vec(),
        };

        Value::Array(vec![
            self.id.to_cbor(),
            Value::Bytes(addr),
            Value::Integer(self.addr.port().into())
        ])
    }
}

impl Reachable for NodeInfo {
    fn reachable(&self) -> bool { false }
    fn unreachable(&self) -> bool { false }
    fn set_reachable(&mut self, _: bool) {}
}

impl fmt::Display for NodeInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f,
            "{},{}",
            self.id,
            self.addr
        )?;
        Ok(())
    }
}
