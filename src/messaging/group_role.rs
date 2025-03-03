use serde::{Deserialize, Serialize};

use crate::{
    Error,
    error::Result,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum GroupRole {
    Owner = 0,
    Moderator = 1,
    Member = 2,
    Banned = -1,
}

#[allow(dead_code)]
impl GroupRole {
    pub(crate) fn value(&self) -> i32 {
        *self as i32
    }

    pub(crate) fn from_value(value: i32) -> Result<Self> {
        match value {
            0 => Ok(GroupRole::Owner),
            1 => Ok(GroupRole::Moderator),
            2 => Ok(GroupRole::Member),
            -1 => Ok(GroupRole::Banned),
            _ => Err(Error::Argument(format!("Invalid role raw value"))),
        }
    }
}
