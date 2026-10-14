use std::sync::Arc;
use super::{ContextKind, SessionContext};
use crate::{cryptobox::Nonce, errors::ArgumentError, CryptoIdentity, Id, Identity, Signature};

fn context(for_channel: bool, contact_id: Id) -> SessionContext {
    let user = Arc::new(CryptoIdentity::new());
    let session = Arc::new(CryptoIdentity::new());
    if for_channel {
        SessionContext::for_channel(contact_id, user, session)
    } else {
        SessionContext::for_user(contact_id, user, session)
    }
}

#[test]
fn test_session_context_signing() {
    for channel in [false, true] {
        let user = Arc::new(CryptoIdentity::new());
        let session = Arc::new(CryptoIdentity::new());
        let contact = CryptoIdentity::new();
        let context = if channel {
            SessionContext::for_channel(
                *contact.id(),
                user.clone(),
                session.clone(),
            )
        } else {
            SessionContext::for_user(
                *contact.id(),
                user.clone(),
                session.clone(),
            )
        };
        assert_eq!(context.session_id(), session.id());

        let rc = context.sign(b"message");
        assert!(rc.is_ok());
        let signed = rc.unwrap();

        assert_eq!(signed.len(), Signature::BYTES);
        assert!(session.verify(b"message", &signed).unwrap());
        assert!(context.verify(b"message", &signed).unwrap());
        assert!(!user.verify(b"message", &signed).unwrap());
        assert!(!context.verify(b"tampered", &signed).unwrap());
        assert!(!context
            .verify(b"message", &user.sign_into(b"message").unwrap())
            .unwrap());
        assert!(context.verify(b"message", &[0; 1]).is_err());
    }
}

#[test]
fn test_session_context_encryption1() {
    for channel in [false, true] {
        let peer = CryptoIdentity::new();
        let context = context(channel, *peer.id());
        for plain in [b"message".as_slice(), b"".as_slice()] {
            let rc = context.encrypt(peer.id(), plain);
            assert!(rc.is_ok());
            let encrypted = rc.unwrap();

            let rc = peer.decrypt_into(context.session_id(), &encrypted);
            assert!(rc.is_ok());
            let decrypted = rc.unwrap();
            assert_eq!(decrypted, plain);

            let rc = peer.encrypt_into(context.session_id(), plain);
            assert!(rc.is_ok());
            let encrypted = rc.unwrap();

            let rc = context.decrypt(peer.id(), &encrypted);
            assert!(rc.is_ok());
            let decrypted = rc.unwrap();
            assert_eq!(decrypted, plain);
        }

        assert!(context.rx_crypto_context.is_none());
        assert!(context.tx_crypto_context.is_none());
    }
}

#[test]
fn test_session_context_encryption_for_user() {
    let peer = CryptoIdentity::new();
    let context = context(false, *peer.id());
    let mut session_ctxt = context.create_crypto_context(peer.id()).unwrap();
    assert_eq!(session_ctxt.id(), peer.id());

    let mut peer_ctxt = peer.create_crypto_context(context.session_id()).unwrap();
    let rc = session_ctxt.encrypt_into(b"outgoing");
    assert!(rc.is_ok());
    let encrypted = rc.unwrap();

    let rc = peer_ctxt.decrypt_into(&encrypted);
    assert!(rc.is_ok());
    assert_eq!(rc.unwrap(), b"outgoing");

    let rc = peer_ctxt.encrypt_into(b"incoming");
    assert!(rc.is_ok());
    let encrypted = rc.unwrap();

    let rc = session_ctxt.decrypt_into(&encrypted);
    assert!(rc.is_ok());
    assert_eq!(rc.unwrap(), b"incoming");

    assert!(context.rx_crypto_context.is_none());
    assert!(context.tx_crypto_context.is_none());
}

