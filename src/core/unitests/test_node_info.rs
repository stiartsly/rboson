use std::net::{
    IpAddr,
    Ipv4Addr,
    SocketAddr
};

use crate::core::{Id, NodeInfo};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_serde() {
        let id = Id::random();
        let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 12345);
        let ni = NodeInfo::new(id.clone(), addr);
        // ni.set_version(1);
        let ser = serde_cbor::to_vec(&ni).expect("Failed to serialize NodeInfo");
        let des: NodeInfo = serde_cbor::from_slice(&ser).expect("Failed to deserialize NodeInfo");
        assert_eq!(des, ni);
        assert_eq!(des.id(), &id);
        assert_eq!(des.version(), 0); // Lost version information.
    }
}
