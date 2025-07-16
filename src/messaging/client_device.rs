use std::fmt;
use std::time::{Duration, SystemTime};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use bs58;

use crate::{
	as_secs,
	Id
};

#[derive(Debug, Clone, Serialize, Deserialize, Hash)]
pub struct ClientDevice {
	#[serde(skip)]
	client_id: String,	// MQTT Client ID

	#[serde(rename = "id")]
	id		: Id,
	#[serde(rename = "n")]
	name 	: String,
    #[serde(rename = "a")]
	app_name: Option<String>,
	#[serde(rename = "c")]
    created	: u64,
	#[serde(rename = "ls")]
	last_seen: u64,
	#[serde(rename = "la")]
    last_address: String,
}

impl ClientDevice {
	#[cfg(test)]
	pub(crate) fn new(id: &Id, device_name: &str, app_name: Option<&str>,
		created: u64, last_seen: u64, last_address: &str) -> Self {

		Self {
			id			: id.clone(),
			name		: device_name.to_string(),
			app_name	: app_name.map(|v|v.to_string()),
			last_address: last_address.to_string(),

			created,
			last_seen,
			client_id	: Self::digest(id),
		}
	}

	fn digest(id: &Id) -> String {
		bs58::encode(
			&Sha256::digest(id.as_bytes())
		).into_string()
	}

	pub fn update_client_id(&mut self) {
		self.client_id = Self::digest(&self.id);
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

	pub fn app_name(&self) -> Option<&str> {
		self.app_name.as_deref()
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

		if self.name.is_empty() {
			write!(f, ", name=default")?;
		}
		self.app_name.as_ref().map(|v| {
			write!(f, ", app={}", v)
		});

		write!(f, ", created={}", self.created)?;
		if self.last_seen > 0 {
			write!(f, ", lastSeen={}, address={}",
			as_secs!(SystemTime::now()) - self.last_seen,
			self.last_seen)?;
		}
		write!(f, "]")
	}
}