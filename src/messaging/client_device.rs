use std::fmt;
use std::time::{Duration, SystemTime};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use bs58;

use crate::Id;

#[derive(Debug, Clone, Serialize, Deserialize, Hash)]
#[allow(unused)]
pub struct ClientDevice {
	#[serde(skip)]
	client_id: String,

	#[serde(rename = "id")]
	id		: Id,
	#[serde(rename = "n")]
	name	: String,
    #[serde(rename = "a")]
	app_name: String,
	#[serde(rename = "c")]
    created	: u64,
	#[serde(rename = "ls")]
	last_seen: u64,
	#[serde(rename = "la")]
    last_address: String,
}

#[allow(unused)]
impl ClientDevice {
	pub(crate) fn new(id: &Id, device_name: &str, app_name: &str,
		created: u64, last_seen: u64, last_address: &str) -> Self {

		Self {
			client_id	: bs58::encode(&Sha256::digest(id.as_bytes())).into_string(),
			id			: id.clone(),
			name		: device_name.into(),
			app_name	: app_name.into(),
			last_address: last_address.into(),

			created,
			last_seen,
		}
	}

	pub fn id(&self) -> &Id {
		&self.id
	}

	pub fn client_id(&self) -> &str {
		&self.client_id
	}

	pub fn name(&self) -> &str {
		&self.name
	}

	pub fn app_name(&self) -> &str {
		&self.app_name
	}

	pub fn created(&self) -> SystemTime {
		SystemTime::UNIX_EPOCH + Duration::from_secs(self.created)
	}

	pub fn last_seen(&self) -> SystemTime {
		SystemTime::UNIX_EPOCH + Duration::from_secs(self.last_seen)
	}

	pub fn last_address(&self) -> &str {
		&self.last_address
	}
}

impl PartialEq for ClientDevice {
    fn eq(&self, device: &Self) -> bool {
		self.id == device.id
    }
}

impl fmt::Display for ClientDevice {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		write!(f, "Device: {} [clientId={}, name={}, app={}",
			self.id.to_base58(),
			self.client_id,
			self.name,
			self.app_name
		)?;
		// TODO:
		write!(f, "]")
	}
}