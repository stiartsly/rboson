use crate::{
    Id,
    Error,
    error::Result,
    core::CryptoIdentity,
};

#[derive(Debug, Clone)]
pub struct DeviceProfile {
    identity: Option<CryptoIdentity>,
    name    : String,
    app     : String,
}

impl DeviceProfile {
    pub(crate) fn new(identity: Option<CryptoIdentity>, name: &str, app: &str) -> Self {
        Self {
            identity,
            name    : name.into(),
            app     : app.into(),
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

    pub fn set_identity(&mut self, identity: CryptoIdentity) -> Result<()> {
        if self.has_identity() {
            return Err(Error::State("Identity already set.".into()));
        }

        self.identity = Some(identity);
        Ok(())
    }

    pub fn name(&self) -> &str {
        self.name.as_str()
    }

    pub fn app_name(&self) -> &str {
        self.app.as_str()
    }
}
