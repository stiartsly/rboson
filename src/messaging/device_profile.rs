use crate::{
    Id,
    Identity,
};

use crate::core::{
    crypto_identity::CryptoIdentity,
};

#[allow(unused)]
pub struct DeviceProfile {
    identity: Option<CryptoIdentity>,
    name: String,
    app: String
}

#[allow(unused)]
impl DeviceProfile {
    pub(crate) fn new(name: &str, app: &str) -> Self {
        Self {
            identity: None,
            name: name.to_string(),
            app: app.to_string(),
        }
    }

    pub fn id(&self) -> Option<&Id> {
        self.identity.as_ref().map(|v| v.id())
    }

    pub fn identity(&self) -> Option<&CryptoIdentity> {
        self.identity.as_ref()
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn app(&self) -> &str {
        &self.app
    }
}
