use boson::{Id, signature::KeyPair, PeerBuilder};
use boson::activeproxy::{Client, Options};

#[cfg(test)]
mod tests {
    use super::*;

    fn options(service_peerid: &Id, service: &str) -> Options {
        let user_key = KeyPair::random();
        let device_key = KeyPair::random();
        let user_id = Id::from(user_key.public_key());
        Options::read(format!(
            "service:\n  peerId: {service_peerid}\n{service}\
             client:\n  userId: {user_id}\n  userPrivateKey: \"{}\"\n  devicePrivateKey: \"{}\"\n\
             upstream:\n  host: 127.0.0.1\n  port: 8080\n  scheme: http://\n",
            user_key.private_key(),
            device_key.private_key(),
        )).unwrap()
    }

    #[test]
    fn supplied_peer_endpoint_takes_precedence_over_service_host() {
        let service_peerid = Id::random();
        let peer = PeerBuilder::new("peer.example:9090").build().unwrap();
        let options = options(
            &service_peerid,
            "  host: 192.0.2.1\n  port: 9091\n",
        ).with_peer(peer.clone());

        let client = Client::new(None, options).unwrap();

        assert_eq!(client.service_peer(), Some(peer));
        assert_eq!(client.service_endpoint().as_deref(), Some("peer.example:9090"));
    }

    #[test]
    fn service_host_is_used_without_node_or_peer() {
        let service_peerid = Id::random();
        let options = options(
            &service_peerid,
            "  host: 192.0.2.1\n  port: 9091\n",
        );

        let client = Client::new(None, options).unwrap();

        assert!(client.service_peer().is_none());
        assert_eq!(client.service_endpoint().as_deref(), Some("192.0.2.1:9091"));
    }
}