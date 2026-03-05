use std::fmt;
use std::hash::{Hash, Hasher};
use std::result::Result as SResult;
use std::net::{
    SocketAddr,
    IpAddr,
    Ipv4Addr,
    Ipv6Addr
};
use serde::{
    Serialize,
    Deserialize,
    Serializer,
    Deserializer,
    de::{self, Visitor, SeqAccess},
    ser::{SerializeTuple},
};

use super::{
    Id,
    version
};

pub(crate) trait Reachable {
    fn reachable(&self) -> bool { false }
    fn unreachable(&self) -> bool { false }
    fn set_reachable(&mut self, _: bool) {}
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

    pub const fn ip(&self) -> IpAddr {
        self.addr.ip()
    }

    pub fn host(&self) -> String {
        self.addr.ip().to_string()
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

    pub fn format_version(&self) -> String {
        version::format_version(self.ver)
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
}

impl Reachable for NodeInfo {}
impl Hash for NodeInfo {
    fn hash<H: Hasher>(&self, state: &mut H) {
        0x6030A.hash(state); // 'n'
        self.id.hash(state);
        self.addr.hash(state);
        self.ver.hash(state);
    }
}

impl fmt::Display for NodeInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f,
            "<{}@{}:{}>",
            self.id,
            self.addr.ip(),
            self.addr.port()
        )
    }
}

impl Serialize for NodeInfo {
    fn serialize<S>(&self, serializer: S) -> SResult<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut state = serializer.serialize_tuple(3)?;
        let addr = match self.addr.ip() {
            IpAddr::V4(addr4) => addr4.octets().to_vec(),
            IpAddr::V6(addr6) => addr6.octets().to_vec(),
        };

        state.serialize_element(&self.id)?;
        state.serialize_element(&addr)?;
        state.serialize_element(&self.addr.port())?;
        state.end()
    }
}

impl<'de> Deserialize<'de> for NodeInfo {
    fn deserialize<D>(deserializer: D) -> SResult<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct ImplVisitor;

        impl<'de> Visitor<'de> for ImplVisitor {
            type Value = NodeInfo;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("node info tuple")
            }

            fn visit_seq<A>(self, mut seq: A) -> SResult<Self::Value, A::Error>
            where
                A: SeqAccess<'de>,
            {
                let id: Id = seq.next_element()?
                        .ok_or_else(|| de::Error::invalid_length(0, &self))?;

                let ip_bytes: Vec<u8> = seq.next_element()?
                    .ok_or_else(||de::Error::invalid_length(1, &self))?;

                let port: u16 = seq.next_element()?
                    .ok_or_else(|| de::Error::invalid_length(2, &self))?;

                let ip = match ip_bytes.len() {
                    4 => {
                        let ip: [u8; 4] = ip_bytes.as_slice().try_into().unwrap();
                        IpAddr::V4(Ipv4Addr::from(ip))
                    },
                    16 => {
                        let ip: [u8; 16] = ip_bytes.as_slice().try_into().unwrap();
                        IpAddr::V6(Ipv6Addr::from(ip))
                    },
                    _ => return Err(de::Error::invalid_value(de::Unexpected::Bytes(&ip_bytes), &self)),
                };

                Ok(NodeInfo {
                    id,
                    addr:
                    SocketAddr::new(ip, port),
                    ver: 0
                })
            }
        }

        deserializer.deserialize_tuple(4, ImplVisitor)
    }
}
