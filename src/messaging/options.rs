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

#[derive(Clone, Debug, Default)]
pub struct OptionsBuilder {
    peerid: Option<Id>,
    endpoint: Option<url::Url>,
    director_node_id: Option<Id>,
    director_endpoint: Option<url::Url>,
    user_key: Option<KeyPair>,
    user_id: Option<Id>,
    device_key: Option<KeyPair>,
    device_id: Option<Id>,
    data_dir: Option<PathBuf>,
    database_uri: Option<String>,
    database_pool_size: Option<usize>,
    database_schema: Option<String>,
}

impl OptionsBuilder {
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

    fn check_completeness(&self) -> Result<()> {
        if self.user_key.is_none() {
            return Err(ArgumentError::new("user_key is required"));
        }

        if self.device_key.is_none() {
            return Err(ArgumentError::new("device_key is required"));
        }

        if let Some(uri) = &self.database_uri {
            if uri.trim().is_empty() {
                return Err(ArgumentError::new("database URI must not be empty"));
            }
        }
        if let Some(dir) = &self.data_dir {
            if dir.as_os_str().is_empty() {
                return Err(ArgumentError::new("data_dir must not be empty"));
            }
        }
        if let Some(endpoint) = &self.endpoint {
            Self::validate_endpoint(endpoint)?;
        }
        Ok(())
    }

    pub fn with_service_peerid(&mut self, peer_id: Id) -> &mut Self {
        self.peerid = Some(peer_id);
        self
    }

    pub fn with_service_endpoint(&mut self, endpoint: impl AsRef<str>) -> Result<&mut Self> {
        let url = url::Url::parse(endpoint.as_ref())
            .map_err(|e| ArgumentError::new(format!("Invalid endpoint URL: {e}")))?;
        Self::validate_endpoint(&url)?;
        self.endpoint = Some(url);
        Ok(self)
    }

    pub fn with_service_endpoint_str(&mut self, endpoint: &str) -> Result<&mut Self> {
        let url = url::Url::parse(endpoint)
            .map_err(|e| ArgumentError::new(format!("Invalid endpoint URL: {e}")))?;
        self.with_service_endpoint_url(url)
    }

    pub fn with_service_endpoint_url(&mut self, url: url::Url) -> Result<&mut Self> {
        Self::validate_endpoint(&url)?;
        self.endpoint = Some(url);
        Ok(self)
    }

    pub fn with_director_node_id(&mut self, node_id: Id) -> &mut Self {
        self.director_node_id = Some(node_id);
        self
    }

    pub fn with_director_endpoint(&mut self, endpoint: impl AsRef<str>) -> Result<&mut Self> {
        let url = url::Url::parse(endpoint.as_ref())
            .map_err(|e| ArgumentError::new(format!("Invalid director endpoint URL: {e}")))?;
        self.director_endpoint = Some(url);
        Ok(self)
    }

    pub fn with_director_endpoint_url(&mut self, url: url::Url) -> &mut Self {
        self.director_endpoint = Some(url);
        self
    }

    pub fn with_user_id(&mut self, user_id: Id) -> &mut Self {
        self.user_id = Some(user_id);
        self
    }

    pub fn with_user_keypair(&mut self, user_key: KeyPair) -> &mut Self {
        self.user_id = Some(Id::from(user_key.public_key()));
        self.user_key = Some(user_key);
        self
    }

    pub fn with_user_private_key(&mut self, private_key: PrivateKey) -> &mut Self {
        self.with_user_keypair(KeyPair::from(private_key))
    }

    pub fn with_user_key_str(&mut self, key_str: &str) -> Result<&mut Self> {
        let sk = PrivateKey::try_from(key_str).map_err(|e| ArgumentError::new(e.to_string()))?;
        Ok(self.with_user_private_key(sk))
    }

    pub fn with_generated_user_key(&mut self) -> &mut Self {
        self.with_user_keypair(KeyPair::random())
    }

    pub fn with_device_id(&mut self, device_id: Id) -> &mut Self {
        self.device_id = Some(device_id);
        self
    }

    pub fn with_device_keypair(&mut self, device_key: KeyPair) -> &mut Self {
        self.device_id = Some(Id::from(device_key.public_key()));
        self.device_key = Some(device_key);
        self
    }

    pub fn with_device_private_key(&mut self, private_key: PrivateKey) -> &mut Self {
        self.with_device_keypair(KeyPair::from(private_key))
    }

    pub fn with_device_key_str(&mut self, key_str: &str) -> Result<&mut Self> {
        let sk = PrivateKey::try_from(key_str).map_err(|e| ArgumentError::new(e.to_string()))?;
        Ok(self.with_device_private_key(sk))
    }

    pub fn with_generated_device_key(&mut self) -> &mut Self {
        self.with_device_keypair(KeyPair::random())
    }

