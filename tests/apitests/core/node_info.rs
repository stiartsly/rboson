use std::net::{
    IpAddr,
    Ipv4Addr,
    Ipv6Addr,
    SocketAddr
};

use boson::{
    Id,
    NodeInfo
};

/*
 * APIs for testcase
 - NodeInfo::new()
 - ip()
 - port()
 - socket_addr(),
 - id()
 - version()
 - set_version()
 - is_ipv4()
 - is_ipv6()
 - test_matches()
 - Eq
 */

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_with_ipv4() {
        let id = Id::random();
        let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 12345);
        let node = NodeInfo::new(id.clone(), addr.clone());
        assert_eq!(node.id(), &id);
        assert_eq!(node.ip(), Ipv4Addr::new(127, 0, 0, 1));
        assert_eq!(node.port(), 12345);
        assert_eq!(node.address(), &addr);
        assert_eq!(node.has_address4(), true);
        assert_eq!(node.has_address6(), false);
    }

    #[test]
    fn test_new_with_ipv6() {
        let id = Id::random();
        let addr = SocketAddr::new(
            IpAddr::V6(Ipv6Addr::new(0, 0, 0, 0, 0, 0, 0, 1)),
            12345
        );
        let node = NodeInfo::new(id.clone(), addr.clone());
        assert_eq!(node.id(), &id);
        assert_eq!(node.ip(), IpAddr::V6(Ipv6Addr::new(0, 0, 0, 0, 0, 0, 0, 1)));
        assert_eq!(node.port(), 12345);
        assert_eq!(node.address(), &addr);
        assert_eq!(node.has_address4(), false);
        assert_eq!(node.has_address6(), true);
    }

    #[test]
    fn test_matches_with_same_id() {
        let id = Id::random();
        let addr1 = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 12345);
        let addr2 = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 2)), 12333);
        let node1 = NodeInfo::new(id.clone(), addr1.clone());
        let node2 = NodeInfo::new(id.clone(), addr2.clone());
        assert_eq!(node1.matches(&node2), true);
        assert_eq!(node1.id(), node2.id());
        assert_ne!(node1.address(), node2.address());
    }

    #[test]
    fn test_matches_with_same_addr() {
        let id1 = Id::random();
        let id2 = Id::random();
        let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 12345);
        let ni1 = NodeInfo::new(id1.clone(), addr.clone());
        let ni2 = NodeInfo::new(id2.clone(), addr.clone());
        assert_eq!(ni1.matches(&ni2), true);
        assert_ne!(ni1.id(), ni2.id());
        assert_eq!(ni1.address(), ni2.address());
    }

    #[test]
    fn test_version() {
        let id = Id::random();
        let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 12345);
        let ni = NodeInfo::new(id, addr.clone());

        assert_eq!(ni.id(), &id);
        assert_eq!(ni.address(), &addr);
    }

    #[test]
    fn test_equal() {
        let id = Id::random();
        let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 12345);
        let ni1 = NodeInfo::new(id.clone(), addr.clone());
        let ni2 = NodeInfo::new(id.clone(), addr.clone());
        assert_eq!(ni1, ni2);
        assert_eq!(ni1.id(), ni2.id());
        assert_eq!(ni1.address(), ni2.address());
    }
}
