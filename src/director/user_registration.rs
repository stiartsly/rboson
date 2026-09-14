#[derive(Clone, Debug, Default)]
pub struct UserRegistration {
    name: Option<String>,
    email: Option<String>,
    bio: Option<String>,
    passphrase: Option<String>,
    device_name: Option<String>,
    app_name: Option<String>,
}

impl UserRegistration {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn with_name(mut self, value: impl Into<String>) -> Self {
        self.name = Some(value.into());
        self
    }
    pub fn with_email(mut self, value: impl Into<String>) -> Self {
        self.email = Some(value.into());
        self
    }
    pub fn with_bio(mut self, value: impl Into<String>) -> Self {
        self.bio = Some(value.into());
        self
    }
    pub fn with_passphrase(mut self, value: impl Into<String>) -> Self {
        self.passphrase = Some(value.into());
        self
    }

    pub fn with_initial_device(mut self, name: impl Into<String>, app: impl Into<String>) -> Self {
        self.device_name = Some(name.into());
        self.app_name = Some(app.into());
        self
    }

    pub fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    pub fn email(&self) -> Option<&str> {
        self.email.as_deref()
    }

    pub fn bio(&self) -> Option<&str> {
        self.bio.as_deref()
    }

    pub fn passphrase(&self) -> Option<&str> {
        self.passphrase.as_deref()
    }

    pub fn device_name(&self) -> Option<&str> {
        self.device_name.as_deref()
    }

    pub fn app_name(&self) -> Option<&str> {
        self.app_name.as_deref()
    }

    pub fn has_initial_device(&self) -> bool {
        self.device_name.is_some()
    }
}
