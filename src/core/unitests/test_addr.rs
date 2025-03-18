use std::net::{
    IpAddr,
    SocketAddr
};
use crate::core::{
    is_bogon,
    is_global_unicast,
    is_any_unicast
};

#[test]
fn test_is_global_unicast() {
    assert_eq!(is_global_unicast(&"8.8.8.8".parse::<IpAddr>().unwrap()), true);
    assert_eq!(is_global_unicast(&"2001:4860:4860::8888".parse::<IpAddr>().unwrap()), true);

    //wildcard.
    assert_eq!(is_global_unicast(&"0.0.0.0".parse::<IpAddr>().unwrap()), false);
    assert_eq!(is_global_unicast(&"::0".parse::<IpAddr>().unwrap()), false);

    //Loopback.
    assert_eq!(is_global_unicast(&"127.0.0.15".parse::<IpAddr>().unwrap()), false);
    assert_eq!(is_global_unicast(&"::1".parse::<IpAddr>().unwrap()), false);

    //private/LL.
    assert_eq!(is_global_unicast(&"192.168.13.47".parse::<IpAddr>().unwrap()), false);
    assert_eq!(is_global_unicast(&"169.254.1.0".parse::<IpAddr>().unwrap()), false);
    assert_eq!(is_global_unicast(&"fe80::".parse::<IpAddr>().unwrap()), false);

    //ULA
    assert_eq!(is_global_unicast(&"::1".parse::<IpAddr>().unwrap()), false);
    assert_eq!(is_global_unicast(&"fc00::".parse::<IpAddr>().unwrap()), false);
    assert_eq!(is_global_unicast(&"fd00::".parse::<IpAddr>().unwrap()), false);
}

#[test]
fn test_is_any_unicast() {
    assert_eq!(is_any_unicast(&"192.168.1.1".parse::<IpAddr>().unwrap()), true);
    assert_eq!(is_any_unicast(&"10.0.0.1".parse::<IpAddr>().unwrap()), true);
}

#[test]
fn test_is_bogon() {
    assert_eq!(is_bogon(&"47.101.142.224:1234".parse::<SocketAddr>().unwrap()), false);
    assert_eq!(is_bogon(&"151.101.2.132:1234".parse::<SocketAddr>().unwrap()), false);

    assert_eq!(is_bogon(&"192.168.1.1:1234".parse::<SocketAddr>().unwrap()), true);
    assert_eq!(is_bogon(&"10.0.0.1:1234".parse::<SocketAddr>().unwrap()), true);
    assert_eq!(is_bogon(&"127.0.0.1:1234".parse::<SocketAddr>().unwrap()), true);

    assert_eq!(is_bogon(&"192.168.0.8:0".parse::<SocketAddr>().unwrap()), true);
}
