use std::{
    fmt,
    hash::{Hash, Hasher},
    result::Result as StdResult,
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
    Network,
    Result, errors::{Error, ArgumentError},
};

/// Node network information in the Boson network.
#[derive(Serialize, Deserialize)]
#[serde(into = "SerdeNodeInfo", try_from = "SerdeNodeInfo")]
#[derive(Debug, Clone)]
pub struct NodeInfo {
    id: Id,
    addr4: Option<SocketAddr>,
    addr6: Option<SocketAddr>,
    default_family: Network
}

impl NodeInfo {
    pub fn new(id: Id, addr: SocketAddr) -> Self {
        let default_family = Network::from(&addr);
        let addrs = match default_family {
            Network::IPv4 => (Some(addr), None),
            Network::IPv6 => (None, Some(addr)),
        };

        Self {
            id,
            addr4: addrs.0,
            addr6: addrs.1,
            default_family
        }
    }

    /// Constructs a `NodeInfo` with an optional IPv4 and an optional IPv6 socket address.
    pub fn with_addresses(
        id: Id,
        addr4: Option<SocketAddr>,
        addr6: Option<SocketAddr>,
    ) -> Result<Self> {
        if !(addr4.is_some() || addr6.is_some()) {
            return Err(ArgumentError::new("At least one address must be specified"));
        }
        if let Some(addr) = addr4 {
            if !addr.is_ipv4() {
                return Err(ArgumentError::new(format!("Invalid IPv4 address: {}", addr)));
            }
            if addr.port() == 0 {
                return Err(ArgumentError::new("Invalid port of IPv4 address: 0"));
            }
        }
        if let Some(addr) = addr6 {
            if !addr.is_ipv6() {
                return Err(ArgumentError::new(format!("Invalid IPv6 address: {}", addr)));
            }
            if addr.port() == 0 {
                return Err(ArgumentError::new("Invalid port of IPv6 address: 0"));
            }
        }

        let preferred_family = match addr4.is_some() {
            true => Network::IPv4,
            false => Network::IPv6,
        };
        Ok(Self {
            id,
            addr4,
            addr6,
            default_family: preferred_family
        })
    }

    pub const fn id(&self) -> &Id {
        &self.id
    }

    /// Returns a view of this node narrowed to a single protocol family,
    /// dropping any address of the other family.
    ///
    /// # Panics
    /// Panics if no address of the requested family is available.
    pub fn narrow_down(&self, family: Network) -> NodeInfo {
        let addr = match family {
            Network::IPv4 => self.addr4,
            Network::IPv6 => self.addr6,
        };

        let addr = addr.unwrap_or_else(|| panic!("No {} address is available", family));

        if !self.has_multi_addresses() {
            return self.clone();
        }

        Self::new(self.id.clone(), addr)
    }

    pub fn has_address(&self, family: Network) -> bool {
        match family {
            Network::IPv4 => self.addr4.is_some(),
            Network::IPv6 => self.addr6.is_some(),
        }
    }

    pub fn has_address4(&self) -> bool {
        self.addr4.is_some()
    }

    pub fn has_address6(&self) -> bool {
        self.addr6.is_some()
    }

    pub fn has_multi_addresses(&self) -> bool {
        self.addr4.is_some() && self.addr6.is_some()
    }

    pub const fn default_family(&self) -> Network {
        self.default_family
    }

    pub fn addresses(&self) -> Vec<SocketAddr> {
        let mut addrs = Vec::with_capacity(2);
        if let Some(addr) = self.addr4 {
            addrs.push(addr);
        }
        if let Some(addr) = self.addr6 {
            addrs.push(addr);
        }
        addrs
    }

    /// Gets the socket address of the node for the default family.
    pub fn address(&self) -> &SocketAddr {
        self.addr4.as_ref()
            .or(self.addr6.as_ref())
            .expect("NodeInfo must have at least one address")
    }

    /// Gets the socket address of the node for the given protocol family, if available.
    pub fn address_for(&self, family: Network) -> Option<&SocketAddr> {
        match family {
            Network::IPv4 => self.addr4.as_ref(),
            Network::IPv6 => self.addr6.as_ref(),
        }
    }

    pub fn address4(&self) -> Option<&SocketAddr> {
        self.addr4.as_ref()
    }

    pub fn address6(&self) -> Option<&SocketAddr> {
        self.addr6.as_ref()
    }

    /// Gets the IP address of the node for the preferred family.
    pub fn ip(&self) -> IpAddr {
        self.address().ip()
    }

