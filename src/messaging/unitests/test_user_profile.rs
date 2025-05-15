use crate::{
    core::crypto_identity::CryptoIdentity,
    signature,
    Identity,
};

use crate::messaging::{
    user_profile::UserProfile,
};

#[test]
fn test_user_profile() {
    let identity = CryptoIdentity::from_keypair(signature::KeyPair::random());
    let profile = UserProfile::new(&identity, "Alice", true);

    assert_eq!(profile.id(), identity.id());
    // assert_eq!(profile.identity(), &identity);
    assert_eq!(profile.name(), "Alice");
    assert_eq!(profile.has_avatar(), true);
}