    pub fn with_data_dir(&mut self, data_dir: impl AsRef<Path>) -> &mut Self {
        self.data_dir = Some(data_dir.as_ref().to_path_buf());
        self
    }

    pub fn with_database(&mut self, uri: impl Into<String>, pool_size: usize) -> Result<&mut Self> {
        self.with_database_uri(uri)?;
        Ok(self.with_database_pool_size(pool_size))
    }

    pub fn with_database_uri(&mut self, uri: impl Into<String>) -> Result<&mut Self> {
        let uri = uri.into();
        if uri.trim().is_empty() {
            return Err(ArgumentError::new("Database URI is empty"));
        }
        self.database_uri = Some(uri);
        Ok(self)
    }

    pub fn with_database_pool_size(&mut self, pool_size: usize) -> &mut Self {
        self.database_pool_size = Some(pool_size);
        self
    }

    pub fn with_database_schema_name(&mut self, schema: impl Into<String>) -> &mut Self {
        self.database_schema = Some(schema.into());
        self
    }

    /*
    pub fn with_database_path(&mut self, path: impl AsRef<Path>) -> &mut Self {
        self.database_path = Some(path.as_ref().to_path_buf());
        self
    }
    */

    pub fn build(&self) -> Result<Options> {
        self.check_completeness()?;

        let peerid = self.peerid.unwrap_or_default();
        let user_key = self.user_key.clone().unwrap();
        let user_id = self
            .user_id
            .unwrap_or_else(|| Id::from(user_key.public_key()));
        let device_key = self.device_key.clone().unwrap();
        let device_id = self
            .device_id
            .unwrap_or_else(|| Id::from(device_key.public_key()));
        let data_dir = self.data_dir.clone().unwrap_or_else(get_current_path);
        let database_uri = self
            .database_uri
            .clone()
            .unwrap_or_else(|| "jdbc:sqlite:messaging.db".to_string());
        let database_pool_size = self.database_pool_size.unwrap_or(1);
        let database_schema = self
            .database_schema
            .clone()
            .unwrap_or_else(|| "public".to_string());
        let database_path = sqlite_path(&database_uri)
            .map(PathBuf::from)
            .map(|path| {
                if path.is_absolute() {
                    path
                } else {
                    data_dir.join(path)
                }
            })
            .unwrap_or_else(|| data_dir.join("messaging.db"));

        Ok(Options {
            peerid,
            endpoint: self.endpoint.clone(),
            director_node_id: self.director_node_id,
            director_endpoint: self.director_endpoint.clone(),
            user_id,
            user_key,
            device_id,
            device_key,
            data_dir,
            database_uri,
            database_pool_size,
            database_schema,
            database_path,
        })
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(try_from = "SerdeOptions")]
pub struct Options {
    peerid: Id,
    endpoint: Option<url::Url>,

    director_node_id: Option<Id>,
    director_endpoint: Option<url::Url>,

    user_id: Id,
    user_key: KeyPair,
    device_id: Id,
    device_key: KeyPair,
    data_dir: PathBuf,

    database_uri: String,
    database_pool_size: usize,
    database_schema: String,

    database_path: PathBuf,
}

impl Options {
    pub fn builder() -> OptionsBuilder {
        OptionsBuilder::default()
    }

    pub fn parse(yaml: impl AsRef<str>) -> Result<Self> {
        let expanded = expand_environ_vars(yaml.as_ref())?;
        serde_yaml::from_str::<SerdeOptions>(&expanded)
            .map_err(|e| {
                ArgumentError::new(format!("Invalid content in messaging client config: {e}"))
            })?
            .try_into()
    }

    pub fn load(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        let yaml = fs::read_to_string(path)
            .map_err(|e| IOError::new(format!("Error reading config {}: {e}", path.display())))?;
        Self::parse(yaml)
    }

    pub fn service_peerid(&self) -> &Id {
        &self.peerid
    }

    pub fn service_endpoint(&self) -> Option<&url::Url> {
        self.endpoint.as_ref()
    }

    pub fn director_node_id(&self) -> Option<&Id> {
        self.director_node_id.as_ref()
    }

    pub fn director_endpoint(&self) -> Option<&url::Url> {
        self.director_endpoint.as_ref()
    }

    pub fn user_key(&self) -> &KeyPair {
        &self.user_key
    }

    pub fn user_private_key(&self) -> &PrivateKey {
        self.user_key.private_key()
    }

    pub fn user_id(&self) -> &Id {
        &self.user_id
    }

    pub fn device_key(&self) -> &KeyPair {
        &self.device_key
    }

    pub fn device_private_key(&self) -> &PrivateKey {
        self.device_key.private_key()
    }

