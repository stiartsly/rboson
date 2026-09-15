use serde::Deserialize;
use std::{fs, path::Path};
use url::Url;

use super::UserRegistration;
use crate::{
    errors::{ArgumentError, Result},
    signature::KeyPair,
    Id,
};

#[derive(Debug, Deserialize)]
struct SerdeOptions {
    director: SerdeDirector,
    #[serde(default)]
    user: Option<SerdeUser>,
    #[serde(default)]
    device: Option<SerdeDevice>,
}

#[derive(Debug, Deserialize)]
struct SerdeDirector {
    url: String,
    #[serde(rename = "nodeId", default)]
    node_id: Option<Id>,
    #[serde(default)]
    insecure: bool,
}

#[derive(Debug, Deserialize)]
struct SerdeUser {
    #[serde(default)]
    id: Option<Id>,
    #[serde(rename = "privateKey", default)]
    private_key: Option<String>,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    email: Option<String>,
    #[serde(default)]
    bio: Option<String>,
    #[serde(default)]
    passphrase: Option<String>,
}

#[derive(Debug, Deserialize)]
struct SerdeDevice {
    #[serde(rename = "privateKey")]
    private_key: String,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    app: Option<String>,
}

#[derive(Clone)]
pub struct DirectorOptions {
    director_url: Url,
    node_id: Option<Id>,
    user_key: Option<KeyPair>,
    user_id: Option<Id>,
    device_key: Option<KeyPair>,
    insecure: bool,
    registration: UserRegistration,
}

impl std::fmt::Debug for DirectorOptions {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DirectorOptions")
            .field("director_url", &self.director_url)
            .field("node_id", &self.node_id)
            .field("user_id", &self.user_id)
            .field("has_user_key", &self.user_key.is_some())
            .field("has_device_key", &self.device_key.is_some())
            .field("insecure", &self.insecure)
            .field("user_name", &self.registration.name())
            .field("user_email", &self.registration.email())
            .field("user_bio", &self.registration.bio())
            .field(
                "has_user_passphrase",
                &self.registration.passphrase().is_some(),
            )
            .field("device_name", &self.registration.device_name())
            .field("app_name", &self.registration.app_name())
            .finish()
    }
}

impl DirectorOptions {
    pub fn builder(url: impl AsRef<str>) -> Result<DirectorOptionsBuilder> {
        DirectorOptionsBuilder::new(url)
    }

    pub fn director_url(&self) -> &Url {
        &self.director_url
    }

    pub fn node_id(&self) -> Option<&Id> {
        self.node_id.as_ref()
    }

    pub fn user_id(&self) -> Option<&Id> {
        self.user_id.as_ref()
    }

    pub fn user_key(&self) -> Option<&KeyPair> {
        self.user_key.as_ref()
    }

    pub fn device_key(&self) -> Option<&KeyPair> {
        self.device_key.as_ref()
    }

    pub fn insecure(&self) -> bool {
        self.insecure
    }

    pub fn registration(&self) -> &UserRegistration {
        &self.registration
    }
}

#[derive(Clone)]
pub struct DirectorOptionsBuilder {
    director_url: Url,
    node_id: Option<Id>,
    user_key: Option<KeyPair>,
    user_id: Option<Id>,
    device_key: Option<KeyPair>,
    insecure: bool,
    user_name: Option<String>,
    user_email: Option<String>,
    user_bio: Option<String>,
    user_passphrase: Option<String>,
    device_name: Option<String>,
    app_name: Option<String>,
}

impl std::fmt::Debug for DirectorOptionsBuilder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DirectorOptionsBuilder")
            .field("director_url", &self.director_url)
            .field("node_id", &self.node_id)
            .field("user_id", &self.user_id)
            .field("has_user_key", &self.user_key.is_some())
            .field("has_device_key", &self.device_key.is_some())
            .field("insecure", &self.insecure)
            .field("user_name", &self.user_name)
            .field("user_email", &self.user_email)
            .field("user_bio", &self.user_bio)
            .field("has_user_passphrase", &self.user_passphrase.is_some())
            .field("device_name", &self.device_name)
            .field("app_name", &self.app_name)
            .finish()
    }
}

impl DirectorOptionsBuilder {
    pub fn new(url: impl AsRef<str>) -> Result<Self> {
        let director_url = parse_director_url(url.as_ref())?;
        Ok(Self {
            director_url,
            node_id: None,
            user_key: None,
            user_id: None,
            device_key: None,
            insecure: false,
            user_name: None,
            user_email: None,
            user_bio: None,
            user_passphrase: None,
            device_name: None,
            app_name: None,
        })
    }

