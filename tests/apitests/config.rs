use std::net::IpAddr;
use boson::{
    Config,
    default_configuration
};

/**
# default_configuration::Builder
 - new
 - with_auto_ipv4
 - with_auto_ipv6
 - with_ipv4
 - with_ipv6
 - with_listening_port
 - with_storage_path
 - add_bootstrap_node
 - add_bootstrap_nodes
 - load
 - build

# trait Config
 - addr4
 - addr6
 - listening_port
 - storage_path
 - bootstra_nodes
 */
#[test]
fn test_build_cfg() {
    let ipv4_str = "192.168.1.102";
    let port = 32222;
    let cfg: Box<dyn Config>;

    cfg = default_configuration::Builder::new()
        .with_listening_port(port)
        .with_ipv4(ipv4_str)
        .with_storage_path("tests")
        .build()
        .map_err(|_| assert!(false))
        .unwrap();

    assert_eq!(cfg.addr6().is_none(), true);
    assert_eq!(cfg.addr4().is_some(), true);
    assert_eq!(cfg.addr4().unwrap().is_ipv4(), true);
    assert_eq!(cfg.addr4().unwrap().port(), port);
    assert_eq!(cfg.addr4().unwrap().ip(), IpAddr::V4(ipv4_str.parse().unwrap()));
    assert_eq!(cfg.listening_port(), port);
    assert_eq!(cfg.bootstrap_nodes().len(), 0);
    assert_eq!(cfg.storage_path(), "tests");

    #[cfg(feature = "inspect")]
    cfg.dump();
}

#[test]
fn test_load_cfg() {
    let path = match std::fs::metadata("apitests.conf") {
        Ok(_) => "apitests.conf",
        Err(_) => "tests/apitests/apitests.conf",
    };
    let cfg = default_configuration::Builder::new()
        .load(path)
        .map_err(|_| assert!(false))
        .unwrap()
        .build()
        .map_err(|_| assert!(false))
        .unwrap();

    #[cfg(feature = "inspect")]
    cfg.dump();

    assert_eq!(cfg.addr4().is_some(), true);
    assert_eq!(cfg.addr6().is_some(), false);
    assert_eq!(cfg.listening_port(), 39003);
    assert_eq!(cfg.bootstrap_nodes().len(), 4);
    assert_eq!(cfg.storage_path(), "apitests_data");

    let nodes = cfg.bootstrap_nodes();
    let n1 = &nodes[0];
    assert_eq!(n1.id().to_base58(), "HZXXs9LTfNQjrDKvvexRhuMk8TTJhYCfrHwaj3jUzuhZ");
    assert_eq!(n1.ip().to_string(), "155.138.245.211");
    assert_eq!(n1.port(), 39001);

    let n2 = &nodes[1];
    assert_eq!(n2.id().to_base58(), "6o6LkHgLyD5sYyW9iN5LNRYnUoX29jiYauQ5cDjhCpWQ");
    assert_eq!(n2.ip().to_string(), "45.32.138.246");
    assert_eq!(n2.port(), 39001);

    let n3 = &nodes[2];
    assert_eq!(n3.id().to_base58(), "8grFdb2f6LLJajHwARvXC95y73WXEanNS1rbBAZYbC5L");
    assert_eq!(n3.ip().to_string(), "140.82.57.197");
    assert_eq!(n3.port(), 39001);

    let n4 = &nodes[3];
    assert_eq!(n4.id().to_base58(), "4A6UDpARbKBJZmW5s6CmGDgeNmTxWFoGUi2Z5C4z7E41");
    assert_eq!(n4.ip().to_string(), "66.42.74.13");
    assert_eq!(n4.port(), 39001);
}