#[test]
fn transmit_context_is_lazy_cached_and_preserves_nonce_state() {
    for channel in [false, true] {
        let peer = CryptoIdentity::new();
        let mut context = context(channel, *peer.id());
        assert!(context.tx_crypto_context.is_none());

        let receiver = context
            .session_identity
            .create_crypto_context(context.user_identity.id())
            .unwrap();
        let session_id = *context.session_id();

        let tx = context.tx_crypto_context().unwrap();
        assert_eq!(tx.id(), &session_id);
        let address = tx as *const _;
        let first = tx.encrypt_into(b"first").unwrap();
        let tx = context.tx_crypto_context().unwrap();
        assert_eq!(address, tx as *const _);
        let second = tx.encrypt_into(b"second").unwrap();
        let mut expected_nonce = Nonce::try_from(&first[..Nonce::BYTES]).unwrap();
        expected_nonce.increment();
        assert_eq!(expected_nonce.as_bytes(), &second[..Nonce::BYTES]);
        assert_eq!(receiver.decrypt_into(&first).unwrap(), b"first");
        assert_eq!(receiver.decrypt_into(&second).unwrap(), b"second");
        assert!(context.rx_crypto_context.is_none());
    }
}

#[test]
fn receive_context_is_lazy_and_shared_with_contact_lookup() {
    for channel in [false, true] {
        let contact = CryptoIdentity::new();
        let mut context = context(channel, *contact.id());
        assert!(context.rx_crypto_context.is_none());
        let mut sender = contact.create_crypto_context(context.session_id()).unwrap();
        let encrypted = sender.encrypt_into(b"contact message").unwrap();
        let rx = context.rx_crypto_context().unwrap();
        assert_eq!(rx.id(), contact.id());
        assert_eq!(rx.decrypt_into(&encrypted).unwrap(), b"contact message");
        let address = rx as *const _;
        assert_eq!(
            address,
            context.rx_crypto_context_for(contact.id()).unwrap() as *const _
        );
        if let ContextKind::Channel(members) = &context.kind {
            assert!(members.is_empty());
        }
        assert!(context.tx_crypto_context.is_none());
    }
}

#[test]
fn user_context_rejects_other_senders_before_and_after_caching() {
    let contact = CryptoIdentity::new();
    let other = CryptoIdentity::new();
    let mut context = context(false, *contact.id());
    let error = context.rx_crypto_context_for(other.id()).unwrap_err();
    assert!(error.is::<ArgumentError>());
    assert!(context.rx_crypto_context.is_none());
    context.rx_crypto_context_for(contact.id()).unwrap();
    let error = context.rx_crypto_context_for(other.id()).unwrap_err();
    assert!(error.is::<ArgumentError>());
    assert_eq!(context.rx_crypto_context().unwrap().id(), contact.id());
}

#[test]
fn channel_context_caches_independent_member_contexts() {
    let contact = CryptoIdentity::new();
    let members = [CryptoIdentity::new(), CryptoIdentity::new()];
    let mut context = context(true, *contact.id());
    let session_id = *context.session_id();
    for member in &members {
        let mut sender = member.create_crypto_context(&session_id).unwrap();
        let encrypted = sender.encrypt_into(b"member message").unwrap();
        let rx = context.rx_crypto_context_for(member.id()).unwrap();
        assert_eq!(rx.id(), member.id());
        assert_eq!(rx.decrypt_into(&encrypted).unwrap(), b"member message");
    }
    assert!(context.rx_crypto_context.is_none());
    context.rx_crypto_context_for(contact.id()).unwrap();
    let cached = match &context.kind {
        ContextKind::Channel(cached) => cached,
        ContextKind::User => panic!("Expected a channel context"),
    };
    assert_eq!(cached.len(), members.len());

    let first = context.rx_crypto_context_for(members[0].id()).unwrap() as *const _;
    assert_eq!(
        first,
        context.rx_crypto_context_for(members[0].id()).unwrap() as *const _
    );
    assert_ne!(
        first,
        context.rx_crypto_context_for(members[1].id()).unwrap() as *const _
    );
}
