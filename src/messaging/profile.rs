use std::fmt;
use sha2::{Digest, Sha256};
use unicode_normalization::UnicodeNormalization;
use serde::Deserialize;

use crate::Id;

#[derive(Debug, Clone, Deserialize, Hash)]
pub struct Profile {
	#[serde(rename = "id")]
	id: Id,

	#[serde(rename = "p")]
    home_peerid: Id,

    #[serde(rename = "ps")]
    #[serde(with = "super::serde_bytes_with_base64")]
    home_peer_sig: Vec<u8>,

	#[serde(rename = "n")]
    name: String,

	#[serde(rename = "a")]
    #[serde(skip)]
    avatar: bool,

	#[serde(rename = "nt")]
    notice: Option<String>,

    #[serde(rename = "s")]
    #[serde(with = "super::serde_bytes_with_base64")]
    sig: Vec<u8>
}

unsafe impl Send for Profile {}
unsafe impl Sync for Profile {}

#[allow(unused)]
impl Profile {
    pub fn id(&self) -> &Id {
        &self.id
    }

    pub fn home_peerid(&self) -> &Id {
        &self.home_peerid
    }

    pub fn home_peer_sig(&self) -> &[u8] {
        &self.home_peer_sig
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn has_avatar(&self) -> bool {
        self.avatar
    }

    pub fn notice(&self) -> Option<&str> {
        self.notice.as_ref().map(|v| v.as_str())
    }

    pub fn sig(&self) -> &[u8] {
        &self.sig
    }

    pub fn is_genuine(&self) -> bool {
        //TODO: unimplemented!();
        true
    }
}

impl fmt::Display for Profile {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Profile: {}[homePeer={}]",
            self.id,
            self.home_peerid
        )?;

        if !self.name.is_empty() {
            write!(f, ", name={}", self.name)?;
        }

        write!(f, ", avatar={}", if self.avatar { "yes" } else { "no" })?;

        if let Some(notice) = self.notice.as_ref() {
            write!(f, ", notice={}", notice)?;
        }

        write!(f, "]")?;
        Ok(())
    }
}

pub(crate) fn digest(id: &Id,
    peerid: &Id,
    name: Option<&str>,
    avatar: bool,
    notice: Option<&str>
) -> Vec<u8> {

    let mut sha256 = Sha256::new();
    sha256.update(id.as_bytes());
    sha256.update(peerid.as_bytes());

    name.map(|v| {
        sha256.update(v.nfc().collect::<String>().as_bytes());
    });

    let avatar: u8 = avatar as u8;
    sha256.update(&[avatar]);
    notice.map(|v| {
        sha256.update(v.nfc().collect::<String>().as_bytes());
    });

    sha256.finalize().to_vec()
}

