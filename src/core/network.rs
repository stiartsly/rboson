use std::fmt;
use std::net::SocketAddr;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Network {
    IPv4 = 4,
    IPv6 = 6,
}

impl Network {
    pub fn from(addr: &SocketAddr) -> Self {
        match addr.is_ipv4() {
            true  => Network::IPv4,
            false => Network::IPv6,
        }
    }

    pub fn is_ipv4(&self) -> bool {
        self == &Network::IPv4
    }

    pub fn is_ipv6(&self) -> bool {
        self == &Network::IPv6
    }
}

impl fmt::Display for Network {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let str = match self {
            Network::IPv4 => "v4",
            Network::IPv6 => "v6",
        };
        write!(f, "{}", str)
    }
}
