use std::time::SystemTime;
use serde::{
	Deserialize,
	Serialize,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct ClientDevice {
	//#[serde(rename = "id")]
	//device_id: Id,

	//#[serde(rename = "clientId")]
	//client_id: String,

	#[serde(rename = "name")]
	device_name: String,

    #[serde(rename = "app")]
	app_name: String,

	#[serde(rename = "created")]
    created_timestamp: i64, // Timestamp in Unix format (use i64 for time)

	#[serde(rename = "banned")]
	is_banned: bool,

	#[serde(rename = "lastActivity")]
	last_activity_timestamp: i64,

	#[serde(rename = "lastAddress")]
    last_activity_address: Option<String>, // Optional address field
}

impl ClientDevice {
	// TODO: for new function.

	//pub fn client_id(&self) -> &str {
	//	&self.client_id
	//}

	//pub fn device_id(&self) -> &Id {
	//	&self.device_id
	//}

	pub fn device_name(&self) -> &str {
		&self.device_name
	}

	pub fn app_name(&self) -> &str {
		&self.app_name
	}

	pub fn created(&self) -> SystemTime {
		unimplemented!()
	}

	pub fn is_banned(&self) -> bool {
		self.is_banned
	}

	pub fn last_actitivity(&self) -> SystemTime {
		unimplemented!()
	}

	pub fn last_address(&self) -> Option<&str> {
		unimplemented!()
	}
}

impl PartialEq for ClientDevice {
    fn eq(&self, _: &Self) -> bool {
		//self.device_id == other.device_id
		false
    }
}
