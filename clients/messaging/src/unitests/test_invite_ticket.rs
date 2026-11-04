
use super::InviteTicket;
use boson_api::{CryptoIdentity, Id};
use std::time::SystemTime;

#[test]
fn test_serde() {
    let inviter = CryptoIdentity::new();
    let channel_id = Id::random();
    let session_id = Id::random();
    let invitee = Id::random();
    let expiration = boson_api::as_ms!(SystemTime::now()) + 60000;
    let ticket = InviteTicket::create(
        &inviter,
        channel_id,
        session_id,
        Some(invitee),
        expiration as u64,
        vec![7; 64],
    )
    .unwrap();

    assert!(ticket.is_named_ticket());
    assert!(ticket.is_genuine());
    assert!(!ticket.is_expired());

    let encoded = serde_json::to_vec(&ticket).unwrap();
    let decoded = serde_json::from_slice::<InviteTicket>(&encoded).unwrap();
    assert_eq!(decoded.channel_id(), &channel_id);
    assert_eq!(decoded.session_id(), &session_id);
    assert_eq!(decoded.invitee(), Some(&invitee));
    assert!(decoded.is_genuine());

    let json = ticket.to_string();
    assert!(json.contains("\"sig\":\""));
    assert!(json.contains("\"sk\":\""));
}

#[test]
fn test_revise() {
    let inviter = CryptoIdentity::new();
    let ticket = InviteTicket::create(
        &inviter,
        Id::random(),
        Id::random(),
        None,
        u64::MAX,
        vec![1; 64],
    )
    .unwrap();

    let revised = ticket.revise(vec![2; 64]).unwrap();
    assert!(revised.is_bearer_ticket());
    assert!(revised.is_genuine());
    assert_eq!(revised.session_key(), &[2; 64]);
}
