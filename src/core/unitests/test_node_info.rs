use crate::core::{Id, NodeInfo};
use serde_cbor::Value;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_serde() {
        let id = Id::random();
        let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 12345);
        let ni = NodeInfo::new(id.clone(), addr);

        let encoded = serde_cbor::to_vec(&ni).expect("Failed to serialize NodeInfo");
        let decoded: NodeInfo =
            serde_cbor::from_slice(&encoded).expect("Failed to deserialize NodeInfo");

        assert_eq!(decoded, ni);
        assert_eq!(decoded.id(), &id);
    }

    #[test]
    fn test_serde_failed_with_invalid_length() {
        let encoded = serde_cbor::to_vec(&Value::Array(vec![
            serde_cbor::value::to_value(Id::random()).expect("Failed to encode id"),
            Value::Bytes(vec![127, 0, 0]),
            Value::Integer(12345.into()),
        ]))
        .expect("Failed to serialize malformed node info");

        let decoded = serde_cbor::from_slice::<NodeInfo>(&encoded);
        assert!(decoded.is_err());
    }

    #[test]
    fn test_serde_with_byte_string_address() {
        let id = Id::random();
        let encoded = serde_cbor::to_vec(&Value::Array(vec![
            Value::Bytes(id.as_bytes().to_vec()),
            Value::Bytes(vec![45, 32, 138, 246]),
            Value::Integer(39001.into()),
        ]))
        .expect("Failed to serialize byte string node info");

        let decoded = serde_cbor::from_slice::<NodeInfo>(&encoded)
            .expect("Failed to deserialize byte string node info");

        assert_eq!(decoded.id(), &id);
        assert_eq!(decoded.address().to_string(), "45.32.138.246:39001");
    }

    #[test]
    fn test_serde_with_sequence_address() {
        let id = Id::random();
        let encoded = serde_cbor::to_vec(&Value::Array(vec![
            Value::Bytes(id.as_bytes().to_vec()),
            Value::Array(vec![
                Value::Integer(45.into()),
                Value::Integer(32.into()),
                Value::Integer(138.into()),
                Value::Integer(246.into()),
            ]),
            Value::Integer(39001.into()),
        ]))
        .expect("Failed to serialize sequence node info");

        let decoded = serde_cbor::from_slice::<NodeInfo>(&encoded)
            .expect("Failed to deserialize sequence node info");

        assert_eq!(decoded.id(), &id);
        assert_eq!(decoded.address().to_string(), "45.32.138.246:39001");
    }
}
