use std::time::{SystemTime, Duration};

use crate::{
    Id,
    signature::KeyPair,
    core::CryptoIdentity,
    messaging::InviteTicket,
};

#[test]
fn test_public_invite_ticket() {
    let channel_id = Id::random();
    let keypair = KeyPair::random();
    let inviter = Id::from(keypair.public_key());
    let invitee = Id::random();
    let expire  = SystemTime::now() + Duration::from_millis(InviteTicket::EXPIRATION);
    let is_public = true;

    let expire_ts = expire.duration_since(SystemTime::UNIX_EPOCH).unwrap().as_secs();
    let digest = InviteTicket::digest(
        &channel_id,
        &inviter,
        &invitee,
        is_public,
        expire_ts
    );
    let sig = keypair.private_key().sign_into(&digest).unwrap();

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
fn test_private_invite_ticket() {
    let channel_id = Id::random();
    let inviter_kp = KeyPair::random();
    let inviter_id = Id::from(inviter_kp.public_key());
    let invitee_id = Id::random();
    let expire  = SystemTime::now() + Duration::from_millis(InviteTicket::EXPIRATION);
    let is_public = false;
    let channel_kp = KeyPair::random();
    let session_sk = channel_kp.private_key();

    let expire_ts = expire.duration_since(SystemTime::UNIX_EPOCH).unwrap().as_secs();
    let digest = InviteTicket::digest(
        &channel_id,
        &inviter_id,
        &invitee_id,
        is_public,
        expire_ts
    );
    let sig = inviter_kp.private_key().sign_into(&digest).unwrap();

    let ticket = InviteTicket::new(
        channel_id.clone(),
        inviter_id.clone(),
        is_public,
        expire_ts,
        sig.clone(),
        Some(session_sk.as_bytes().to_vec())
    );

    assert_eq!(ticket.channel_id(), &channel_id);
    assert_eq!(ticket.inviter(), &inviter_id);
    assert_eq!(ticket.is_public(), is_public);
    assert_eq!(ticket.is_expired(), false);
    assert_eq!(ticket.session_key(), Some(session_sk.as_bytes()));
    assert_eq!(ticket.is_valid(&invitee_id), true);

    let proof = ticket.proof();
    assert_eq!(proof.channel_id(), &channel_id);
    assert_eq!(proof.inviter(), &inviter_id);
    assert_eq!(proof.is_public(), is_public);
    assert_eq!(proof.is_expired(), false);
    assert_eq!(proof.session_key(), None);
    assert_eq!(proof.is_valid(&invitee_id), true);
}

#[test]
fn test_private_invite_ticket_using_cryptoidentity() {
    let channel_kp = KeyPair::random();
    let channel_id = Id::from(channel_kp.public_key());
    let inviter = CryptoIdentity::from_keypair(KeyPair::random());
    let invitee_id = Id::random();
    let expire  = SystemTime::now() + Duration::from_millis(InviteTicket::EXPIRATION);
    let is_public = false;
    let session_sk = channel_kp.private_key();

    let expire_ts = expire.duration_since(SystemTime::UNIX_EPOCH).unwrap().as_secs();
    let digest = InviteTicket::digest(
        &channel_id,
        inviter.id(),
        &invitee_id,
        is_public,
        expire_ts
    );
    let sig = inviter.sign_into(&digest).unwrap();

    let ticket = InviteTicket::new(
        channel_id.clone(),
        inviter.id().clone(),
        is_public,
        expire_ts,
        sig.clone(),
        Some(session_sk.as_bytes().to_vec())
    );

    assert_eq!(ticket.channel_id(), &channel_id);
    assert_eq!(ticket.inviter(), inviter.id());
    assert_eq!(ticket.is_public(), is_public);
    assert_eq!(ticket.is_expired(), false);
    assert_eq!(ticket.session_key(), Some(session_sk.as_bytes()));
    assert_eq!(ticket.is_valid(&invitee_id), true);

    let proof = ticket.proof();
    assert_eq!(proof.channel_id(), &channel_id);
    assert_eq!(proof.inviter(), inviter.id());
    assert_eq!(proof.is_public(), is_public);
    assert_eq!(proof.is_expired(), false);
    assert_eq!(proof.session_key(), None);
    assert_eq!(proof.is_valid(&invitee_id), true);
}
