use std::fmt;
use serde::Deserialize;
use crate::Id;

#[derive(Clone, Debug, Deserialize)]
pub struct Device {
    #[serde(rename="id")]
    pub id: Id,

    #[serde(rename="userId")]
    pub user_id: Id,

    #[serde(default)]
    pub name: Option<String>,

    #[serde(default)]
    pub app: Option<String>,

    #[serde(rename="createdAt")]
    pub created_at: u64,

    #[serde(rename="updatedAt")]
    pub updated_at: u64,

    #[serde(rename="lastSeen")]
    pub last_seen: u64,

    #[serde(rename="lastAddress")]
    pub last_address: Option<String>,
}

impl Device {
    pub fn id(&self) -> &Id {
        &self.id
    }

    pub fn user_id(&self) -> &Id {
        &self.user_id
    }

    pub fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    pub fn app(&self) -> Option<&str> {
        self.app.as_deref()
    }

    pub fn created_at(&self) -> u64 {
        self.created_at
    }

    pub fn updated_at(&self) -> u64 {
        self.updated_at
    }

    pub fn last_seen(&self) -> u64 {
        self.last_seen
    }

    pub fn last_address(&self) -> Option<&str> {
        self.last_address.as_deref()
    }
}

impl fmt::Display for Device {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Device {{ id: {}, user_id: {}, name: {:?}, app: {:?}, last_seen: {}}}",
            self.id,
            self.user_id,
            self.name,
            self.app,
            self.last_seen,
        )
    }
}
