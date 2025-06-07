use crate::Id;
use crate::messaging::{
    client_device::ClientDevice
};

#[test]
fn test_serde_and_deserde() {
    let deviceid = Id::random();
    let device = ClientDevice::new(
        &deviceid,
        Some("Alice"),
        Some("Example"),
        1234567890,
        1234567890,
        "localhost",
    );

    let serialized = serde_json::to_string(&device).unwrap();
    let deserialized: ClientDevice = serde_json::from_str(&serialized).unwrap();
    assert!(deserialized.client_id().is_empty());

    assert_eq!(device.id().clone(), deviceid);
    assert_eq!(device.client_id().is_empty(), false);
    assert_eq!(device.id(), deserialized.id());
    assert_eq!(device.name(), deserialized.name());
    assert_eq!(device.app_name(), deserialized.app_name());
    assert_eq!(device.created(), deserialized.created());
    assert_eq!(device.last_seen(), deserialized.last_seen());
    assert_eq!(device.last_address(), deserialized.last_address());
}
