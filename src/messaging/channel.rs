use crate::messaging::client::BoxFuture;
use crate::messaging::contact::Contact;
use crate::messaging::errors::Result;
use crate::Id;
use std::fmt;
use std::result;
use std::time::SystemTime;

/// Controls who may invite new members to a channel.
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
    type Error = &'static str;

    fn try_from(value: i32) -> result::Result<Self, Self::Error> {
        match value {
            0 => Ok(Permission::Public),
            1 => Ok(Permission::MemberInvite),
            2 => Ok(Permission::ModeratorInvite),
            3 => Ok(Permission::OwnerInvite),
            _ => Err("Invalid Permission value"),
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
        matches!(self, Role::Banned)
    }
}

impl TryFrom<i32> for Role {
    type Error = &'static str;

    fn try_from(value: i32) -> result::Result<Self, Self::Error> {
        match value {
            0 => Ok(Role::Owner),
            1 => Ok(Role::Moderator),
            2 => Ok(Role::Member),
            -1 => Ok(Role::Banned),
            _ => Err("Invalid Role value"),
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

/// A single member of a channel, combining identity and role.
pub trait ChannelMember: Send + Sync {
    /// The member's boson `Id`.
    fn id(&self) -> &Id;

    /// The member's role within the channel.
    fn role(&self) -> Role;

    /// The time the member joined.
    fn joined(&self) -> SystemTime;

    // --- convenience helpers ---

    fn is_owner(&self) -> bool {
        matches!(self.role(), Role::Owner)
    }
    fn is_moderator(&self) -> bool {
        matches!(self.role(), Role::Moderator)
    }
    fn is_member(&self) -> bool {
        matches!(self.role(), Role::Member)
    }
    fn is_banned(&self) -> bool {
        self.role().is_banned()
    }
}

/// A channel contact – a group conversation on the boson network.
///
/// Extends [`Contact`] with channel-specific information.
pub trait Channel: Contact {
    /// The channel's join / invite permission policy.
    fn permission(&self) -> Permission;

    /// An optional notice for the channel.
    fn notice(&self) -> Option<&str>;

    /// Whether the channel is announced to the network.
    fn is_announced(&self) -> bool;

    /// Refresh members from the service.
    fn load_members(&self) -> BoxFuture<'_, Result<()>>;

    /// Total number of tracked members, including banned members.
    fn size(&self) -> usize;

    /// Number of banned members.
    fn banned(&self) -> usize;

    /// Members ordered by join time, with unbanned members first.
    fn members(&self) -> Vec<&dyn ChannelMember>;

    /// Look up a member by ID.
    fn member(&self, member_id: &Id) -> Option<&dyn ChannelMember>;

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
