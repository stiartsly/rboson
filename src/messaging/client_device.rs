use std::fmt;
use std::time::{Duration, SystemTime};
use serde::{Deserialize, Serialize};

#[cfg(test)]
use sha2::{Digest, Sha256};
#[cfg(test)]
use bs58;

use crate::Id;

#[derive(Debug, Clone, Serialize, Deserialize, Hash)]
pub struct ClientDevice {
	#[serde(skip)]
	client_id: String,	// MQTT Client ID

	#[serde(rename = "id", with = "crate::serde_id_as_bytes")]
	id		: Id,
	#[serde(rename = "n")]
	name 	: String,
    #[serde(rename = "a")]
	app_name: String,
	#[serde(rename = "c")]
    created	: u64,
	#[serde(rename = "ls")]
	last_seen: u64,
	#[serde(rename = "la")]
    last_address: String,
}

impl ClientDevice {
	#[cfg(test)]
	pub(crate) fn new(id: &Id, device_name: &str, app_name: &str,
		created: u64, last_seen: u64, last_address: &str) -> Self {

		Self {
			id				: id.clone(),
			name			: device_name.to_string(),
			app_name		: app_name.to_string(),
			last_address	: last_address.to_string(),

			created,
			last_seen,

			client_id		: bs58::encode(&Sha256::digest(&id)).into_string()
		}
	}

	pub fn id(&self) -> &Id {
		&self.id
	}

	#[cfg(test)]
	pub(crate) fn client_id(&self) -> &str {
		&self.client_id
	}

	pub fn name(&self) -> &str {
		&self.name
	}

	pub fn app(&self) -> &str {
		&self.app_name
	}

	pub fn created(&self) -> SystemTime {
		SystemTime::UNIX_EPOCH + Duration::from_millis(self.created)
	}

	pub fn last_seen(&self) -> SystemTime {
		SystemTime::UNIX_EPOCH + Duration::from_millis(self.last_seen)
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
		write!(f, "Device: {} [clientId={}",
			self.id.to_base58(),
			self.client_id
		)?;

		if !self.name.is_empty() {
			write!(f, ", name={}", self.name)?;
		} else {
			write!(f, ", name=N/A")?;
		}

		if !self.app_name.is_empty() {
			write!(f, ", app={}", self.app_name)?;
		} else {
			write!(f, ", app=N/A")?;
		}

		write!(f, ", created={}", self.created)?;
		if self.last_seen > 0 {
			write!(f, ", lastSeen={}, address={}",
			crate::as_secs!(SystemTime::now()) - self.last_seen,
			self.last_address)?;
		}
		write!(f, "]")
	}
}
