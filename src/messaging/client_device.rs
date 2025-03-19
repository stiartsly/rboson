use std::fmt;
use std::time::SystemTime;
use serde::{
	Deserialize,
	Serialize,
};

use crate::Id;

#[derive(Debug, Clone, Serialize, Deserialize, Hash)]
#[allow(dead_code)]
pub struct ClientDevice {
	//#[serde(rename = "id")]
	//id: Id,

	//client_id: String,

	#[serde(rename = "n")]
	name: String,

    #[serde(rename = "a")]
	app: String,

	#[serde(rename = "c")]
    created: u64, // Timestamp in Unix format (use i64 for time)

	#[serde(rename = "ls")]
	last_seen: u64,

	#[serde(rename = "la")]
    last_address: String,
}

#[allow(dead_code)]
impl ClientDevice {
	pub(crate) fn new(_id: &Id, device_name: String, app_name: String,
		created: u64, last_seen: u64, last_address: String) -> Self {
		Self {
			name: device_name,
			app: app_name,
			created,
			last_seen,
			last_address,
		}
	}

	pub fn id(&self) -> &Id {
		unimplemented!()
	}

	pub fn client_id(&self) -> &str {
		unimplemented!()
	}

	pub fn name(&self) -> &str {
		&self.name
	}

	pub fn app(&self) -> &str {
		&self.app
	}

	pub fn created(&self) -> SystemTime {
		SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(self.created)
	}

	pub fn last_seen(&self) -> SystemTime {
		SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(self.last_seen)
	}

	pub fn last_address(&self) -> &str {
		&self.last_address
	}
}

impl PartialEq for ClientDevice {
    fn eq(&self, _: &Self) -> bool {
		unimplemented!()
    }
}

impl fmt::Display for ClientDevice {
	fn fmt(&self, _f: &mut fmt::Formatter) -> fmt::Result {
		//write!(f, "{}", self.name);
		unimplemented!()
	}
}
