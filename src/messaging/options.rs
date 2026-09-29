use std::{
    env, fs,
    sync::Arc,
    path::{Path, PathBuf},
};
use serde::Deserialize;

use crate::{
    Id,
    PeerInfo,
    signature::{KeyPair, PrivateKey},
    errors::{ArgumentError, Error, IOError, Result},
};
use super::{
    ConnectionListener,
    MessageListener,
    ChannelListener,
    ContactListener,
    SessionListener,
    FriendRequestListener,
};

pub const SCHEME_MQTT: &str = "mqtt";
pub const SCHEME_MQTTS: &str = "mqtts";

struct NoopConnectionListener;
impl ConnectionListener for NoopConnectionListener {
    fn on_ready(&self) {}
}

struct NoopMessageListener;
impl MessageListener for NoopMessageListener {
    fn on_message(&self, _message: &dyn super::Message) {}
}

struct NoopChannelListener;
impl ChannelListener for NoopChannelListener {}

struct NoopContactListener;
impl ContactListener for NoopContactListener {}

struct NoopSessionListener;
impl SessionListener for NoopSessionListener {
    fn on_new_session(&self, _session_info: &super::SessionInfo) {}
}

struct NoopFriendRequestListener;
impl FriendRequestListener for NoopFriendRequestListener {}

#[derive(Default, Clone, Deserialize)]
#[serde(try_from = "SerdeOptions")]
pub struct OptionsBuilder {
    peer_id: Option<Id>,
    peer_endpoint: Option<url::Url>,

    user_key: Option<KeyPair>,
    device_key: Option<KeyPair>,

    data_dir: Option<PathBuf>,

    database_uri: Option<String>,
    database_pool_size: Option<usize>,
    database_schema: Option<String>,

    log_level: Option<log::LevelFilter>,
    log_file: Option<String>,
    log_console: bool,

    connection_listener: Option<Arc<dyn ConnectionListener>>,
    message_listener: Option<Arc<dyn MessageListener>>,
    channel_listener: Option<Arc<dyn ChannelListener>>,
    contact_listener: Option<Arc<dyn ContactListener>>,
    session_listener: Option<Arc<dyn SessionListener>>,
    friend_request_listener: Option<Arc<dyn FriendRequestListener>>,
}


