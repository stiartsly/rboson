use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    str::FromStr,
};

use serde_json::{Map, Value};

use crate::messaging::errors::{Error, Result};
use crate::{signature, Id};

const SCHEME_MQTT: &str = "mqtt";
const SCHEME_MQTTS: &str = "mqtts";
const DEFAULT_DATABASE_URI: &str = "jdbc:sqlite:photonmessaging.db";

/// Configuration for the messaging client.
#[derive(Clone)]
pub struct Configuration {
    pub service_peer_id: Id,
    pub service_endpoint: Option<url::Url>,
    pub user_key: signature::KeyPair,
    pub device_key: signature::KeyPair,
    pub data_dir: PathBuf,
    pub database_uri: String,
    pub database_pool_size: usize,
    pub database_schema_name: Option<String>,

    /// SQLite path retained for compatibility with the current built-in store.
    pub database_path: PathBuf,
}

impl Configuration {
    pub fn default_data_dir() -> PathBuf {
        let base = std::env::var_os("XDG_DATA_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                let mut home = std::env::var_os("HOME")
                    .map(PathBuf::from)
                    .unwrap_or_else(|| PathBuf::from("."));
                home.push(".local/share");
                home
            });
        base.join("boson/client/photon-messaging")
    }

    pub fn builder() -> ConfigurationBuilder {
        ConfigurationBuilder::default()
    }

    /// Construct a configuration with the original Rust API.
    pub fn new(
        service_peer_id: Id,
        service_endpoint: Option<url::Url>,
        user_key: signature::KeyPair,
        device_key: signature::KeyPair,
        data_dir: Option<PathBuf>,
    ) -> Self {
        let data_dir = data_dir.unwrap_or_else(Self::default_data_dir);
        let database_path = data_dir.join("photonmessaging.db");
        Self {
            service_peer_id,
            service_endpoint,
            user_key,
            device_key,
            data_dir,
            database_uri: DEFAULT_DATABASE_URI.to_string(),
            database_pool_size: 0,
            database_schema_name: None,
            database_path,
        }
    }

    pub fn validate_endpoint(url: &url::Url) -> Result<()> {
        let scheme = url.scheme();
        if scheme != SCHEME_MQTT && scheme != SCHEME_MQTTS {
            return Err(Error::Argument(format!(
                "Invalid endpoint scheme '{scheme}': expected 'mqtt' or 'mqtts'"
            )));
        }
        if url.host_str().is_none() {
            return Err(Error::Argument("Endpoint is missing a hostname".into()));
        }
        if url.port().is_none() {
            return Err(Error::Argument(
                "Endpoint must specify a port (1-65535)".into(),
            ));
        }
        Ok(())
    }

    pub fn from_map(map: &HashMap<String, Value>) -> Result<Self> {
        let service = object(map.get("service"), "service")?;
        let client = object(map.get("client"), "client")?;
        let database = object(map.get("database"), "database")?;

        let mut builder = Self::builder()
            .service_peer_id(parse_id(string(service.get("peerId"), "service.peerId")?)?)?
            .user_key(parse_key(string(
                client.get("userPrivateKey"),
                "client.userPrivateKey",
            )?)?)?
            .device_key(parse_key(string(
                client.get("devicePrivateKey"),
                "client.devicePrivateKey",
            )?)?)?
            .database_uri(string(database.get("uri"), "database.uri")?)?;

        if let Some(endpoint) = optional_string(service.get("endpoint"), "service.endpoint")? {
            builder = builder.service_endpoint(endpoint)?;
        }
        if let Some(data_dir) = optional_string(map.get("dataDir"), "dataDir")? {
            builder = builder.data_dir(data_dir);
        }
        if let Some(pool_size) = database.get("poolSize") {
            let pool_size = pool_size
                .as_u64()
                .ok_or_else(|| Error::Argument("database.poolSize must be non-negative".into()))?;
            builder = builder.database_pool_size(pool_size as usize);
        }
        if let Some(schema) = optional_string(database.get("schema"), "database.schema")? {
            builder = builder.database_schema_name(schema);
        }
        builder.build()
    }

    pub fn to_map(&self) -> HashMap<String, Value> {
        let mut service = Map::new();
        service.insert(
            "peerId".into(),
            Value::String(self.service_peer_id.to_string()),
        );
        if let Some(endpoint) = &self.service_endpoint {
            service.insert("endpoint".into(), Value::String(endpoint.to_string()));
        }

        let mut client = Map::new();
        client.insert(
            "userPrivateKey".into(),
            Value::String(self.user_key.private_key().to_base58()),
        );
        client.insert(
            "devicePrivateKey".into(),
            Value::String(self.device_key.private_key().to_base58()),
        );

        let mut database = Map::new();
        database.insert("uri".into(), Value::String(self.database_uri.clone()));
        if self.database_pool_size != 0 {
            database.insert(
                "poolSize".into(),
                Value::Number(self.database_pool_size.into()),
            );
        }
        if let Some(schema) = &self.database_schema_name {
            database.insert("schema".into(), Value::String(schema.clone()));
        }

        HashMap::from([
            ("service".into(), Value::Object(service)),
            ("client".into(), Value::Object(client)),
            (
                "dataDir".into(),
                Value::String(self.data_dir.to_string_lossy().into_owned()),
            ),
            ("database".into(), Value::Object(database)),
        ])
    }
}

