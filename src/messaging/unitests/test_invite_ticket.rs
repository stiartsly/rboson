use std::time::{SystemTime, Duration};

use crate::{
    Id,
    signature::KeyPair,
    cryptobox::PrivateKey,
    messaging::InviteTicket
};

#[test]
fn test_invite_public_ticket() {
    let channel_id = Id::random();
    let inviter_keypair = KeyPair::random();
    let inviter = Id::from(inviter_keypair.public_key());
    let invitee = Id::random();
    let expire  = SystemTime::now() + Duration::from_secs(InviteTicket::EXPIRATION);
    let is_public = true;

    let expire_ts = expire.duration_since(SystemTime::UNIX_EPOCH).unwrap().as_secs();
    let digest = InviteTicket::digest(
        &channel_id,
        &inviter,
        is_public,
        expire_ts,
        &invitee
    );
    let sig = inviter_keypair.private_key().sign_into(&digest).unwrap();

    let ticket = InviteTicket::new(
        channel_id.clone(),
        inviter.clone(),
        is_public,
        expire_ts,
        sig.clone(),
        None
    );

    assert_eq!(ticket.channel_id(), &channel_id);
    assert_eq!(ticket.inviter(), &inviter);
    assert_eq!(ticket.is_public(), is_public);
    assert_eq!(ticket.is_expired(), false);
    assert_eq!(ticket.session_key(), None);
    assert_eq!(ticket.is_valid(&invitee), true);

    let proof = ticket.proof();
    assert_eq!(proof.channel_id(), &channel_id);
    assert_eq!(proof.inviter(), &inviter);
    assert_eq!(proof.is_public(), is_public);
    assert_eq!(proof.is_expired(), false);
    assert_eq!(proof.session_key(), None);
    assert_eq!(proof.is_valid(&invitee), true);
}

#[test]
fn test_invite_private_ticket() {
    let channel_id = Id::random();
    let inviter_keypair = KeyPair::random();
    let inviter = Id::from(inviter_keypair.public_key());
    let invitee = Id::random();
    let expire  = SystemTime::now() + Duration::from_secs(InviteTicket::EXPIRATION);
    let is_public = false;
    let session_sk = PrivateKey::try_from(inviter_keypair.private_key()).unwrap();

    let expire_ts = expire.duration_since(SystemTime::UNIX_EPOCH).unwrap().as_secs();
    let digest = InviteTicket::digest(
        &channel_id,
        &inviter,
        is_public,
        expire_ts,
        &invitee
    );
    let sig = inviter_keypair.private_key().sign_into(&digest).unwrap();

    let ticket = InviteTicket::new(
        channel_id.clone(),
        inviter.clone(),
        is_public,
        expire_ts,
        sig.clone(),
        Some(session_sk.as_bytes().to_vec())
    );

    assert_eq!(ticket.channel_id(), &channel_id);
    assert_eq!(ticket.inviter(), &inviter);
    assert_eq!(ticket.is_public(), is_public);
    assert_eq!(ticket.is_expired(), false);
    assert_eq!(ticket.session_key(), Some(session_sk.as_bytes()));
    assert_eq!(ticket.is_valid(&invitee), true);

    let proof = ticket.proof();
    assert_eq!(proof.channel_id(), &channel_id);
    assert_eq!(proof.inviter(), &inviter);
    assert_eq!(proof.is_public(), is_public);
    assert_eq!(proof.is_expired(), false);
    assert_eq!(proof.session_key(), None);
    assert_eq!(proof.is_valid(&invitee), true);
}
