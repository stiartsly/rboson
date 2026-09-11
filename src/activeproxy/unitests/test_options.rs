use crate::{
    activeproxy::OptionsBuilder,
    signature::KeyPair,
    Id,
};

#[test]
fn test_activeproxy_configuration_builds_options() {
    let user_key = KeyPair::random();
    let device_key = KeyPair::random();
    let service_peer_id = Id::random();
    let yaml = format!(
        "service:\n  peerId: {service_peer_id}\n  host: 192.0.2.1\n  port: 9090\nclient:\n  userPrivateKey: {}\n  devicePrivateKey: {}\nupstream:\n  host: 127.0.0.1\n  port: 8080\n  scheme: http://\nnameAccess: true\nannouncePeer: true\n",
        user_key.private_key(),
        device_key.private_key(),
    );

    let options = OptionsBuilder::read_from(&yaml)
        .expect("configuration should parse")
        .build()
        .expect("options should build");

    assert_eq!(options.service_peerid(), &service_peer_id);
    assert_eq!(options.service_host(), Some("192.0.2.1"));
    assert_eq!(options.service_port(), 9090);
    assert_eq!(options.user_id(), &Id::from(user_key.public_key()));
    assert_eq!(options.device_key().private_key(), device_key.private_key());
    assert_eq!(options.upstream_host(), "127.0.0.1");
    assert_eq!(options.upstream_port(), 8080);
    assert_eq!(options.upstream_scheme(), "http://");
    assert!(options.is_name_access_enabled());
    assert!(options.is_announce_peer_enabled());
}
