use std::{
    fmt,
    hash::{Hash, Hasher},
    result::Result as SResult,
    net::{
        SocketAddr,
        IpAddr,
        Ipv4Addr,
        Ipv6Addr
    },
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
    version,
    Network,
};

#[derive(Debug, Clone)]
pub struct NodeInfo {
    id: Id,
    addr: SocketAddr,
    ver: i32,
}

impl NodeInfo {
    pub fn new(id: Id, addr: SocketAddr) -> Self {
        Self {id, addr, ver: 0}
    }

    pub fn set_version(&mut self, ver: i32) {
        self.ver = ver;
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

    pub fn network(&self) -> Network {
        Network::from(self.socket_addr())
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

impl Hash for NodeInfo {
    fn hash<H: Hasher>(&self, state: &mut H) {
        0x6030A.hash(state); // 'n'
        self.id.hash(state);
        self.addr.hash(state);
    }
}

impl Eq for NodeInfo {}
impl PartialEq for NodeInfo {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id && self.addr == other.addr
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
    fn serialize<S>(&self, se: S) -> SResult<S::Ok, S::Error>
    where S: Serializer,
    {
        let serde_as_json = se.is_human_readable();
        let mut s = se.serialize_tuple(3)?;
        s.serialize_element(&self.id)?;
        if serde_as_json {
            s.serialize_element(&self.addr.ip().to_string())?;
        } else {
            let addr = match self.addr.ip() {
                IpAddr::V4(addr4) => addr4.octets().to_vec(),
                IpAddr::V6(addr6) => addr6.octets().to_vec(),
            };
            s.serialize_element(&addr)?;
        }
        s.serialize_element(&self.addr.port())?;
        s.end()
    }
}

impl<'de> Deserialize<'de> for NodeInfo {
    fn deserialize<D>(de: D) -> SResult<Self, D::Error>
    where D: Deserializer<'de>,
    {
        struct ImplVisitor;
        impl<'de> Visitor<'de> for ImplVisitor {
            type Value = NodeInfo;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("NodeInfo struct")
            }

            fn visit_seq<A>(self, mut seq: A) -> SResult<Self::Value, A::Error>
            where
                A: SeqAccess<'de>,
            {
                let bad_length = || de::Error::invalid_length(0, &self);

                let id = seq.next_element::<Id>()?.ok_or_else(|| bad_length())?;
                let ip = seq.next_element::<Vec<u8>>()?.ok_or_else(|| bad_length())?;
                let port: u16 = seq.next_element()?.ok_or_else(|| bad_length())?;

                let ip = match ip.len() {
                    4 => {
                        let mut octets = [0u8; 4];
                        octets.copy_from_slice(&ip);
                        IpAddr::V4(Ipv4Addr::from(octets))
                    },
                    16 => {
                        let mut octets = [0u8; 16];
                        octets.copy_from_slice(&ip);
                        IpAddr::V6(Ipv6Addr::from(octets))
                    },
                    _ => return Err(de::Error::invalid_value(de::Unexpected::Bytes(&ip), &self)),
                };

                Ok(NodeInfo {
                    id,
                    addr: SocketAddr::new(ip, port),
                    ver: 0
                })
            }
        }

        de.deserialize_tuple(3, ImplVisitor)
    }
}
