use crate::{
    Id,
    core::{Error, Result},
    core::CryptoIdentity,
};

#[derive(Debug, Clone)]
pub struct DeviceProfile {
    identity: Option<CryptoIdentity>,
    name    : String,
    app     : Option<String>
}

impl DeviceProfile {
    pub(crate) fn new(identity: CryptoIdentity, name: String, app: Option<String>) -> Self {
        Self {
            identity: Some(identity),
            name,
            app
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
        self.app.as_deref()
    }
}