impl OptionsBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_peer_id(&mut self, peer_id: Id) -> &mut Self {
        self.peer_id = Some(peer_id);
        self
    }

    pub fn with_service_peerid(&mut self, peer_id: Id) -> &mut Self {
        self.with_peer_id(peer_id)
    }

    pub fn with_peer_endpoint(&mut self, endpoint: impl AsRef<str>) -> Result<&mut Self> {
        let url = url::Url::parse(endpoint.as_ref())
            .map_err(|e| ArgumentError::new(format!("Invalid endpoint URL: {e}")))?;
        self.with_peer_endpoint_url(url)
    }

    pub fn with_peer_endpoint_url(&mut self, url: url::Url) -> Result<&mut Self> {
        Self::validate_endpoint(&url)?;
        self.peer_endpoint = Some(url);
        Ok(self)
    }

    pub fn with_service_endpoint_url(&mut self, url: url::Url) -> Result<&mut Self> {
        self.with_peer_endpoint_url(url)
    }

    pub fn with_service_endpoint(&mut self, endpoint: impl AsRef<str>) -> Result<&mut Self> {
        self.with_peer_endpoint(endpoint)
    }

    pub fn with_peer(&mut self, peer: &PeerInfo) -> Result<&mut Self> {
        self.with_peer_id(peer.id().clone());
        self.with_peer_endpoint(peer.endpoint())
    }

    pub fn with_user_keypair(&mut self, user_key: KeyPair) -> &mut Self {
        self.user_key = Some(user_key);
        self
    }

    pub fn with_user_private_key(&mut self, private_key: PrivateKey) -> &mut Self {
        self.with_user_keypair(KeyPair::from(private_key))
    }

    pub fn with_device_keypair(&mut self, device_key: KeyPair) -> &mut Self {
        self.device_key = Some(device_key);
        self
    }

    pub fn with_device_private_key(&mut self, private_key: PrivateKey) -> &mut Self {
        self.with_device_keypair(KeyPair::from(private_key))
    }

    pub fn with_data_dir(&mut self, data_dir: impl AsRef<Path>) -> &mut Self {
        self.data_dir = Some(data_dir.as_ref().to_path_buf());
        self
    }

    pub fn with_database(&mut self, uri: impl Into<String>, pool_size: usize) -> Result<&mut Self> {
        let uri = uri.into();
        if uri.trim().is_empty() {
            return Err(ArgumentError::new("Database URI is empty"));
        }
        self.database_uri = Some(uri);
        self.database_pool_size = Some(pool_size);
        Ok(self)
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

    pub fn with_database_schema(&mut self, schema: impl Into<String>) -> &mut Self {
        self.database_schema = Some(schema.into());
        self
    }

    pub fn with_database_schema_name(&mut self, schema: impl Into<String>) -> &mut Self {
        self.with_database_schema(schema)
    }

    pub fn with_log_level(&mut self, level: log::LevelFilter) -> &mut Self {
        self.log_level = Some(level);
        self
    }

    pub fn with_log_file(&mut self, path: impl Into<String>) -> &mut Self {
        self.log_file = Some(path.into());
        self
    }

    pub fn with_log_console(&mut self, enabled: bool) -> &mut Self {
        self.log_console = enabled;
        self
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

    pub fn with_connection_listener(&mut self, listener: impl ConnectionListener + 'static) -> &mut Self {
        self.connection_listener = Some(Arc::new(listener));
        self
    }

    pub fn with_message_listener(&mut self, listener: impl MessageListener + 'static) -> &mut Self {
        self.message_listener = Some(Arc::new(listener));
        self
    }

    pub fn with_channel_listener(&mut self, listener: impl ChannelListener + 'static) -> &mut Self {
        self.channel_listener = Some(Arc::new(listener));
        self
    }

    pub fn with_contact_listener(&mut self, listener: impl ContactListener + 'static) -> &mut Self {
        self.contact_listener = Some(Arc::new(listener));
        self
    }

    pub fn with_session_listener(&mut self, listener: impl SessionListener + 'static) -> &mut Self {
        self.session_listener = Some(Arc::new(listener));
        self
    }

    pub fn with_friend_request_listener(&mut self, listener: impl FriendRequestListener + 'static) -> &mut Self {
        self.friend_request_listener = Some(Arc::new(listener));
        self
    }

    pub fn build(&self) -> Result<Options> {
        self.check_completeness()?;

        let user_key = self.user_key.as_ref().unwrap();
        let device_key = self.device_key.as_ref().unwrap();

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
            peer_id: self.peer_id.clone().unwrap(),
            peer_endpoint: self.peer_endpoint.clone().unwrap(),
            user_id: Id::from(user_key.public_key()),
            user_key: user_key.clone(),
            device_id: Id::from(device_key.public_key()),
            device_key: device_key.clone(),
            data_dir,
            database_uri,
            database_pool_size,
            database_schema,
            database_path,
            log_level: self.log_level.unwrap_or(log::LevelFilter::Info),
            log_file: self.log_file.clone(),
            log_console: self.log_console,

            connection_listener: self.connection_listener.clone()
                .unwrap_or_else(|| Arc::new(NoopConnectionListener)),
            message_listener: self.message_listener.clone()
                .unwrap_or_else(|| Arc::new(NoopMessageListener)),
            channel_listener: self.channel_listener.clone()
                .unwrap_or_else(|| Arc::new(NoopChannelListener)),
            contact_listener: self.contact_listener.clone()
                .unwrap_or_else(|| Arc::new(NoopContactListener)),
            session_listener: self.session_listener.clone()
                .unwrap_or_else(|| Arc::new(NoopSessionListener)),
            friend_request_listener: self.friend_request_listener.clone()
                .unwrap_or_else(|| Arc::new(NoopFriendRequestListener)),
        })
    }

    fn validate_endpoint(url: &url::Url) -> Result<()> {
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
        if self.peer_id.is_none() {
            return Err(ArgumentError::new("service peer id is required"));
        }
        if self.peer_endpoint.is_none() {
            return Err(ArgumentError::new("service peer endpoint is required"));
        }

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
        Ok(())
    }
}

impl Drop for OptionsBuilder {
    fn drop(&mut self) {
        self.user_key = None;
        self.device_key = None;
    }
}



pub struct Options {
    peer_id: Id,
    peer_endpoint: url::Url,

    user_id: Id,
    user_key: KeyPair,
    device_id: Id,
    device_key: KeyPair,

    data_dir: PathBuf,

    database_uri: String,
    database_pool_size: usize,
    database_schema: String,

    database_path: PathBuf,

    log_level: log::LevelFilter,
    log_file: Option<String>,
    log_console: bool,

    connection_listener: Arc<dyn ConnectionListener>,
    message_listener: Arc<dyn MessageListener>,
    channel_listener: Arc<dyn ChannelListener>,
    contact_listener: Arc<dyn ContactListener>,
    session_listener: Arc<dyn SessionListener>,
    friend_request_listener: Arc<dyn FriendRequestListener>,
}


impl Options {
    pub fn builder() -> OptionsBuilder {
        OptionsBuilder::new()
    }

    pub fn parse(yaml: impl AsRef<str>) -> Result<Self> {
        OptionsBuilder::parse(yaml)?.build()
    }

    pub fn load(path: impl AsRef<Path>) -> Result<Self> {
        OptionsBuilder::load(path)?.build()
    }

    pub fn peer_id(&self) -> &Id {
        &self.peer_id
    }

    pub fn service_peerid(&self) -> &Id {
        self.peer_id()
    }

