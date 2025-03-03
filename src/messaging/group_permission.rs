use serde::{Deserialize, Serialize};

use crate::{
    Error,
    error::Result,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GroupPermission {
    Public = 0,
    MemberInvite = 1,
    ModeratorInvite = 2,
    OwnerInvite = 3,
}

#[allow(dead_code)]
impl GroupPermission {
    pub(crate) fn value(&self) -> i32 {
        *self as i32
    }

    pub(crate) fn from_value(value: i32) -> Result<Self> {
        match value {
            0 => Ok(GroupPermission::Public),
            1 => Ok(GroupPermission::MemberInvite),
            2 => Ok(GroupPermission::ModeratorInvite),
            3 => Ok(GroupPermission::OwnerInvite),
            _ => Err(Error::Argument(format!("Invalid permisson value"))),
        }
    }
}
/*
impl<'de> Deserialize<'de> for Permission {
    fn deserialize<D>(deserializer: D) -> Result<Self>
    where D: Deserializer<'de>
    {
        struct PermissionVisitor;

        impl<'de> Visitor<'de> for PermissionVisitor {
            type Value = Permission;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a valid permission integer (0-3)")
            }

            fn visit_i32<E>(self, value: i32) -> Result<Self::Value, E>
            where E: de::Error,
            {
                Permission::from_value(value).map_err(de::Error::custom)
            }
        }

        deserializer.deserialize_i32(PermissionVisitor).map_err(|e| Error::from(e))
    }
}*/
