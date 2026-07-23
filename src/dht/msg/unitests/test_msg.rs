use std::net::SocketAddr;
use crate::{
    Id,
    dht::msg::{msg, Message, msg::{Method, Kind}}
};

#[cfg(test)]
mod tests {
    use super::*;

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
}
