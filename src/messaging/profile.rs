use std::fmt;
use sha2::{Digest, Sha256};
use unicode_normalization::UnicodeNormalization;
use serde::{Serialize, Deserialize};

use crate::Id;

#[allow(unused)]
#[derive(Debug, Clone, Serialize, Deserialize, Hash)]
pub struct Profile {
	#[serde(rename = "id")]
	id: Id,

	#[serde(rename = "p")]
    home_peerid: Id,

    #[serde(rename = "ps")]
    home_peer_sig: Vec<u8>,

	#[serde(rename = "n")]
    name: String,

	#[serde(rename = "a")]
    avatar: bool,

	#[serde(rename = "nt")]
    notice: Option<String>,

    #[serde(rename = "s")]
    sig: Vec<u8>
}

#[allow(unused)]
impl Profile {
    pub(crate) fn new(id: &Id,
            home_peerid: &Id,
            name: &str,
            avatar: bool,
            notice: &str,
            home_peer_sig: &[u8],
            sig: &[u8]
    ) -> Self {
        Self {
            id			: id.clone(),
            home_peerid	: home_peerid.clone(),
            home_peer_sig: home_peer_sig.to_vec(),
            name		: name.to_string(),
            avatar		: avatar,
            notice		: Some(notice.to_string()),
            sig			: sig.to_vec()
        }
    }

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
        unimplemented!()

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

