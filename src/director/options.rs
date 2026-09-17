use serde::Deserialize;
use std::{env, fs, path::Path};
use url::Url;

use super::UserRegistration;
use crate::{
    errors::{ArgumentError, Error, IOError, Result},
    signature::KeyPair,
    Id,
};

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

impl DirectorOptions {
    pub fn from_url(url: impl AsRef<str>) -> Result<Self> {
        Ok(Self {
            director_url: parse_director_url(url.as_ref())?,
            node_id: None,
            user_key: None,
            user_id: None,
            device_key: None,
            insecure: false,
            registration: UserRegistration::new(),
        })
    }

    pub fn parse(yaml: impl AsRef<str>) -> Result<Self> {
        let expanded_yaml = expand_environment_variables(yaml.as_ref())?;
        serde_yaml::from_str::<SerdeOptions>(&expanded_yaml)
            .map_err(|e|
                ArgumentError::new(format!("Invalid Director YAML format: {e}"))
            )?
            .try_into()
    }

    pub fn read(yaml: impl AsRef<str>) -> Result<Self> {
        Self::parse(yaml)
    }

    pub fn load(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        let yaml = fs::read_to_string(path).map_err(|e| {
            IOError::new(format!("Reading Director config {} failed: {e}", path.display()))
        })?;
        Self::parse(yaml)
    }

    pub fn with_director_url(mut self, url: impl AsRef<str>) -> Result<Self> {
        self.director_url = parse_director_url(url.as_ref())?;
        Ok(self)
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

    /*
    pub fn with_user_id(mut self, user_id: Id) -> Self {
        self.user_key = None;
        self.user_id = Some(user_id);
        self
    }
    */

    pub fn with_device_key(mut self, key: KeyPair) -> Self {
        self.device_key = Some(key);
        self
    }

    pub fn with_user_name(mut self, name: impl Into<String>) -> Self {
        self.registration = self.registration.with_name(name);
        self
    }

    pub fn with_user_email(mut self, email: impl Into<String>) -> Self {
        self.registration = self.registration.with_email(email);
        self
    }

    pub fn with_user_bio(mut self, bio: impl Into<String>) -> Self {
        self.registration = self.registration.with_bio(bio);
        self
    }

    pub fn with_user_passphrase(mut self, passphrase: impl Into<String>) -> Self {
        self.registration = self.registration.with_passphrase(passphrase);
        self
    }

    pub fn with_initial_device(
        mut self,
        name: impl Into<String>,
        app: impl Into<String>,
    ) -> Self {
        self.registration = self.registration.with_initial_device(name, app);
        self
    }

    pub fn with_insecure(mut self, insecure: bool) -> Self {
        self.insecure = insecure;
        self
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

    pub fn check_valid(&self) -> Result<()> {
        if self.user_id.is_none() && self.device_key.is_some() {
            return Err(ArgumentError::new(
                "A device key requires a user key or user ID",
            ));
        }
        if self.user_id.is_some() && self.user_key.is_none() && self.device_key.is_none() {
            return Err(ArgumentError::new("A user ID requires a device key"));
        }
        if self.registration.has_initial_device() && self.device_key.is_none() {
            return Err(ArgumentError::new(
                "An initial device requires a device key",
            ));
        }
        Ok(())
    }
}

impl TryFrom<&str> for DirectorOptions {
    type Error = Error;

    fn try_from(yaml: &str) -> Result<Self> {
        Self::parse(yaml)
    }
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

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SerdeOptions {
    director: SerdeDirector,
    user: SerdeUser,
    device: SerdeDevice,
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
#[serde(deny_unknown_fields)]
struct SerdeUser {
    #[serde(default)]
    id: Id,
    #[serde(rename = "privateKey")]
    private_key: String,
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
#[serde(deny_unknown_fields)]
struct SerdeDevice {
    #[serde(rename = "privateKey")]
    private_key: String,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    app: Option<String>,
}

impl TryFrom<SerdeOptions> for DirectorOptions {
    type Error = Error;

    fn try_from(options: SerdeOptions) -> Result<Self> {
        let mut opts = Self::from_url(options.director.url)?
            .with_insecure(options.director.insecure);

        if let Some(node_id) = options.director.node_id {
            opts = opts.with_node_id(node_id);
        }

        let user_key = match options.user.private_key.parse::<KeyPair>() {
            Ok(key) => key,
            Err(e) => {
                return Err(ArgumentError::new(
                    &format!("Failed to parse user private key: {e}"),
                ));
            }
        };

        let generated_userid = Id::from(user_key.public_key());
        if options.user.id != generated_userid {
            return Err(ArgumentError::new(
                &format!("user.id does not match user.privateKey {}!={}", options.user.id, generated_userid),
            ));
        }

        if let Some(node_id) = options.director.node_id {
            opts = opts.with_node_id(node_id);
        }
        opts = opts.with_user_key(user_key);

        if let Some(name) = options.user.name {
            opts = opts.with_user_name(name);
        }
        if let Some(email) = options.user.email {
            opts = opts.with_user_email(email);
        }
        if let Some(bio) = options.user.bio {
            opts = opts.with_user_bio(bio);
        }
        if let Some(passphrase) = options.user.passphrase {
            opts = opts.with_user_passphrase(passphrase);
        }

        let device_key = match options.device.private_key.parse::<KeyPair>() {
            Ok(key) => key,
            Err(e) => {
                return Err(ArgumentError::new(
                    &format!("Failed to parse device private key: {e}"),
                ));
            }
        };

        opts = opts.with_device_key(device_key);
        match (options.device.name, options.device.app) {
            (Some(name), Some(app)) => {
                opts = opts.with_initial_device(name, app);
            }
            (_, _) => return Err(ArgumentError::new(
                "Director device name and app must be provided together",
            )),
        }

        opts.check_valid()?;
        Ok(opts)
    }
}

fn parse_director_url(url: &str) -> Result<Url> {
    let url = Url::parse(url).map_err(|e|
        ArgumentError::new(e.to_string())
    )?;
    if !matches!(url.scheme(), "http" | "https") {
        return Err(ArgumentError::new("Director URL must use http or https"));
    }
    Ok(url)
}

fn expand_environment_variables(input: &str) -> Result<String> {
    let mut expanded = String::with_capacity(input.len());
    let mut remaining = input;

    while let Some(start) = remaining.find("${") {
        let (prefix, after) = remaining.split_at(start);
        expanded.push_str(prefix);
        let end = after.find('}').ok_or_else(|| {
            ArgumentError::new("unterminated environment variable reference")
        })?;
        let name = &after[2..end];
        if name.is_empty() {
            return Err(ArgumentError::new("empty environment variable name"));
        }
        let value = env::var(name).map_err(|_| {
            ArgumentError::new(format!("environment variable `{name}` is not set"))
        })?;
        expanded.push_str(&value);
        remaining = &after[end + 1..];
    }

    expanded.push_str(remaining);
    Ok(expanded)
}