#[derive(Default)]
pub struct ConfigurationBuilder {
    service_peer_id: Option<Id>,
    service_endpoint: Option<url::Url>,
    user_key: Option<signature::KeyPair>,
    device_key: Option<signature::KeyPair>,
    data_dir: Option<PathBuf>,
    database_uri: Option<String>,
    database_pool_size: usize,
    database_schema_name: Option<String>,
}

impl ConfigurationBuilder {
    pub fn service(mut self, peer_id: Id, endpoint: &str) -> Result<Self> {
        self = self.service_peer_id(peer_id)?;
        self.service_endpoint(endpoint)
    }

    pub fn service_peer_id(mut self, peer_id: Id) -> Result<Self> {
        self.service_peer_id = Some(peer_id);
        Ok(self)
    }

    pub fn service_endpoint(mut self, endpoint: &str) -> Result<Self> {
        let endpoint = url::Url::parse(endpoint)
            .map_err(|error| Error::Argument(format!("Invalid endpoint: {error}")))?;
        Configuration::validate_endpoint(&endpoint)?;
        self.service_endpoint = Some(endpoint);
        Ok(self)
    }

    pub fn user_key(mut self, key: signature::KeyPair) -> Result<Self> {
        self.user_key = Some(key);
        Ok(self)
    }

    pub fn user_key_str(self, key: &str) -> Result<Self> {
        self.user_key(parse_key(key)?)
    }

    pub fn generate_user_key(self) -> Result<Self> {
        self.user_key(signature::KeyPair::random())
    }

    pub fn device_key(mut self, key: signature::KeyPair) -> Result<Self> {
        self.device_key = Some(key);
        Ok(self)
    }

    pub fn device_key_str(self, key: &str) -> Result<Self> {
        self.device_key(parse_key(key)?)
    }

    pub fn generate_device_key(self) -> Result<Self> {
        self.device_key(signature::KeyPair::random())
    }

    pub fn data_dir(mut self, data_dir: impl AsRef<Path>) -> Self {
        self.data_dir = Some(data_dir.as_ref().to_path_buf());
        self
    }

    pub fn database(mut self, uri: &str, pool_size: usize) -> Result<Self> {
        self = self.database_uri(uri)?;
        Ok(self.database_pool_size(pool_size))
    }

    pub fn database_uri(mut self, uri: &str) -> Result<Self> {
        if uri.trim().is_empty() {
            return Err(Error::Argument("Database URI is empty".into()));
        }
        self.database_uri = Some(uri.to_string());
        Ok(self)
    }

    pub fn database_pool_size(mut self, pool_size: usize) -> Self {
        self.database_pool_size = pool_size;
        self
    }

