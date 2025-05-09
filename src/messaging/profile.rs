use sha2::{Digest, Sha256};
use unicode_normalization::UnicodeNormalization;
use serde::{Serialize, Deserialize};

use crate::{
    Id,
};

#[allow(unused)]
#[derive(Debug, Clone, Serialize, Deserialize, Hash)]
pub struct ClientDevice {
	#[serde(rename = "id")]
	id: Id,

	#[serde(skip)]
	_client_id: String,

	#[serde(rename = "p")]
    home_peerid: Id,

    #[serde(rename = "ps")]
    home_peer_sig: Vec<u8>,

	#[serde(rename = "n")]
    name: String,

	#[serde(rename = "a")]
    avatar: bool,

	#[serde(rename = "nt")]
    notice: String,

    #[serde(rename = "s")]
    sig: Vec<u8>
}

pub(crate) fn digest(id: &Id, home_peerid: &Id, name: Option<&str>, avatar: bool, notice: Option<&str>) -> Vec<u8> {
    let mut sha256 = Sha256::new();
    sha256.update(id.as_bytes());
    sha256.update(home_peerid.as_bytes());

    name.map(|v| {
        sha256.update(v.nfc().collect::<String>().as_bytes());
    });

    let avatar: u8 = avatar as u8;
    sha256.update(std::slice::from_ref(&avatar));
    notice.map(|v| {
        sha256.update(v.nfc().collect::<String>().as_bytes());
    });

    sha256.finalize().to_vec()
}

