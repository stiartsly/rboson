use std::{fmt, time::SystemTime};
use serde::{Deserialize, Serialize};
use crate::Id;
use crate::{Result, errors::ArgumentError};
use crate::messaging::contact::ContactOps;
use crate::messaging::channel_member::ChannelMember;


/// Controls who may invite new members to a channel.
#[derive(Serialize, Deserialize)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(i32)]
pub enum Permission {
    /// Anyone may join; invitations not required.
    Public = 0,
    /// Any existing member may invite.
    MemberInvite = 1,
    /// Only moderators or the owner may invite.
    ModeratorInvite = 2,
    /// Only the channel owner may invite.
    OwnerInvite = 3,
}

impl TryFrom<i32> for Permission {
    type Error = crate::Error;

    fn try_from(value: i32) -> Result<Self> {
        match value {
            0 => Ok(Permission::Public),
            1 => Ok(Permission::MemberInvite),
            2 => Ok(Permission::ModeratorInvite),
            3 => Ok(Permission::OwnerInvite),
            _ => Err(ArgumentError::new("Invalid Permission value")),
        }
    }
}

impl From<Permission> for i32 {
    fn from(p: Permission) -> i32 {
        p as i32
    }
}

impl fmt::Display for Permission {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Permission::Public => "Public",
            Permission::MemberInvite => "MemberInvite",
            Permission::ModeratorInvite => "ModeratorInvite",
            Permission::OwnerInvite => "OwnerInvite",
        })
    }
}

/// Role of a member within a channel.
#[derive(Serialize, Deserialize)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(i32)]
pub enum Role {
    /// The channel creator / owner.
    Owner = 0,
    /// A moderator with elevated privileges.
    Moderator = 1,
    /// A regular channel member.
    Member = 2,
    /// A banned member who may not participate.
    Banned = -1,
}

impl Role {
    /// Returns `true` when this role is `Banned`.
    pub fn is_banned(&self) -> bool {
        return matches!(self, Role::Banned)
    }
}

impl TryFrom<i32> for Role {
    type Error = crate::Error;

    fn try_from(value: i32) -> Result<Self> {
        match value {
            0 => Ok(Role::Owner),
            1 => Ok(Role::Moderator),
            2 => Ok(Role::Member),
            -1 => Ok(Role::Banned),
            _ => Err(ArgumentError::new("Invalid Role value")),
        }
    }
}

impl TryFrom<&str> for Role {
    type Error = crate::Error;

    fn try_from(value: &str) -> Result<Self> {
        match value {
            "Owner" => Ok(Role::Owner),
            "Moderator" => Ok(Role::Moderator),
            "Member" => Ok(Role::Member),
            "Banned" => Ok(Role::Banned),
            _ => Err(ArgumentError::new("Invalid Role value")),
        }
    }
}

impl From<Role> for i32 {
    fn from(r: Role) -> i32 {
        r as i32
    }
}

impl fmt::Display for Role {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Role::Owner => "Owner",
            Role::Moderator => "Moderator",
            Role::Member => "Member",
            Role::Banned => "Banned",
        })
    }
}

/// A channel contact – a group conversation on the boson network.
///
/// Extends [`ContactOps`] with channel-specific information.
pub trait Channel: ContactOps {
    /// The channel's join / invite permission policy.
    fn permission(&self) -> Permission;

    /// An optional notice for the channel.
    fn notice(&self) -> Option<&str>;

    /// Whether the channel is announced to the network.
    fn is_announced(&self) -> bool;

    /// Refresh members from the service.
    //fn load_members(&self) -> impl Future<Output = Result<()>>;

    /// Total number of tracked members, including banned members.
    fn size(&self) -> usize;

    /// Number of banned members.
    fn banned(&self) -> usize;

    /// Members ordered by join time, with unbanned members first.
    fn members(&self) -> Vec<ChannelMember>;

    /// Look up a member by ID.
    fn member(&self, member_id: &Id) -> Option<ChannelMember>;

    fn has_member(&self, member_id: &Id) -> bool {
        self.member(member_id).is_some()
    }

    /// Creates an editor that builds a new channel without mutating this one.
    fn edit_channel(&self) -> Box<dyn ChannelEditor>;
}

/// Builder for an immutable channel update.
pub trait ChannelEditor: Send {
    /// Update the channel permission.
    fn permission(self: Box<Self>, permission: Permission) -> Box<dyn ChannelEditor>;

    /// Update the channel's display name.
    fn name(self: Box<Self>, name: String) -> Box<dyn ChannelEditor>;

    /// Update the channel notice.
    fn notice(self: Box<Self>, notice: Option<String>) -> Box<dyn ChannelEditor>;

    /// Update whether the channel is announced.
    fn announced(self: Box<Self>, announced: bool) -> Box<dyn ChannelEditor>;

    /// Build the updated immutable channel.
    fn build(self: Box<Self>) -> Box<dyn Channel>;
}
