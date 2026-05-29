use std::path::PathBuf;
use crate::Id;
use crate::signature;
use crate::messaging::errors::{Error, Result};

/// Scheme prefixes supported for the service endpoint.
const SCHEME_MQTT:  &str = "mqtt";
const SCHEME_MQTTS: &str = "mqtts";

/// Configuration for the messaging client.
pub struct Configuration {
    /// The boson `Id` of the messaging service peer.
    pub service_peer_id: Id,

    /// Optional MQTT(S) endpoint override.  When `None` the client resolves
    /// the endpoint from the DHT via `service_peer_id`.
    pub service_endpoint: Option<url::Url>,

    /// Ed25519 keypair that identifies the *user*.
    pub user_key: signature::KeyPair,

    /// Ed25519 keypair that identifies *this device*.
    pub device_key: signature::KeyPair,

    /// Local directory used to persist state and the SQLite database.
    pub data_dir: PathBuf,

    /// SQLite database file path (relative to `data_dir` or absolute).
    pub database_path: PathBuf,
}

impl Configuration {
    /// The default data directory: `~/.local/share/boson/client/photon-messaging`.
    pub fn default_data_dir() -> PathBuf {
        // Prefer XDG_DATA_HOME, fall back to ~/.local/share.
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

    /// Construct a configuration with the minimum required parameters.
    ///
    /// `data_dir` defaults to [`Configuration::default_data_dir()`] when `None`.
    pub fn new(
        service_peer_id:  Id,
        service_endpoint: Option<url::Url>,
        user_key:         signature::KeyPair,
        device_key:       signature::KeyPair,
        data_dir:         Option<PathBuf>,
    ) -> Self {
        let data_dir = data_dir.unwrap_or_else(Self::default_data_dir);
        let database_path = data_dir.join("photonmessaging.db");
        Self {
            service_peer_id,
            service_endpoint,
            user_key,
            device_key,
            data_dir,
            database_path,
        }
    }

    /// Validate the endpoint URL: must be an absolute `mqtt://` or `mqtts://`
    /// URL with a hostname and a valid port.
    pub fn validate_endpoint(url: &url::Url) -> Result<()> {
        let scheme = url.scheme();
        if scheme != SCHEME_MQTT && scheme != SCHEME_MQTTS {
            return Err(Error::Argument(format!(
                "Invalid endpoint scheme '{}': expected 'mqtt' or 'mqtts'", scheme
            )));
        }
        if url.host_str().is_none() {
            return Err(Error::Argument("Endpoint is missing a hostname".into()));
        }
        match url.port() {
            None | Some(0) => Err(Error::Argument("Endpoint must specify a port (1-65535)".into())),
            _ => Ok(()),
        }
    }
}