    /// Reads options from YAML with `director`, optional `user`, and optional
    /// `device` sections. A user may provide either `id`, `privateKey`, or both.
    ///
    /// ```yaml
    /// director:
    ///   url: https://director.example
    ///   nodeId: 4W...
    ///   insecure: false
    /// user:
    ///   privateKey: "0x..."
    ///   name: Alice
    ///   email: alice@example.com
    ///   bio: Boson user
    ///   passphrase: secret
    /// device:
    ///   privateKey: "0x..."
    ///   name: Laptop
    ///   app: Boson
    /// ```
    pub fn read_from(yaml: &str) -> Result<Self> {
        let parsed = serde_yaml::from_str::<SerdeOptions>(yaml)
            .map_err(|e| ArgumentError::new(format!("Invalid Director YAML format: {e}")))?;
        let mut builder = Self::new(parsed.director.url)?.with_insecure(parsed.director.insecure);

        if let Some(node_id) = parsed.director.node_id {
            builder = builder.with_node_id(node_id);
        }

        if let Some(user) = parsed.user {
            let user_key = user
                .private_key
                .as_deref()
                .map(str::parse::<KeyPair>)
                .transpose()?;
            match (user.id, user_key) {
                (Some(user_id), Some(user_key)) => {
                    if user_id != Id::from(user_key.public_key()) {
                        return Err(ArgumentError::new("user.id does not match user.privateKey"));
                    }
                    builder = builder.with_user_key(user_key);
                }
                (Some(user_id), None) => builder = builder.with_user_id(user_id),
                (None, Some(user_key)) => builder = builder.with_user_key(user_key),
                (None, None) => {
                    return Err(ArgumentError::new(
                        "Director user requires id or privateKey",
                    ));
                }
            }
            if let Some(name) = user.name {
                builder = builder.with_user_name(name);
            }
            if let Some(email) = user.email {
                builder = builder.with_user_email(email);
            }
            if let Some(bio) = user.bio {
                builder = builder.with_user_bio(bio);
            }
            if let Some(passphrase) = user.passphrase {
                builder = builder.with_user_passphrase(passphrase);
            }
        }

        if let Some(device) = parsed.device {
            builder = builder.with_device_key(device.private_key.parse::<KeyPair>()?);
            match (device.name, device.app) {
                (Some(name), Some(app)) => builder = builder.with_initial_device(name, app),
                (None, None) => {}
                _ => {
                    return Err(ArgumentError::new(
                        "Director device name and app must be provided together",
                    ));
                }
            }
        }

        Ok(builder)
    }

    pub fn load_from(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        let yaml = fs::read_to_string(path).map_err(|e| {
            ArgumentError::new(format!(
                "Reading Director config {} failed: {e}",
                path.display()
            ))
        })?;
        Self::read_from(&yaml)
    }

    pub fn with_node_id(mut self, node_id: Id) -> Self {
        self.node_id = Some(node_id);
        self
    }

    pub fn with_user_key(mut self, key: KeyPair) -> Self {
        self.user_id = Some(Id::from(key.public_key()));
        self.user_key = Some(key);
        self
    }

    pub fn with_user_id(mut self, user_id: Id) -> Self {
        self.user_key = None;
        self.user_id = Some(user_id);
        self
    }

    pub fn with_device_key(mut self, key: KeyPair) -> Self {
        self.device_key = Some(key);
        self
    }

    pub fn with_user_name(mut self, name: impl Into<String>) -> Self {
        self.user_name = Some(name.into());
        self
    }

    pub fn with_user_email(mut self, email: impl Into<String>) -> Self {
        self.user_email = Some(email.into());
        self
    }

    pub fn with_user_bio(mut self, bio: impl Into<String>) -> Self {
        self.user_bio = Some(bio.into());
        self
    }

    pub fn with_user_passphrase(mut self, passphrase: impl Into<String>) -> Self {
        self.user_passphrase = Some(passphrase.into());
        self
    }

    pub fn with_initial_device(mut self, name: impl Into<String>, app: impl Into<String>) -> Self {
        self.device_name = Some(name.into());
        self.app_name = Some(app.into());
        self
    }

    pub fn with_insecure(mut self, insecure: bool) -> Self {
        self.insecure = insecure;
        self
    }

    pub fn build(self) -> Result<DirectorOptions> {
        if self.user_id.is_none() && self.device_key.is_some() {
            return Err(ArgumentError::new(
                "A device key requires a user key or user ID",
            ));
        }
        if self.user_id.is_some() && self.user_key.is_none() && self.device_key.is_none() {
            return Err(ArgumentError::new("A user ID requires a device key"));
        }
        if self.device_name.is_some() != self.app_name.is_some() {
            return Err(ArgumentError::new(
                "Initial device name and app must be provided together",
            ));
        }
        if self.device_name.is_some() && self.device_key.is_none() {
            return Err(ArgumentError::new(
                "An initial device requires a device key",
            ));
        }

        let mut registration = UserRegistration::new();
        if let Some(name) = self.user_name {
            registration = registration.with_name(name);
        }
        if let Some(email) = self.user_email {
            registration = registration.with_email(email);
        }
        if let Some(bio) = self.user_bio {
            registration = registration.with_bio(bio);
        }
        if let Some(passphrase) = self.user_passphrase {
            registration = registration.with_passphrase(passphrase);
        }
        if let (Some(name), Some(app)) = (self.device_name, self.app_name) {
            registration = registration.with_initial_device(name, app);
        }

        Ok(DirectorOptions {
            director_url: self.director_url,
            node_id: self.node_id,
            user_key: self.user_key,
            user_id: self.user_id,
            device_key: self.device_key,
            insecure: self.insecure,
            registration,
        })
    }
}

fn parse_director_url(url: &str) -> Result<Url> {
    let url = Url::parse(url).map_err(|e| ArgumentError::new(e.to_string()))?;
    if !matches!(url.scheme(), "http" | "https") {
        return Err(ArgumentError::new("Director URL must use http or https"));
    }
    Ok(url)
}
