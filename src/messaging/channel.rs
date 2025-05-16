use crate::{
    Id,
};

use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Permission {
    Public = 0,
    MemberInvite = 1,
    ModeratorInvite = 2,
    OwnerInvite = 3
}

#[derive(Debug, Clone, Deserialize, Hash)]
#[allow(unused)]
pub struct Channel {
    #[serde(rename = "id")]
    owner: Id,

    #[serde(rename = "pm")]
    permission: Permission,

    #[serde(skip)]
    notice: String,
}

#[allow(dead_code)]
impl Channel {
    // TODO:
}

pub struct Member {}
pub struct Role {}