    /// Gets the IP address of the node for the given protocol family, if available.
    pub fn ip_for(&self, family: Network) -> Option<IpAddr> {
        self.address_for(family).map(|addr| addr.ip())
    }

    pub fn ip4(&self) -> Option<IpAddr> {
        self.addr4.map(|addr| addr.ip())
    }

    pub fn ip6(&self) -> Option<IpAddr> {
        self.addr6.map(|addr| addr.ip())
    }

    /// Returns the string form of the IP address for the preferred family.
    pub fn host(&self) -> String {
        self.ip().to_string()
    }

    /// Returns the string form of the IP address for the given protocol family, if available.
    pub fn host_for(&self, family: Network) -> Option<String> {
        self.ip_for(family).map(|ip| ip.to_string())
    }

    pub fn host4(&self) -> Option<String> {
        self.ip4().map(|ip| ip.to_string())
    }

    pub fn host6(&self) -> Option<String> {
        self.ip6().map(|ip| ip.to_string())
    }

    /// Gets the port number of the node for the preferred family.
    pub fn port(&self) -> u16 {
        self.address().port()
    }

    /// Gets the port number of the node for the given protocol family, if available.
    pub fn port_for(&self, family: Network) -> Option<u16> {
        self.address_for(family).map(|addr| addr.port())
    }

    pub fn port4(&self) -> Option<u16> {
        self.addr4.map(|addr| addr.port())
    }

    pub fn port6(&self) -> Option<u16> {
        self.addr6.map(|addr| addr.port())
    }

    /// Checks whether this node info conflicts with another, i.e. they share the same id
    /// *or* the same socket address (IPv4 or IPv6). This is a partial match used to
    /// detect identity/address collisions, not full equality (see [`PartialEq`]).
    pub fn matches(&self, other: &NodeInfo) -> bool {
        self.id == other.id ||
            ((self.addr4.is_some() || other.addr4.is_some()) && self.addr4 == other.addr4) ||
            ((self.addr6.is_some() || other.addr6.is_some()) && self.addr6 == other.addr6)
    }
}

impl Hash for NodeInfo {
    fn hash<H: Hasher>(&self, state: &mut H) {
        0x6030A.hash(state); // 'n'
        self.id.hash(state);
        self.addr4.hash(state);
        self.addr6.hash(state);
    }
}

impl Eq for NodeInfo {}
impl PartialEq for NodeInfo {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id &&
        self.addr4 == other.addr4 &&
        self.addr6 == other.addr6
    }
}

impl fmt::Display for NodeInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}@", self.id)?;
        if let Some(addr4) = &self.addr4 {
            write!(f, "{}:{}", addr4.ip(), addr4.port())?;
        }
        if self.has_multi_addresses() {
            write!(f, "|")?;
        }
        if let Some(addr6) = &self.addr6 {
            write!(f, " [{}]:{}", addr6.ip(), addr6.port())?;
        }
        Ok(())
    }
}

struct SerdeNodeInfo {
    id: Id,

    addr4: Option<EncodedIpAddr>,
    port4: Option<u16>,

    addr6: Option<EncodedIpAddr>,
    port6: Option<u16>
}

struct EncodedIpAddr(IpAddr);

impl Serialize for EncodedIpAddr {
    fn serialize<S>(&self, serializer: S) -> StdResult<S::Ok, S::Error>
    where S: Serializer,
    {
        if serializer.is_human_readable() {
            return serializer.serialize_str(&self.0.to_string());
        }

        match self.0 {
            IpAddr::V4(addr) => serializer.serialize_bytes(&addr.octets()),
            IpAddr::V6(addr) => serializer.serialize_bytes(&addr.octets()),
        }
    }
}

impl<'de> Deserialize<'de> for EncodedIpAddr {
    fn deserialize<D>(deserializer: D) -> StdResult<Self, D::Error>
    where D: Deserializer<'de>,
    {
        struct IpAddrVisitor;

        impl IpAddrVisitor {
            fn from_bytes<E>(bytes: &[u8]) -> StdResult<EncodedIpAddr, E>
            where E: de::Error,
            {
                match bytes.len() {
                    4 => Ok(EncodedIpAddr(IpAddr::V4(Ipv4Addr::from(
                        <[u8; 4]>::try_from(bytes).unwrap()
                    )))),
                    16 => Ok(EncodedIpAddr(IpAddr::V6(Ipv6Addr::from(
                        <[u8; 16]>::try_from(bytes).unwrap()
                    )))),
                    _ => Err(de::Error::invalid_length(bytes.len(), &IpAddrVisitor)),
                }
            }
        }

        impl<'de> Visitor<'de> for IpAddrVisitor {
            type Value = EncodedIpAddr;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("an IPv4 or IPv6 address")
            }

            fn visit_str<E>(self, value: &str) -> StdResult<Self::Value, E>
            where E: de::Error,
            {
                value.parse::<IpAddr>()
                    .map(EncodedIpAddr)
                    .map_err(de::Error::custom)
            }

            fn visit_bytes<E>(self, value: &[u8]) -> StdResult<Self::Value, E>
            where E: de::Error,
            {
                Self::from_bytes(value)
            }

            fn visit_byte_buf<E>(self, value: Vec<u8>) -> StdResult<Self::Value, E>
            where E: de::Error,
            {
                Self::from_bytes(&value)
            }

            fn visit_seq<A>(self, mut sequence: A) -> StdResult<Self::Value, A::Error>
            where A: SeqAccess<'de>,
            {
                let mut bytes = Vec::with_capacity(16);
                while let Some(byte) = sequence.next_element::<u8>()? {
                    bytes.push(byte);
                }
                Self::from_bytes(&bytes)
            }
        }

