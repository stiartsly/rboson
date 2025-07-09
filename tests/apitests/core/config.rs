use std::net::IpAddr;
use log::LevelFilter;
use boson::{
    config::Config,
    configuration,
    signature
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

    cfg = configuration::Builder::new()
        .with_port(port)
        .with_ipv4(ipv4_str)
        .with_data_dir("tests")
        .build()
        .map_err(|_| assert!(false))
        .unwrap();

    assert_eq!(cfg.addr6().is_none(), true);
    assert_eq!(cfg.addr4().is_some(), true);
    assert_eq!(cfg.addr4().unwrap().is_ipv4(), true);
    assert_eq!(cfg.addr4().unwrap().port(), port);
    assert_eq!(cfg.addr4().unwrap().ip(), IpAddr::V4(ipv4_str.parse().unwrap()));
    assert_eq!(cfg.port(), port);
    assert_eq!(cfg.bootstrap_nodes().len(), 0);
    assert_eq!(cfg.data_dir(), "tests");

    #[cfg(feature = "inspect")]
    cfg.dump();
}

#[test]
fn test_load_cfg() {
    let path = match std::fs::metadata("apitests1.conf") {
        Ok(_) => "apitests1.conf",
        Err(_) => "tests/apitests/core/apitests1.conf",
    };
    let cfg = configuration::Builder::new()
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
    assert_eq!(cfg.port(), 39003);
    assert_eq!(cfg.bootstrap_nodes().len(), 1);
    assert_eq!(cfg.data_dir(), "apitests1_data");
    assert_eq!(cfg.log_level(), LevelFilter::Info);
    assert_eq!(cfg.log_file(), None);
    assert_eq!(cfg.activeproxy().is_some(), false);

    let nodes = cfg.bootstrap_nodes();
    let n1 = &nodes[0];
    assert_eq!(n1.id().to_base58(), "HZXXs9LTfNQjrDKvvexRhuMk8TTJhYCfrHwaj3jUzuhZ");
    assert_eq!(n1.ip().to_string(), "155.138.245.211");
    assert_eq!(n1.port(), 39001);

    assert_eq!(cfg.log_level(), LevelFilter::Info);
    assert_eq!(cfg.log_file(), None);
    assert_eq!(cfg.activeproxy().is_none(), true);
    assert_eq!(cfg.user().is_none(), true);
}

#[test]
fn test_load_cfg_full(){
    let path = match std::fs::metadata("apitests2.conf") {
        Ok(_) => "apitests2.conf",
        Err(_) => "tests/apitests/core/apitests2.conf",
    };
    let cfg = configuration::Builder::new()
        .load(path)
        .map_err(|e| {println!("{e}"); assert!(false)})
        .unwrap()
        .build()
        .map_err(|_| assert!(false))
        .unwrap();

    #[cfg(feature = "inspect")]
    cfg.dump();

    assert_eq!(cfg.addr4().is_some(), true);
    assert_eq!(cfg.addr6().is_some(), false);
    assert_eq!(cfg.port(), 39004);
    assert_eq!(cfg.bootstrap_nodes().len(), 1);
    assert_eq!(cfg.data_dir(), "apitests2_data");
    assert_eq!(cfg.activeproxy().is_some(), true);

    let nodes = cfg.bootstrap_nodes();
    let n1 = &nodes[0];
    assert_eq!(n1.id().to_base58(), "HZXXs9LTfNQjrDKvvexRhuMk8TTJhYCfrHwaj3jUzuhZ");
    assert_eq!(n1.ip().to_string(), "155.138.245.211");
    assert_eq!(n1.port(), 39001);

    assert_eq!(cfg.log_level(), LevelFilter::Debug);
    assert_eq!(cfg.log_file(), Some("apitests2.log".to_string()));

    let result = cfg.activeproxy();
    assert!(result.is_some());
    let ap = result.unwrap();
    assert_eq!(ap.server_peerid(), "FemkhMoaGnt8HUYANxX9zKgd5Ghy7tWxDkxqd1fe6kJT");
    assert_eq!(ap.peer_private_key().is_some(), true);
    assert_eq!(ap.domain_name().is_some(), false);
    assert_eq!(ap.upstream_host(), "127.0.0.1");
    assert_eq!(ap.upstream_port(), 8080);

    let result = cfg.user();
    assert!(result.is_some());
    let user = result.unwrap();
    assert_eq!(user.private_key(), "0xa3218958b88d86dead1a58b439a22c161e0573022738b570210b123dc0b046faec6f3cd4ed1e6801ebf33fd60c07cf9924ef01d829f3f5af7377f054bff31501");
    let result = signature::PrivateKey::try_from(user.private_key());
    assert!(result.is_ok());
    let sk = result.unwrap();
    assert_eq!(sk.as_bytes().len(), signature::PrivateKey::BYTES);
}
