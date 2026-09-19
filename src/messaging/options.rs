use std::{
    env, fs,
    path::{Path, PathBuf},
};

use serde::Deserialize;

use crate::errors::{ArgumentError, Error, IOError, Result};
use crate::signature::{KeyPair, PrivateKey};
use crate::Id;

pub const SCHEME_MQTT: &str = "mqtt";
pub const SCHEME_MQTTS: &str = "mqtts";
pub const DEFAULT_DATABASE_URI: &str = "jdbc:sqlite:messaging.db";

#[derive(Clone, Debug, Deserialize)]
#[serde(try_from = "SerdeOptions")]
pub struct Options {
    pub peerid: Option<Id>,
    pub endpoint: Option<url::Url>,

    pub user_key: Option<KeyPair>,
    pub user_id: Option<Id>,

    pub device_key: Option<KeyPair>,
    pub device_id: Option<Id>,

    pub data_dir: PathBuf,
    pub database_uri: String,
    pub database_pool_size: usize,
    pub database_schema_name: Option<String>,

    pub database_path: PathBuf,
}

impl Options {
    pub fn default_data_dir() -> PathBuf {
        let base = env::var_os("XDG_DATA_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                let mut home = env::var_os("HOME")
                    .map(PathBuf::from)
                    .unwrap_or_else(|| PathBuf::from("."));
                home.push(".local/share");
                home
            });
        base.join("boson/client/photon-messaging")
    }

    pub fn new() -> Self {
        Self {
            peerid: None,
            endpoint: None,
            user_key: None,
            user_id: None,
            device_key: None,
            device_id: None,
            data_dir: PathBuf::from("."),
            database_uri: DEFAULT_DATABASE_URI.to_string(),
            database_pool_size: 0,
            database_schema_name: None,
            database_path: PathBuf::from("messaging.db"),
        }
    }

    pub fn parse(yaml: impl AsRef<str>) -> Result<Self> {
        let expanded_yaml = expand_environ_vars(yaml.as_ref())?;
        serde_yaml::from_str::<SerdeOptions>(&expanded_yaml)
            .map_err(|e| ArgumentError::new(format!("invalid messaging YAML format: {e}")))?
            .try_into()
    }

    pub fn read(yaml: impl AsRef<str>) -> Result<Self> {
        Self::parse(yaml)
    }

    pub fn load(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        let yaml = fs::read_to_string(path)
            .map_err(|e| IOError::new(format!("Reading config {} failed: {e}", path.display())))?;
        Self::parse(yaml)
    }

    pub fn validate_endpoint(url: &url::Url) -> Result<()> {
        let scheme = url.scheme();
        if scheme != SCHEME_MQTT && scheme != SCHEME_MQTTS {
            return Err(ArgumentError::new(format!(
                "Invalid endpoint scheme '{scheme}': expected 'mqtt' or 'mqtts'"
            )));
        }
        if url.host_str().is_none() {
            return Err(ArgumentError::new("Endpoint is missing a hostname"));
        }
        if url.port().is_none() {
            return Err(ArgumentError::new("Endpoint must specify a port (1-65535)"));
        }
        Ok(())
    }

    pub fn check_completeness(&self) -> Result<()> {
        if let (Some(user_id), Some(user_key)) = (&self.user_id, &self.user_key) {
            if user_id != &Id::from(user_key.public_key()) {
                return Err(ArgumentError::new("userId does not match userKey"));
            }
        }
        if let (Some(device_id), Some(device_key)) = (&self.device_id, &self.device_key) {
            if device_id != &Id::from(device_key.public_key()) {
                return Err(ArgumentError::new("deviceId does not match deviceKey"));
            }
        }
        if self.user_id.is_none() && self.user_key.is_none() {
            return Err(ArgumentError::new(
                "user identity (user_id or user_key) is required",
            ));
        }
        if self.device_key.is_none() {
            return Err(ArgumentError::new("device_key is required"));
        }
        if self.database_uri.trim().is_empty() {
            return Err(ArgumentError::new("database URI must not be empty"));
        }
        if self.data_dir.as_os_str().is_empty() {
            return Err(ArgumentError::new("data_dir must not be empty"));
        }
        if let Some(endpoint) = &self.endpoint {
            Self::validate_endpoint(endpoint)?;
        }
        Ok(())
    }

    pub fn with_service_peerid(mut self, peer_id: Id) -> Self {
        self.peerid = Some(peer_id);
        self
    }

    pub fn with_service_endpoint(mut self, endpoint: impl AsRef<str>) -> Result<Self> {
        let url = url::Url::parse(endpoint.as_ref())
            .map_err(|e| ArgumentError::new(format!("Invalid endpoint URL: {e}")))?;
        Self::validate_endpoint(&url)?;
        self.endpoint = Some(url);
        Ok(self)
    }

    pub fn with_service_endpoint_url(mut self, url: url::Url) -> Result<Self> {
        Self::validate_endpoint(&url)?;
        self.endpoint = Some(url);
        Ok(self)
    }

    pub fn with_user_id(mut self, user_id: Id) -> Self {
        self.user_id = Some(user_id);
        self
    }

    pub fn with_userid(self, user_id: Id) -> Self {
        self.with_user_id(user_id)
    }

    pub fn with_user_keypair(mut self, user_key: KeyPair) -> Self {
        self.user_id = Some(Id::from(user_key.public_key()));
        self.user_key = Some(user_key);
        self
    }

    pub fn with_user_private_key(self, private_key: PrivateKey) -> Self {
        self.with_user_keypair(KeyPair::from(private_key))
    }

    pub fn with_user_key_str(self, key_str: &str) -> Result<Self> {
        let sk = PrivateKey::try_from(key_str).map_err(|e| ArgumentError::new(e.to_string()))?;
        Ok(self.with_user_private_key(sk))
    }

    pub fn with_generated_user_key(self) -> Self {
        self.with_user_keypair(KeyPair::random())
    }

    pub fn with_device_id(mut self, device_id: Id) -> Self {
        self.device_id = Some(device_id);
        self
    }

    pub fn with_device_keypair(mut self, device_key: KeyPair) -> Self {
        self.device_id = Some(Id::from(device_key.public_key()));
        self.device_key = Some(device_key);
        self
    }

    pub fn with_device_private_key(self, private_key: PrivateKey) -> Self {
        self.with_device_keypair(KeyPair::from(private_key))
    }

    pub fn with_device_key_str(self, key_str: &str) -> Result<Self> {
        let sk = PrivateKey::try_from(key_str).map_err(|e| ArgumentError::new(e.to_string()))?;
        Ok(self.with_device_private_key(sk))
    }

    pub fn with_generated_device_key(self) -> Self {
        self.with_device_keypair(KeyPair::random())
    }

    pub fn with_data_dir(mut self, data_dir: impl AsRef<Path>) -> Self {
        self.data_dir = data_dir.as_ref().to_path_buf();
        self.database_path = sqlite_path(&self.database_uri)
            .map(PathBuf::from)
            .map(|path| {
                if path.is_absolute() {
                    path
                } else {
                    self.data_dir.join(path)
                }
            })
            .unwrap_or_else(|| self.data_dir.join("messaging.db"));
        self
    }

    pub fn with_database(mut self, uri: impl Into<String>, pool_size: usize) -> Result<Self> {
        self = self.with_database_uri(uri)?;
        Ok(self.with_database_pool_size(pool_size))
    }

    pub fn with_database_uri(mut self, uri: impl Into<String>) -> Result<Self> {
        let uri = uri.into();
        if uri.trim().is_empty() {
            return Err(ArgumentError::new("Database URI is empty"));
        }
        self.database_path = sqlite_path(&uri)
            .map(PathBuf::from)
            .map(|path| {
                if path.is_absolute() {
                    path
                } else {
                    self.data_dir.join(path)
                }
            })
            .unwrap_or_else(|| self.data_dir.join("messaging.db"));
        self.database_uri = uri;
        Ok(self)
    }

    pub fn with_database_pool_size(mut self, pool_size: usize) -> Self {
        self.database_pool_size = pool_size;
        self
    }

    pub fn with_database_schema_name(mut self, schema: impl Into<String>) -> Self {
        self.database_schema_name = Some(schema.into());
        self
    }

    pub fn with_database_path(mut self, path: impl AsRef<Path>) -> Self {
        self.database_path = path.as_ref().to_path_buf();
        self
    }

    pub fn service_peerid(&self) -> Option<&Id> {
        self.peerid.as_ref()
    }

    pub fn service_endpoint(&self) -> Option<&url::Url> {
        self.endpoint.as_ref()
    }

    pub fn user_key(&self) -> Option<&KeyPair> {
        self.user_key.as_ref()
    }

    pub fn user_private_key(&self) -> Option<&PrivateKey> {
        self.user_key.as_ref().map(|key| key.private_key())
    }

    pub fn user_id(&self) -> Option<&Id> {
        self.user_id.as_ref()
    }

    pub fn device_key(&self) -> Option<&KeyPair> {
        self.device_key.as_ref()
    }

    pub fn device_private_key(&self) -> Option<&PrivateKey> {
        self.device_key.as_ref().map(|key| key.private_key())
    }

    pub fn device_id(&self) -> Option<&Id> {
        self.device_id.as_ref()
    }

    pub fn data_dir(&self) -> &Path {
        self.data_dir.as_path()
    }

    pub fn database_uri(&self) -> &str {
        &self.database_uri
    }

    pub fn database_pool_size(&self) -> usize {
        self.database_pool_size
    }

    pub fn database_schema_name(&self) -> Option<&str> {
        self.database_schema_name.as_deref()
    }

    pub fn database_path(&self) -> &Path {
        self.database_path.as_path()
    }
}

