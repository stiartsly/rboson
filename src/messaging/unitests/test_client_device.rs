use serde_cbor;

use crate::Id;
use crate::messaging::{
    client_device::ClientDevice
};

#[test]
fn test_serde_and_deserde() {
    let id = Id::random();
    let device = ClientDevice::new(
        &id,
        "Alice".to_string(),
        "Example".to_string(),
        1234567890,
        1234567890,
        "localhost".to_string(),
    );

    let serialized = serde_cbor::to_vec(&device).unwrap();
    let deserialized: ClientDevice = serde_cbor::from_slice(&serialized).unwrap();
    //println!("Deserialized: {:?}", deserialized);
    //println!("{}", deserialized.id());
    //println!("created: {:?}", deserialized.created());

    assert_eq!(device.id(), deserialized.id());
    assert_eq!(device.name(), deserialized.name());
    assert_eq!(device.app(), deserialized.app());
    assert_eq!(device.created(), deserialized.created());
    assert_eq!(device.last_seen(), deserialized.last_seen());
    assert_eq!(device.last_address(), deserialized.last_address());
}
