use crate::{
    Id,
    dht::Node,
    signature,
    activeproxy::{ActiveProxyClient as ActiveProxy, client::ActiveProxyOptions},
    dht::yaml_configuration::NodeConfiguration,
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

    let cfg = Box::new(NodeConfiguration::from(&yaml).unwrap());

    let result = Node::new(cfg);
    assert_eq!(result.is_ok(), true);

    let node = result.unwrap();
    let user_sk = signature::PrivateKey::try_from(user_private_key).unwrap();
    let options = ActiveProxyOptions {
        cached_dir: std::path::PathBuf::from(data_dir).join("activeproxy.cache"),
        server_peerid: Id::try_from(
            json.get("activeproxy").and_then(|v| v.get("serverPeerId")).and_then(|v| v.as_str()).unwrap()
        ).unwrap(),
        user_keypair: signature::KeyPair::from(user_sk.clone()),
        peer_keypair: None,
        upstream_host: json.get("activeproxy").and_then(|v| v.get("upstreamHost")).and_then(|v| v.as_str()).unwrap().to_string(),
        upstream_port: json.get("activeproxy").and_then(|v| v.get("upstreamPort")).and_then(|v| v.as_u64()).unwrap_or(8080) as u16,
        upstream_domain: None,
    };
    let result = ActiveProxy::new(node.clone(), options);
    assert_eq!(result.is_ok(), true);

    let ap = result.unwrap();
    assert_eq!(ap.nodeid(), node.id().clone());
    assert_eq!(ap.upstream_host(), "127.0.0.1");
    assert_eq!(ap.upstream_port(), 8080);
    assert_eq!(ap.upstream_endpoint(), "127.0.0.1:8080");
    assert_eq!(ap.domain_name(), None);
    assert_eq!(ap.remote_peerid().clone(), Id::try_from("FemkhMoaGnt8HUYANxX9zKgd5Ghy7tWxDkxqd1fe6kJT").unwrap());

    remove_path(data_dir);
    remove_file("unitests.log");
}