    pub fn peer_endpoint(&self) -> &url::Url {
        &self.peer_endpoint
    }

    pub fn service_endpoint(&self) -> Option<&url::Url> {
        Some(self.peer_endpoint())
    }

    pub fn user_id(&self) -> &Id {
        &self.user_id
    }

    pub fn user_key(&self) -> &KeyPair {
        &self.user_key
    }

    pub fn user_private_key(&self) -> &PrivateKey {
        self.user_key.private_key()
    }

    pub fn device_id(&self) -> &Id {
        &self.device_id
    }

    pub fn device_key(&self) -> &KeyPair {
        &self.device_key
    }

    pub fn device_private_key(&self) -> &PrivateKey {
        self.device_key.private_key()
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

    pub fn log_level(&self) -> log::LevelFilter {
        self.log_level
    }

    pub fn log_file(&self) -> Option<&str> {
        self.log_file.as_deref()
    }

    pub fn log_console(&self) -> bool {
        self.log_console
    }

    pub(crate) fn connection_listener(&self) -> Arc<dyn ConnectionListener> {
        self.connection_listener.clone()
    }

    pub(crate) fn message_listener(&self) -> Arc<dyn MessageListener> {
        self.message_listener.clone()
    }

    pub(crate) fn channel_listener(&self) -> Arc<dyn ChannelListener> {
        self.channel_listener.clone()
    }

    pub(crate) fn contact_listener(&self) -> Arc<dyn ContactListener> {
        self.contact_listener.clone()
    }

    pub(crate) fn session_listener(&self) -> Arc<dyn SessionListener> {
        self.session_listener.clone()
    }

    pub(crate) fn friend_request_listener(&self) -> Arc<dyn FriendRequestListener> {
        self.friend_request_listener.clone()
    }
}

#[derive(Debug, Deserialize)]
struct SerdeOptions {
    service: SerdeService,
    client: SerdeClient,
    #[serde(rename = "dataDir", default)]
    data_dir: Option<String>,
    #[serde(default)]
    database: Option<SerdeDatabase>,
    #[serde(rename = "logLevel", default)]
    log_level: Option<String>,
    #[serde(rename = "logFile", default)]
    log_file: Option<String>,
    #[serde(rename = "logConsole", default)]
    log_console: Option<bool>,
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
    user_private_key: String,
    #[serde(rename = "deviceId", default)]
    device_id: Option<Id>,
    #[serde(rename = "devicePrivateKey")]
    device_private_key: String,
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

impl TryFrom<SerdeOptions> for OptionsBuilder {
    type Error = Error;

    fn try_from(sopts: SerdeOptions) -> Result<Self> {
        let mut b = OptionsBuilder::default();
        b.with_peer_id(sopts.service.peer_id);

        if let Some(endpoint) = sopts.service.endpoint {
            b.with_peer_endpoint(endpoint)?;
        }

        let user_key: KeyPair = sopts.client
            .user_private_key
            .parse::<PrivateKey>()
            .map_err(|e| ArgumentError::new(format!("invalid user private key: {e}")) as Error)?
            .into();

        if let Some(ref userid) = sopts.client.user_id {
            let fromid = Id::from(user_key.public_key());
            if userid != &fromid {
                return Err(ArgumentError::new("userId does not match userPrivateKey") as Error);
            }
        }

        let device_key: KeyPair = sopts.client
            .device_private_key
            .parse::<PrivateKey>()
            .map_err(|e| ArgumentError::new(format!("invalid device private key: {e}")) as Error)?
            .into();

        if let Some(ref deviceid) = sopts.client.device_id {
            let fromid = Id::from(device_key.public_key());
            if deviceid != &fromid {
                return Err(ArgumentError::new("deviceId does not match devicePrivateKey") as Error);
            }
        }
        b.with_user_keypair(user_key);
        b.with_device_keypair(device_key);

        let path = if let Some(data_dir) = sopts.data_dir {
            expand_home_dir(&data_dir)
        } else {
            get_current_path()
        };
        b.with_data_dir(path);

        if let Some(database) = sopts.database {
            b.with_database(&database.uri, database.pool_size)?
                .with_database_schema(&database.schema);
        } else {
            let database = SerdeDatabase {
                uri: "jdbc:sqlite:messaging.db".to_string(),
                pool_size: 1,
                schema: "public".to_string(),
            };
            b.with_database(&database.uri, database.pool_size)?
                .with_database_schema(&database.schema);
        }

        let log_level = sopts
            .log_level
            .unwrap_or_else(|| "info".to_string())
            .parse::<log::LevelFilter>()
            .map_err(|e| ArgumentError::new(format!("invalid logLevel: {e}")))?;
        b.with_log_level(log_level);

        if let Some(log_file) = sopts.log_file {
            b.with_log_file(log_file);
        }
        b.with_log_console(sopts.log_console.unwrap_or(true));
        Ok(b)
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