        deserializer.deserialize_any(IpAddrVisitor)
    }
}

impl From<NodeInfo> for SerdeNodeInfo {
    fn from(node: NodeInfo) -> Self {
        Self {
            id: node.id,
            addr4: node.addr4.map(|addr| EncodedIpAddr(addr.ip())),
            port4: node.addr4.map(|addr| addr.port()),
            addr6: node.addr6.map(|addr| EncodedIpAddr(addr.ip())),
            port6: node.addr6.map(|addr| addr.port()),
        }
    }
}

impl TryFrom<SerdeNodeInfo> for NodeInfo {
    type Error = Error;

    fn try_from(s: SerdeNodeInfo) -> StdResult<Self, Self::Error> {
        NodeInfo::with_addresses(
            s.id,
            s.addr4.zip(s.port4).map(|(ip, port)| SocketAddr::new(ip.0, port)),
            s.addr6.zip(s.port6).map(|(ip, port)| SocketAddr::new(ip.0, port)),
        )
    }
}

impl Serialize for SerdeNodeInfo {
    fn serialize<S>(&self, se: S) -> StdResult<S::Ok, S::Error>
    where S: Serializer,
    {
        if self.addr4.is_none() && self.addr6.is_none() {
            return Err(serde::ser::Error::custom("NodeInfo must have at least one address"));
        }

        let len = if self.addr4.is_some() && self.addr6.is_some() {5} else {3};
        let mut s = se.serialize_tuple(len)?;
        s.serialize_element(&self.id)?;
        if let Some(addr) = &self.addr4 {
            let port = self.port4.as_ref().unwrap();
            s.serialize_element(addr)?;
            s.serialize_element(&port)?;
        }
        if let Some(addr) = &self.addr6 {
            let port = self.port6.as_ref().unwrap();
            s.serialize_element(addr)?;
            s.serialize_element(&port)?;
        }
        s.end()
    }
}

impl<'de> Deserialize<'de> for SerdeNodeInfo {
    fn deserialize<D>(de: D) -> StdResult<Self, D::Error>
    where D: Deserializer<'de>,
    {
        struct ImplVisitor;
        impl<'de> Visitor<'de> for ImplVisitor {
            type Value = SerdeNodeInfo;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a tuple of 3 or 5 elements for NodeInfo")
            }

            fn visit_seq<A>(self, mut seq: A) -> StdResult<Self::Value, A::Error>
            where
                A: SeqAccess<'de>,
            {
                let bad_length = || de::Error::invalid_length(0, &self);

                let id = seq.next_element::<Id>()?.ok_or_else(bad_length)?;

                let mut addr4 = None;
                let mut port4 = None;
                let mut addr6 = None;
                let mut port6 = None;

                // Each address is encoded as an (ip, port) pair; there may be
                // one pair (addr4 or addr6) or two pairs (addr4 and addr6).
                while let Some(ip) = seq.next_element::<EncodedIpAddr>()? {
                    let port: u16 = seq.next_element()?.ok_or_else(bad_length)?;
                    match ip.0 {
                        IpAddr::V4(_) => {
                            addr4 = Some(ip);
                            port4 = Some(port);
                        },
                        IpAddr::V6(_) => {
                            addr6 = Some(ip);
                            port6 = Some(port);
                        },
                    }
                }

                if addr4.is_none() && addr6.is_none() {
                    return Err(bad_length());
                }

                Ok(SerdeNodeInfo {
                    id,
                    addr4,
                    port4,
                    addr6,
                    port6,
                })
            }
        }

        de.deserialize_tuple(5, ImplVisitor)
    }
}
