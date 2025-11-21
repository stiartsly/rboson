use std::fmt;
use std::sync::{Arc, Mutex};
use std::collections::HashMap;
use serde::{
    Serialize,
    Deserialize,
    ser::{Serializer, SerializeStruct},
    de::{self, Deserializer, Visitor, MapAccess}
};
use serde_repr::{
    Serialize_repr,
    Deserialize_repr
};

use crate::{
    Id,
    CryptoContext,
    messaging::contact::GenericContact,
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
            Permission::Public          => "Public",
            Permission::MemberInvite    => "MemberInvite",
            Permission::ModeratorInvite => "ModeratorInvite",
            Permission::OwnerInvite     => "OwnerInvite",
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
            Role::Owner     => "Owner",
            Role::Moderator => "Moderator",
            Role::Member    => "Member",
            Role::Banned    => "Banned",
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

pub type Channel = GenericContact<ChannelData>;

#[derive(Debug, Clone)]
#[allow(unused)]
pub struct ChannelData {
    owner: Id,
    permission: Permission,
    notice: Option<String>,

    _member_crypto_ctxts: HashMap<Id, Arc<Mutex<CryptoContext>>>,
}

impl ChannelData {
    pub(crate) fn new(owner: Id, permission: Permission, notice: Option<String>) -> Self {
        Self {
            owner,
            permission,
            notice,
            _member_crypto_ctxts: HashMap::new(),
        }
    }
}

#[allow(unused)]
impl Channel {
    pub(crate) fn data(&self) -> &ChannelData {
        self.derived()
    }

    pub(crate) fn data_mut(&mut self) -> &mut ChannelData {
        self.derived_mut()
    }

    pub fn owner(&self) -> &Id {
        unimplemented!()
    }

    pub(crate) fn set_owner(&mut self, _owner: Id) {
        unimplemented!()
    }

    pub fn is_owner(&self, _id: &Id) -> bool {
        // unimplemented!()
        true
    }

    pub fn is_member(&self, _id: &Id) -> bool {
        unimplemented!()
    }

    pub fn is_moderator(&self, _id: &Id) -> bool {
        unimplemented!()
    }

   /* pub(crate) fn session_keypair(&self) -> Option<&cryptobox::KeyPair> {

        unimplemented!()
    }*/

    pub(crate) fn rx_crypto_context_by(&self, _id: &Id) -> Option<&CryptoContext> {
        unimplemented!()
    }

    //pub(crate) fn rx_crypto_context1(&self) -> &CryptoContext {
    //    unimplemented!()
    //}
}
impl Serialize for Channel {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer
    {
        let mut s = serializer.serialize_struct("Channel", 1)?;
        let perm = i32::from(self.data().permission);
        s.serialize_field("pm", &perm)?;
        if let Some(name) = self.name() {
            s.serialize_field("n", name)?;
        }
        s.end()
    }
}

impl<'de> Deserialize<'de> for Channel {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Debug)]
        enum Field {
            Id,             // "id" - Id,
            Peerid,         // "p"  - Id,
            Name,           // "n"  - String
            Remark,         // "r"  - String
            Tags,           // "ts" - String
            Muted,          // "d"  - bool
            Blocked,        // "b"  - bool
            Created,        // "c"  - u64
            LastModified,   // "m" - u64
            Deleted,        // "e"  - bool
            Revision,       // "v"  - i32
            Owner,          // "o"  - Id
            Perm,           // "pm" - i32
            Notice,         // "nt" - String,

            HomePeerSig,    // "ps" - Vec<u8>
            Signature,      // "s"  - Vec<u8>
        }

        impl<'de> Deserialize<'de> for Field {
            fn deserialize<D>(deserializer: D) -> Result<Field, D::Error>
            where
                D: Deserializer<'de>,
            {
                let key = String::deserialize(deserializer)?;
                match key.as_str() {
                    "id"    => Ok(Field::Id),
                    "p"     => Ok(Field::Peerid),
                    "n"     => Ok(Field::Name),
                    "r"     => Ok(Field::Remark),
                    "ts"    => Ok(Field::Tags),
                    "d"     => Ok(Field::Muted),
                    "b"     => Ok(Field::Blocked),
                    "c"     => Ok(Field::Created),
                    "m"     => Ok(Field::LastModified),
                    "e"     => Ok(Field::Deleted),
                    "v"     => Ok(Field::Revision),
                    "o"     => Ok(Field::Owner),
                    "pm"    => Ok(Field::Perm),
                    "nt"    => Ok(Field::Notice),
                    "ps"    => Ok(Field::HomePeerSig),
                    "s"     => Ok(Field::Signature),
                    _ => {
                        Err(de::Error::unknown_field(&key, &["id", "name", "c"]))
                    }
                }
            }
        }

        struct ChannelVisitor;

        impl<'de> Visitor<'de> for ChannelVisitor {
            type Value = Channel;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("struct Channel")
            }

            fn visit_map<V>(self, mut map: V) -> Result<Self::Value, V::Error>
            where
                V: MapAccess<'de>,
            {
                let mut owner: Option<Id> = None;
                let mut perm:  Permission = Permission::Public;
                let mut notice: Option<String> = None;

                let mut peerid: Option<Id> = None;
                let mut id: Option<Id> = None;

                while let Some(key) = map.next_key::<Field>()? {
                    match key {
                        Field::Owner    => {
                            owner = map.next_value::<serde_cbor::Value>().map(|v| {
                                Some(
                                    if let serde_cbor::Value::Bytes(b) = v {
                                        Id::from_bytes(b.try_into().unwrap())
                                    } else {
                                        panic!("Invalid type for Channel owner");
                                    }
                                )
                            })?;
                        },
                        Field::Perm     => {
                            let p: i32 = map.next_value()?;
                            perm = Permission::try_from(p).map_err(|_| de::Error::custom("Invalid permission value"))?;
                        },
                        Field::Notice   => notice = map.next_value()?,
                        Field::Id       => {
                            id = map.next_value::<serde_cbor::Value>().map(|v| {
                                Some(
                                    if let serde_cbor::Value::Bytes(b) = v {
                                        Id::from_bytes(b.try_into().unwrap())
                                    } else {
                                        panic!("Invalid type for Channel Id");
                                    }
                                )
                            })?;
                        },
                        Field::Peerid   => {
                            peerid = map.next_value::<serde_cbor::Value>().map(|v| {
                                Some(
                                    if let serde_cbor::Value::Bytes(b) = v {
                                        Id::from_bytes(b.try_into().unwrap())
                                    } else {
                                        panic!("Invalid type for Channel Home PeerId");
                                    }
                                )
                            })?;
                        },
                        _ => {
                            _ = map.next_value::<serde_cbor::Value>();
                         }
                    }
                }

                let channel_data = ChannelData::new(
                    owner.ok_or_else(|| de::Error::missing_field("o"))?,
                    perm,
                    notice
                );
                let channel = GenericContact::new(
                    id.unwrap(),
                    peerid.unwrap(),
                    channel_data
                );
                Ok(channel)
            }
        }
        deserializer.deserialize_map(ChannelVisitor)
    }
}

impl fmt::Display for Channel {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Channel: {} [...]", self.id())
        // TODO:
    }
}
