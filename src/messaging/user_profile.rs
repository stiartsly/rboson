use crate::{
    Id,
    Identity,
};

use crate::core::{
    crypto_identity::CryptoIdentity,
};

pub struct UserProfile {
    identity: CryptoIdentity,
    name: String,
    avatar: bool
}

impl UserProfile {
    pub(crate) fn new(identity: &CryptoIdentity, name: &str, avatar: bool) -> Self {
        Self {
            identity: identity.clone(),
            name    : name.to_string(),
            avatar
        }
    }

    pub fn id(&self) -> &Id {
        self.identity.id()
    }

    pub fn identity(&self) -> &CryptoIdentity {
        &self.identity
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn has_avatar(&self) -> bool {
        self.avatar
    }
}
