use std::sync::{Arc, Mutex};
use crate::{
    configuration as cfg,
    Id,
    dht::Node,
    ActiveProxyClient as ActiveProxy,
};

fn remove_path(input: &str) {
    if std::fs::metadata(&input).is_ok() {
        match std::fs::remove_dir_all(&input) {
            Ok(_) => {}
            Err(e) => {
                panic!("Failed to remove directory: {}", e);
            }
        }
    }
}

fn remove_file(input: &str) {
    if std::fs::metadata(&input).is_ok() {
        match std::fs::remove_file(&input) {
            Ok(_) => {}
            Err(e) => {
                panic!("Failed to remove file: {}", e);
            }
        }
    }
}

#[test]
fn test_activeproxy() {
    let path = match std::fs::metadata("test_ap.conf") {
        Ok(_) => "test_ap.conf",
        Err(_) => "src/activeproxy/unitests/test_ap.conf",
    };
    let cfg = cfg::Builder::new()
        .load(path)
        .map_err(|e| {println!("{e}"); assert!(false)})
        .unwrap()
        .build()
        .map_err(|_| assert!(false))
        .unwrap();

    let result = Node::new(&cfg);
    assert_eq!(result.is_ok(), true);

    let node = Arc::new(Mutex::new(result.unwrap()));
    let result = ActiveProxy::new(node.clone(), &cfg);
    assert_eq!(result.is_ok(), true);

    let ap = result.unwrap();
    assert_eq!(ap.nodeid(), node.lock().unwrap().id().clone());
    assert_eq!(ap.upstream_host(), "127.0.0.1");
    assert_eq!(ap.upstream_port(), 8080);
    assert_eq!(ap.upstream_endpoint(), "127.0.0.1:8080");
    assert_eq!(ap.domain_name(), None);
    assert_eq!(ap.remote_peerid().clone(), Id::try_from("FemkhMoaGnt8HUYANxX9zKgd5Ghy7tWxDkxqd1fe6kJT").unwrap());

    remove_path(cfg.data_dir());
    remove_file("unitests.log");
}

