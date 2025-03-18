use std::sync::{Arc, Mutex};
use std::str::FromStr;
use crate::{
    configuration as cfg,
    Id,
    Node,
    ActiveProxyClient as ActiveProxy,
};

#[test]
fn test_activeproxy() {
    let path = match std::fs::metadata("apitests2.conf") {
        Ok(_) => "unitests.conf",
        Err(_) => "src/unitests/unitests.conf",
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
    assert_eq!(ap.remote_peerid().clone(), Id::from_str("FemkhMoaGnt8HUYANxX9zKgd5Ghy7tWxDkxqd1fe6kJT").unwrap());
}
