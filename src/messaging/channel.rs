use crate::{
    Id,
};

use serde::{
    Deserialize
};

#[derive(Debug, Clone, Deserialize, Hash)]
struct Permission {}

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