    pub fn database_schema_name(mut self, schema: impl Into<String>) -> Self {
        self.database_schema_name = Some(schema.into());
        self
    }

    pub fn build(self) -> Result<Configuration> {
        let service_peer_id = self
            .service_peer_id
            .ok_or_else(|| Error::Argument("service_peer_id must be set".into()))?;
        let user_key = self
            .user_key
            .ok_or_else(|| Error::Argument("user_key must be set".into()))?;
        let device_key = self
            .device_key
            .ok_or_else(|| Error::Argument("device_key must be set".into()))?;
        let data_dir = self
            .data_dir
            .unwrap_or_else(Configuration::default_data_dir);
        let database_uri = self
            .database_uri
            .unwrap_or_else(|| DEFAULT_DATABASE_URI.to_string());
        let database_path = sqlite_path(&database_uri)
            .map(PathBuf::from)
            .map(|path| {
                if path.is_absolute() {
                    path
                } else {
                    data_dir.join(path)
                }
            })
            .unwrap_or_else(|| data_dir.join("photonmessaging.db"));

        Ok(Configuration {
            service_peer_id,
            service_endpoint: self.service_endpoint,
            user_key,
            device_key,
            data_dir,
            database_uri,
            database_pool_size: self.database_pool_size,
            database_schema_name: self.database_schema_name,
            database_path,
        })
    }
}

fn sqlite_path(uri: &str) -> Option<&str> {
    uri.strip_prefix("jdbc:sqlite:")
        .or_else(|| uri.strip_prefix("sqlite:"))
}

fn object<'a>(value: Option<&'a Value>, field: &str) -> Result<&'a Map<String, Value>> {
    value
        .and_then(Value::as_object)
        .ok_or_else(|| Error::Argument(format!("Missing or invalid {field} configuration")))
}

fn string<'a>(value: Option<&'a Value>, field: &str) -> Result<&'a str> {
    value
        .and_then(Value::as_str)
        .ok_or_else(|| Error::Argument(format!("Missing or invalid {field}")))
}

fn optional_string<'a>(value: Option<&'a Value>, field: &str) -> Result<Option<&'a str>> {
    value
        .map(|value| {
            value
                .as_str()
                .ok_or_else(|| Error::Argument(format!("Invalid {field}")))
        })
        .transpose()
}

fn parse_id(value: &str) -> Result<Id> {
    Id::from_str(value).map_err(|error| Error::Argument(error.to_string()))
}

fn parse_key(value: &str) -> Result<signature::KeyPair> {
    signature::KeyPair::from_str(value).map_err(|error| Error::Argument(error.to_string()))
}

#[cfg(test)]
mod tests {
    use super::Configuration;
    use crate::Id;

    #[test]
    fn rejects_invalid_service_endpoint() {
        let result = Configuration::builder()
            .service_peer_id(Id::random())
            .unwrap()
            .service_endpoint("tcp://10.0.0.1:8883");

        assert!(result.is_err());
    }

    #[test]
    fn configuration_map_round_trip() {
        let config = Configuration::builder()
            .service(Id::random(), "mqtts://10.0.0.1:8883")
            .unwrap()
            .generate_user_key()
            .unwrap()
            .generate_device_key()
            .unwrap()
            .data_dir("/tmp/photon-messaging")
            .database("postgresql://localhost:5432/test", 4)
            .unwrap()
            .database_schema_name("photon")
            .build()
            .unwrap();

        let decoded = Configuration::from_map(&config.to_map()).unwrap();
        assert_eq!(decoded.service_peer_id, config.service_peer_id);
        assert_eq!(decoded.service_endpoint, config.service_endpoint);
        assert_eq!(
            decoded.user_key.private_key(),
            config.user_key.private_key()
        );
        assert_eq!(
            decoded.device_key.private_key(),
            config.device_key.private_key()
        );
        assert_eq!(decoded.database_uri, config.database_uri);
        assert_eq!(decoded.database_pool_size, 4);
        assert_eq!(decoded.database_schema_name.as_deref(), Some("photon"));
    }
}
