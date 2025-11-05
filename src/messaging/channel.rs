use std::fmt;
use std::collections::HashMap;
use serde::{Serialize, Deserialize};
use serde_repr::{Serialize_repr, Deserialize_repr};
use crate::{
    Id,
    CryptoContext,
    cryptobox,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[derive(Serialize_repr, Deserialize_repr)]
#[repr(i32)]
pub enum Permission {
    Public          = 0,
    MemberInvite    = 1,
    ModeratorInvite = 2,
    OwnerInvite     = 3
}

impl TryFrom<i32> for Permission {
    type Error = &'static str;

    fn try_from(value: i32) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Permission::Public),
            1 => Ok(Permission::MemberInvite),
            2 => Ok(Permission::ModeratorInvite),
            3 => Ok(Permission::OwnerInvite),
            _ => Err("Invalid permission value"),
        }
    }
}

impl From<Permission> for i32 {
    fn from(p: Permission) -> Self {
        p as i32
    }
}

impl fmt::Display for Permission {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(match *self {
            Permission::Public => "Public",
            Permission::MemberInvite => "MemberInvite",
            Permission::ModeratorInvite => "ModeratorInvite",
            Permission::OwnerInvite => "OwnerInvite",
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[derive(Serialize_repr, Deserialize_repr)]
#[repr(i32)]
pub enum Role {
    Owner = 0,
    Moderator = 1,
    Member = 2,
    Banned = -1,
}

impl Role {
    pub fn is_banned(&self) -> bool {
        matches!(self, Role::Banned)
    }
}

impl TryFrom<i32> for Role {
    type Error = &'static str;

    fn try_from(value: i32) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Role::Owner),
            1 => Ok(Role::Moderator),
            2 => Ok(Role::Member),
            -1 => Ok(Role::Banned),
            _ => Err("Invalid role value"),
        }
    }
}

impl From<Role> for i32 {
    fn from(p: Role) -> Self {
        p as i32
    }
}

impl fmt::Display for Role {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(match *self {
            Role::Owner => "Owner",
            Role::Moderator => "Moderator",
            Role::Member => "Member",
            Role::Banned => "Banned",
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct Member {
    #[serde(rename = "id")]
    id: Id,

    #[serde(rename = "p")]
    home_peerid: Id,

    #[serde(rename = "r")]
    role: Role,

    #[serde(rename = "j")]
    joined: u64,

    // TODO: channel.
}

#[allow(unused)]
impl Member {
    pub(crate) fn new(id: &Id, home_peerid: &Id, role: Role, joined: u64) -> Self {
        Self {
            id			: id.clone(),
            home_peerid	: home_peerid.clone(),
            role		: role,
            joined		: joined,
        }
    }

    pub fn id(&self) -> &Id {
        &self.id
    }

    pub fn role(&self) -> Role {
        self.role
    }

    pub(crate) fn set_role(&mut self, role: Role) {
        self.role = role;
    }

    pub fn is_owner(&self) -> bool {
        self.role == Role::Owner
    }

    pub fn is_moderator(&self) -> bool {
        self.role == Role::Moderator
    }

    pub fn is_banned(&self) -> bool {
        self.role == Role::Banned
    }

    pub fn joined(&self) -> u64 {
        self.joined
    }

    // TODO: get contact.
    // pub fn contact(&self) -> Option<&Contact> {}
    // pub fn display_name(&self) -> String {
}

impl fmt::Display for Member {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}, {}, {}", self.id.to_base58(), self.role, self.joined)?;
        Ok(())
    }
}

#[derive(Debug, Deserialize)]
#[allow(unused)]
pub struct Channel {
    #[serde(rename = "o")]
    owner: Id,

    #[serde(rename = "pm")]
    permission: Permission,

    #[serde(skip)]
    notice: String,

    #[serde(skip)]
    mamber_crypto_context: HashMap<Id, CryptoContext>,

}

#[allow(unused)]
impl Channel {

    pub fn owner(&self) -> &Id {
        &self.owner
    }

    pub(crate) fn set_owner(&mut self, owner: Id) {
        self.owner = owner;
        self.touch();
    }

    pub fn permission(&self) -> Permission {
        self.permission
    }

    pub(crate) fn set_permission(&mut self, permission: Permission) {
        self.permission = permission;
        self.touch();
    }

    pub(crate) fn session_keypair(&self) -> Option<&cryptobox::KeyPair> {
        unimplemented!()
    }

    pub(crate) fn is_owner(&self, id: &Id) -> bool {
        self.owner == *id
    }

    pub(crate) fn is_member(&self, id: &Id) -> bool {
        unimplemented!()
    }

    pub(crate) fn is_moderator(&self, id: &Id) -> bool {
        unimplemented!()
    }

    fn touch(&mut self) {
        unimplemented!()
    }

    pub(crate) fn rx_crypto_context(&self, id: &Id) -> &CryptoContext {
        unimplemented!()
    }

    pub(crate) fn rx_crypto_context1(&self) -> &CryptoContext {
        unimplemented!()
    }
}

/* Removed duplicate empty Member struct */
