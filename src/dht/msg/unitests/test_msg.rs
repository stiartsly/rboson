use std::net::SocketAddr;
use crate::{
    Id,
    dht::msg::{msg, Message, msg::{Method, Kind}}
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_transaction_ids_positive() {
        for _ in 0..512 {
            assert!(msg::ping_request().txid() > 0);
        }
    }

    #[test]
    fn test_serde_find_value_request() {
        let target = Id::random();
        let mut msg = msg::find_value_request(target.clone(), true, false, 7);
        assert_eq!(msg.kind() as u8, Kind::Request as u8);
        assert_eq!(msg.method() as u8, Method::FindValue as u8);

        assert!(msg.is_req());
        assert!(msg.associated_call().is_none());
        assert!(msg.body().is_some());

        let nodeid = Id::random();
        msg.set_nodeid(nodeid);
        assert_eq!(msg.nodeid(), &nodeid);

        let remote_id = Id::random();
        let remote_addr = SocketAddr::from(([192, 168, 1, 100], 40001));
        msg.set_remote(remote_id, remote_addr);

        assert_eq!(msg.remote_id(), &remote_id);
        assert_eq!(msg.remote_addr(), &remote_addr);

        let encoded = serde_cbor::to_vec(&msg)
            .expect("message serialization failed");
        println!(">>>> encoded: {}", hex::encode(&encoded));
        let decoded: Message = serde_cbor::from_slice(&encoded)
            .expect("message cbor decoding failed");

        assert_eq!(msg.kind() as u8, decoded.kind() as u8);
        assert_eq!(msg.method() as u8, decoded.method() as u8);

        assert!(msg.is_req());
        assert!(decoded.is_req());
        assert!(decoded.associated_call().is_none());

        //assert!(decoded.nodeid().is_none());
        //assert!(decoded.remote_id().is_none());
        //assert!(decoded.remote_addr().is_none());
    }

    #[test]
    fn test_serde_find_peer_request() {
        let target = Id::random();
        let message = msg::find_peer_request(target, true, false, -1, 1);

        let json = serde_json::to_value(&message).expect("JSON serialization failed");
        assert_eq!(json["q"]["t"], target.to_base58());

        println!("JSON: {}", message);

        let cbor = serde_cbor::to_vec(&message).expect("CBOR serialization failed");
        let value: serde_cbor::Value = serde_cbor::from_slice(&cbor)
            .expect("CBOR decoding failed");
        let message_entries = match value {
            serde_cbor::Value::Map(entries) => entries,
            _ => panic!("expected a CBOR message map"),
        };
        let body = message_entries.iter()
            .find(|(key, _)| *key == &serde_cbor::Value::Text("q".to_string()))
            .map(|(_, value)| value)
            .expect("missing request body");
        let body_entries = match body {
            serde_cbor::Value::Map(entries) => entries,
            _ => panic!("expected a CBOR request map"),
        };
        let encoded_target = body_entries.iter()
            .find(|(key, _)| *key == &serde_cbor::Value::Text("t".to_string()))
            .map(|(_, value)| value)
            .expect("missing target field");
        assert!(matches!(encoded_target, serde_cbor::Value::Bytes(bytes) if bytes.len() == Id::BYTES));
    }

    #[test]
    fn test_deserialize_find_peer_response_byte_string_fields() {
        let encoded = hex::decode(
            "bf617918446174192e426172bf617081bf62696458202601fdbd7a5797ef45cad3046f4e40e549039a93b6c56d5951b657bd8eb69f3c6373696758401011a3e2ddc824f0aeae011e68266a4ef8da83824f64cf4ddc7b3d3aa7eb6ab4c7d43ea906fefcd820be951b48cf3e4584a70f89b7bfff6be9e770586e93d80361656f7777772e6578616d706c652e636f6dffff61761a4f520001ff",
        )
        .expect("fixture must be valid hex");

        let message: Message =
            serde_cbor::from_slice(&encoded).expect("find-peer response must deserialize");

        assert_eq!(message.kind() as u8, Kind::Response as u8);
        assert_eq!(message.method() as u8, Method::FindPeer as u8);
        assert_eq!(message.txid(), 11842);
        let Some(msg::Body::FindPeerResponse(response)) = message.body() else {
            panic!("expected a find-peer response body");
        };
        let peers = response.peers().expect("response must contain peers");
        assert_eq!(peers.len(), 1);
        assert_eq!(peers[0].endpoint(), "www.example.com");
        assert_eq!(peers[0].signature().len(), 64);
    }
}
