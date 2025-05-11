use std::fmt;
use std::time::{Duration, SystemTime};
use serde::{
	Deserialize,
	Serialize,
};

use crate::Id;

#[derive(Debug, Clone, Serialize, Deserialize, Hash)]
#[allow(unused)]
pub struct ClientDevice {
	#[serde(skip)]
	client_id: String,

	#[serde(rename = "id")]
	id		: Id,
	#[serde(rename = "n")]
	name	: Option<String>,
    #[serde(rename = "a")]
	app_name: Option<String>,
	#[serde(rename = "c")]
    created	: u64, // Timestamp in Unix format (use i64 for time)
	#[serde(rename = "ls")]
	last_seen: u64,
	#[serde(rename = "la")]
    last_address: String,
}

#[allow(dead_code)]
impl ClientDevice {
	pub(crate) fn new(id: &Id, device_name: Option<&str>, app_name: Option<&str>,
		created: u64, last_seen: u64, last_address: &str) -> Self {
		Self {
			client_id: "TODO".to_string(),
			id		: id.clone(),
			name	: device_name.map(|v| v.to_string()),
			app_name: app_name.map(|v| v.to_string()),
			created,
			last_seen,
			last_address: last_address.to_string(),

		}
	}

	pub fn id(&self) -> &Id {
		&self.id
	}

	pub fn client_id(&self) -> &str {
		&self.client_id
	}

	pub fn name(&self) -> &str {
		self.name.as_ref().map(|v| v.as_str()).unwrap_or("N/A")
	}

	pub fn app_name(&self) -> &str {
		self.name.as_ref().map(|v| v.as_str()).unwrap_or("N/A")
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
		write!(f, "Device: {} [clientId={} ",
			self.id.to_base58(),
			self.client_id,
		)?;
		if let Some(name) = self.name.as_ref() {
			write!(f, "name={}", name)?;
		}
		if let Some(app_name) = self.app_name.as_ref() {
			write!(f, ", app={}", app_name)?;
		}
		// TODO:
		write!(f, "]")
	}
}