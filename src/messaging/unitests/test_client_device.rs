use crate::Id;
use crate::messaging::{
    client_device::ClientDevice
};

#[test]
fn test_json_serde() {
    let deviceid = Id::random();
    let dev = ClientDevice::new(
        &deviceid,
        "Alice",
        Some("Example"),
        1234567890,
        1234567890,
        "localhost",
    );

    let serde = serde_json::to_string(&dev).unwrap();
    let rc = serde_json::from_str(&serde);
    assert!(rc.is_ok());

    let dev_new: ClientDevice = rc.unwrap();
    assert!(dev_new.client_id().is_empty());

    assert_eq!(dev.id().clone(), deviceid);
    assert_eq!(dev.client_id().is_empty(), false);
    assert_eq!(dev.id(), dev_new.id());
    assert_eq!(dev.name(), dev_new.name());
    assert_eq!(dev.app_name(), dev_new.app_name());
    assert_eq!(dev.created(), dev_new.created());
    assert_eq!(dev.last_seen(), dev_new.last_seen());
    assert_eq!(dev.last_address(), dev_new.last_address());
}
