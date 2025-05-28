use crate::{
    Id,
    Identity,
    Error,
    error::Result,
};

use crate::core::{
    crypto_identity::CryptoIdentity,
};

#[allow(unused)]
pub struct DeviceProfile {
    identity: Option<CryptoIdentity>,
    name: String,
    app: Option<String>
}

#[allow(unused)]
impl DeviceProfile {
    pub(crate) fn new(identity: Option<CryptoIdentity>, name: String, app: Option<String>) -> Self {
        Self {
            identity: identity.map(|v| v.clone()),
            name: name.to_string(),
            app: app.map(|v| v.to_string())
        }
    }

    pub fn id(&self) -> Option<&Id> {
        self.identity.as_ref().map(|v| v.id())
    }

    pub fn identity(&self) -> Option<&CryptoIdentity> {
        self.identity.as_ref()
    }

    pub fn has_identity(&self) -> bool {
        self.identity.is_some()
    }

    pub fn set_identity(&mut self, identity: &CryptoIdentity) -> Result<()> {
        if self.has_identity() {
            return Err(Error::State("Identity already set.".into()));
        }

        self.identity = Some(identity.clone());
        Ok(())
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn app(&self) -> Option<&str> {
        self.app.as_ref().map(|v| v.as_str())
    }
}