    pub fn device_id(&self) -> &Id {
        &self.device_id
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

    pub fn database_schema(&self) -> &str {
        &self.database_schema
    }

    pub fn database_path(&self) -> &Path {
        self.database_path.as_path()
    }
}


#[derive(Debug, Deserialize, Default)]
struct SerdeDirector {
    #[serde(rename = "nodeId", default)]
    node_id: Option<Id>,
    #[serde(default)]
    endpoint: Option<String>,
    #[serde(default)]
    url: Option<String>,
}

#[derive(Debug, Deserialize)]
struct SerdeOptions {
    service: SerdeService,
    client: SerdeClient,
    #[serde(default)]
    director: Option<SerdeDirector>,
    #[serde(rename = "dataDir", default)]
    data_dir: Option<String>,
    #[serde(default)]
    database: Option<SerdeDatabase>,
}

#[derive(Debug, Deserialize)]
struct SerdeService {
    #[serde(rename = "peerId")]
    peer_id: Id,
    endpoint: Option<String>,
}

#[derive(Debug, Deserialize)]
struct SerdeClient {
    #[serde(rename = "userId", default)]
    user_id: Option<Id>,
    #[serde(rename = "userPrivateKey")]
    user_private_key: Option<String>,
    #[serde(rename = "deviceId", default)]
    device_id: Option<Id>,
    #[serde(rename = "devicePrivateKey", default)]
    device_private_key: Option<String>,
}

#[derive(Debug, Deserialize)]
struct SerdeDatabase {
    #[serde(default)]
    uri: String,
    #[serde(rename = "poolSize", default)]
    pool_size: usize,
    #[serde(default)]
    schema: String,
}

impl TryFrom<SerdeOptions> for Options {
    type Error = Error;

    fn try_from(sopts: SerdeOptions) -> Result<Self> {
        let mut b = Options::builder();
        b.with_service_peerid(sopts.service.peer_id);

        if let Some(endpoint) = sopts.service.endpoint {
            b.with_service_endpoint(endpoint)?;
        }

        let mut director_node_id = None;
        let mut director_endpoint = None;

        if let Some(d) = sopts.director {
            if let Some(node_id) = d.node_id {
                director_node_id = Some(node_id);
            }
            if let Some(endpoint) = d.endpoint.or(d.url) {
                director_endpoint = Some(endpoint);
            }
        }

        if director_node_id.is_none() {
            if let Ok(id_str) = env::var("BOSON_DIRECTOR_NODEID") {
                if let Ok(id) = Id::try_from(id_str.as_str()) {
                    director_node_id = Some(id);
                }
            }
        }
        if director_endpoint.is_none() {
            if let Ok(url_str) = env::var("BOSON_DIRECTOR_URL") {
                director_endpoint = Some(url_str);
            }
        }

        if let Some(id) = director_node_id {
            b.with_director_node_id(id);
        }
        if let Some(endpoint) = director_endpoint {
            b.with_director_endpoint(endpoint)?;
        }

        let user_key = sopts
            .client
            .user_private_key
            .as_deref()
            .map(PrivateKey::try_from)
            .transpose()
            .map_err(|e| ArgumentError::new(e.to_string()) as Error)?
            .map(KeyPair::from)
            .unwrap_or_else(KeyPair::random);

        if let Some(userid) = sopts.client.user_id {
            let fromid = Id::from(user_key.public_key());
            if userid != fromid {
                return Err(ArgumentError::new("userId does not match userPrivateKey") as Error);
            }
        }
        let explicit_user_id = sopts.client.user_id;
        b.with_user_keypair(user_key);
        if let Some(userid) = explicit_user_id {
            b.with_user_id(userid);
        }

        let device_key = sopts
            .client
            .device_private_key
            .as_deref()
            .map(PrivateKey::try_from)
            .transpose()
            .map_err(|e| ArgumentError::new(e.to_string()) as Error)?
            .map(KeyPair::from)
            .unwrap_or_else(KeyPair::random);

        if let Some(device_id) = sopts.client.device_id {
            let fromid = Id::from(device_key.public_key());
            if device_id != fromid {
                return Err(
                    ArgumentError::new("deviceId does not match devicePrivateKey") as Error,
                );
            }
        }
        let explicit_device_id = sopts.client.device_id;
        b.with_device_keypair(device_key);
        if let Some(device_id) = explicit_device_id {
            b.with_device_id(device_id);
        }

        let path = if let Some(data_dir) = sopts.data_dir {
            expand_home_dir(&data_dir)
        } else {
            get_current_path()
        };
        b.with_data_dir(path);

        if let Some(database) = sopts.database {
            b.with_database_uri(&database.uri)?
                .with_database_pool_size(database.pool_size)
                .with_database_schema_name(&database.schema);
        } else {
            let database = SerdeDatabase {
                uri: "jdbc:sqlite:messaging.db".to_string(),
                pool_size: 1,
                schema: "public".to_string(),
            };
            b.with_database_uri(&database.uri)?
                .with_database_pool_size(database.pool_size)
                .with_database_schema_name(&database.schema);
        }

        b.build()
    }
}

fn get_current_path() -> PathBuf {
    env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
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
