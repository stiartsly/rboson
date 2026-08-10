use crate::{
    dht::Node,
    activeproxy::{
        ActiveProxyClient as ActiveProxy,
    },
    cfg::configuration
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
    let raw = std::fs::read_to_string(path).unwrap();
    let json: serde_json::Value = serde_json::from_str(&raw).unwrap();

    let data_dir = json.get("dataDir").and_then(|v| v.as_str()).unwrap_or("unitests_data");
    let user_private_key = json.get("user").and_then(|v| v.get("privateKey")).and_then(|v| v.as_str()).unwrap();
    let yaml = format!(
        "ipv4: true\nport: {}\nprivateKey: \"{}\"\ndataDir: {}\ndatabaseUri: jdbc:sqlite:storage.db\n",
        json.get("port").and_then(|v| v.as_u64()).unwrap_or(39008),
        user_private_key,
        data_dir,
    );

    let mut b = configuration::Builder::new();
    let _ = b.read_from(&yaml).unwrap();

    let server_peerid = json
        .get("activeproxy")
        .and_then(|v| v.get("serverPeerId"))
        .and_then(|v| v.as_str())
        .unwrap();
    let upstream_host = json
        .get("activeproxy")
        .and_then(|v| v.get("upstreamHost"))
        .and_then(|v| v.as_str())
        .unwrap();
    let upstream_port = json
        .get("activeproxy")
        .and_then(|v| v.get("upstreamPort"))
        .and_then(|v| v.as_u64())
        .unwrap_or(8080) as u16;

    b.with_activeproxy_service_peerid(server_peerid.try_into().unwrap())
        .with_upstream_host(upstream_host)
        .with_upstream_port(upstream_port);

    let cfg = b.build().unwrap();

    let result = Node::new(cfg.build_node_options().unwrap());
    assert_eq!(result.is_ok(), true);

    let node = result.unwrap();

    let options = cfg.build_activeproxy_options().unwrap().unwrap();
    let result = ActiveProxy::new(node.clone(), options);
    assert_eq!(result.is_ok(), true);

    let ap = result.unwrap();
    assert_eq!(ap.nodeid(), node.id().clone());
    assert_eq!(ap.upstream_host(), "127.0.0.1");
    assert_eq!(ap.upstream_port(), 8080);
    assert_eq!(ap.upstream_endpoint(), "127.0.0.1:8080");
    assert_eq!(ap.domain_name(), None);
    assert_eq!(ap.remote_peerid().clone(), "FemkhMoaGnt8HUYANxX9zKgd5Ghy7tWxDkxqd1fe6kJT".try_into().unwrap());

    remove_path(data_dir);
    remove_file("unitests.log");
}