impl Default for Options {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Deserialize)]
struct SerdeOptions {
    service: SerdeService,
    #[serde(default)]
    client: Option<SerdeClient>,
    #[serde(rename = "dataDir", default)]
    data_dir: Option<String>,
    #[serde(default)]
    database: Option<SerdeDatabase>,
}

#[derive(Debug, Deserialize)]
struct SerdeService {
    #[serde(rename = "peerId")]
    peer_id: Id,
    #[serde(default)]
    endpoint: Option<String>,
}

#[derive(Debug, Deserialize)]
struct SerdeClient {
    #[serde(rename = "userId", default)]
    user_id: Id,
    #[serde(rename = "userPrivateKey", default)]
    user_private_key: Option<String>,
    #[serde(rename = "deviceId", default)]
    device_id: Id,
    #[serde(rename = "devicePrivateKey", default)]
    device_private_key: Option<String>,
}

#[derive(Debug, Deserialize)]
struct SerdeDatabase {
    #[serde(default)]
    uri: Option<String>,
    #[serde(rename = "poolSize", default)]
    pool_size: Option<usize>,
    #[serde(default)]
    schema: Option<String>,
}

impl TryFrom<SerdeOptions> for Options {
    type Error = Error;

    fn try_from(sopts: SerdeOptions) -> Result<Self> {
        let mut opts = Options::new();
        opts = opts.with_service_peerid(sopts.service.peer_id);

        if let Some(endpoint) = sopts.service.endpoint {
            opts = opts.with_service_endpoint(endpoint)?;
        }

        if let Some(client) = sopts.client {
            let user_key = client
                .user_private_key
                .as_deref()
                .map(PrivateKey::try_from)
                .transpose()
                .map_err(|e| ArgumentError::new(e.to_string()) as Error)?
                .map(KeyPair::from);

            if let Some(key) = user_key {
                if client.user_id != Id::default() && client.user_id != Id::from(key.public_key()) {
                    return Err(ArgumentError::new("userId does not match userPrivateKey") as Error);
                }
                opts = opts.with_user_keypair(key);
            } else if client.user_id != Id::default() {
                opts = opts.with_user_id(client.user_id);
            }

            let device_key = client
                .device_private_key
                .as_deref()
                .map(PrivateKey::try_from)
                .transpose()
                .map_err(|e| ArgumentError::new(e.to_string()) as Error)?
                .map(KeyPair::from);

            if let Some(key) = device_key {
                if client.device_id != Id::default()
                    && client.device_id != Id::from(key.public_key())
                {
                    return Err(
                        ArgumentError::new("deviceId does not match devicePrivateKey") as Error,
                    );
                }
                opts = opts.with_device_keypair(key);
            } else if client.device_id != Id::default() {
                opts = opts.with_device_id(client.device_id);
            }
        }

        if let Some(data_dir) = sopts.data_dir {
            let path = expand_home_dir(&data_dir);
            opts = opts.with_data_dir(path);
        }

        if let Some(db) = sopts.database {
            if let Some(uri) = db.uri {
                opts = opts.with_database_uri(uri)?;
            }
            if let Some(pool_size) = db.pool_size {
                opts = opts.with_database_pool_size(pool_size);
            }
            if let Some(schema) = db.schema {
                opts = opts.with_database_schema_name(schema);
            }
        }

        opts.check_completeness()?;
        Ok(opts)
    }
}

fn expand_environ_vars(input: &str) -> Result<String> {
    let mut expanded = String::with_capacity(input.len());
    let mut remaining = input;

    while let Some(start) = remaining.find("${") {
        let (prefix, after) = remaining.split_at(start);
        expanded.push_str(prefix);

        let end = after
            .find('}')
            .ok_or_else(|| ArgumentError::new("unterminated environment variable reference"))?;
        let name = &after[2..end];
        if name.is_empty() {
            return Err(ArgumentError::new("empty environment variable name"));
        }
        let value = env::var(name)
            .map_err(|_| ArgumentError::new(format!("environment variable `{name}` is not set")))?;
        expanded.push_str(&value);
        remaining = &after[end + 1..];
    }

    expanded.push_str(remaining);
    Ok(expanded)
}

fn expand_home_dir(path: &str) -> PathBuf {
    if let Some(rest) = path.strip_prefix("~/") {
        let home = env::var_os("HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("."));
        home.join(rest)
    } else if path == "~" {
        env::var_os("HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("."))
    } else {
        PathBuf::from(path)
    }
}

fn sqlite_path(uri: &str) -> Option<&str> {
    uri.strip_prefix("jdbc:sqlite:")
        .or_else(|| uri.strip_prefix("sqlite:"))
}
