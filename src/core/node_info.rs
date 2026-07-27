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
    Network,
    Result, errors::ArgumentError,
};

/// Node network information in the Boson network.
///
/// Mirrors the Java `NodeInfo`: a node is identified by its [`Id`] and
/// may carry an IPv4 address, an IPv6 address, or both.
/// The generic accessors (e.g. [`NodeInfo::socket_addr`],[`NodeInfo::host`],
/// [`NodeInfo::port`], [`NodeInfo::ip`]) operate on the
/// [`NodeInfo::default_family`], which defaults to IPv4 when both are present.
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

impl Serialize for NodeInfo {
    fn serialize<S>(&self, se: S) -> SResult<S::Ok, S::Error>
    where S: Serializer,
    {
        // Wire format carries a single address per NodeInfo
        // (the DHT protocol already segregates entries into separate v4/v6 lists),
        // so we serialize the default one.
        let addr = self.address();
        let serde_as_json = se.is_human_readable();
        let mut s = se.serialize_tuple(3)?;
        s.serialize_element(&self.id)?;
        if serde_as_json {
            s.serialize_element(&addr.ip().to_string())?;
        } else {
            let octets = match addr.ip() {
                IpAddr::V4(addr4) => addr4.octets().to_vec(),
                IpAddr::V6(addr6) => addr6.octets().to_vec(),
            };
            s.serialize_element(&octets)?;
        }
        s.serialize_element(&addr.port())?;
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
                formatter.write_str("a tuple of 3 elements: (u64, IpAddr, u16) for NodeInfo")
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

                Ok(NodeInfo::new(id, SocketAddr::new(ip, port)))
            }
        }

        de.deserialize_tuple(3, ImplVisitor)
    }
}
