use std::fmt;
use std::time::{SystemTime, Duration};
use serde::Deserialize;
use crate::Id;

#[derive(Clone, Debug, Deserialize)]
pub struct Profile {
    #[serde(rename="id")]
    id: Id,
    #[serde(default)]
    admin: bool,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    avatar: Option<String>,
    #[serde(default)]
    email: Option<String>,
    #[serde(default)]
    bio: Option<String>,
    #[serde(rename="createdAt")]
    created_at: u64,
    #[serde(rename="updatedAt")]
    updated_at: u64,
    #[serde(rename="planName")]
    plan_name: String,
    #[serde(rename="passphraseProtected")]
    passphrase_protected: bool,
}

impl Profile {
    pub fn id(&self) -> Id {
        self.id
    }

    pub fn admin(&self) -> bool {
        self.admin
    }

    pub fn name(&self) -> Option<&String> {
        self.name.as_ref()
    }

    pub fn avatar(&self) -> Option<&String> {
        self.avatar.as_ref()
    }

    pub fn email(&self) -> Option<&String> {
        self.email.as_ref()
    }

    pub fn bio(&self) -> Option<&String> {
        self.bio.as_ref()
    }

    pub fn created_at(&self) -> SystemTime {
        SystemTime::UNIX_EPOCH + Duration::from_secs(self.created_at)
    }

    pub fn updated_at(&self) -> SystemTime {
        SystemTime::UNIX_EPOCH + Duration::from_secs(self.updated_at)
    }

    pub fn plan_name(&self) -> &String {
        &self.plan_name
    }

    pub fn is_passphrase_protected(&self) -> bool {
        self.passphrase_protected
    }
}

impl fmt::Display for Profile {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Profile{{id={}, name={:?}, email={:?}, plan={}, admin={}, passphrase_protected={}}}",
            self.id,
            self.name,
            self.email,
            self.plan_name,
            self.admin,
            self.passphrase_protected
        )
    }
}
