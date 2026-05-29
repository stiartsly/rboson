use std::fmt;
use std::result;
use crate::Id;
use crate::messaging::contact::Contact;

/// Controls who may invite new members to a channel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(i32)]
pub enum Permission {
    /// Anyone may join; invitations not required.
    Public          = 0,
    /// Any existing member may invite.
    MemberInvite    = 1,
    /// Only moderators or the owner may invite.
    ModeratorInvite = 2,
    /// Only the channel owner may invite.
    OwnerInvite     = 3,
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
    fn from(p: Permission) -> i32 { p as i32 }
}

impl fmt::Display for Permission {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Permission::Public          => "Public",
            Permission::MemberInvite    => "MemberInvite",
            Permission::ModeratorInvite => "ModeratorInvite",
            Permission::OwnerInvite     => "OwnerInvite",
        })
    }
}

/// Role of a member within a channel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(i32)]
pub enum Role {
    /// The channel creator / owner.
    Owner     = 0,
    /// A moderator with elevated privileges.
    Moderator = 1,
    /// A regular channel member.
    Member    = 2,
    /// A banned member who may not participate.
    Banned    = -1,
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
            0  => Ok(Role::Owner),
            1  => Ok(Role::Moderator),
            2  => Ok(Role::Member),
            -1 => Ok(Role::Banned),
            _  => Err("Invalid Role value"),
        }
    }
}

impl From<Role> for i32 {
    fn from(r: Role) -> i32 { r as i32 }
}

impl fmt::Display for Role {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Role::Owner     => "Owner",
            Role::Moderator => "Moderator",
            Role::Member    => "Member",
            Role::Banned    => "Banned",
        })
    }
}

/// A single member of a channel, combining identity and role.
pub trait ChannelMember: Send + Sync {
    /// The member's boson `Id`.
    fn id(&self) -> &Id;

    /// The member's role within the channel.
    fn role(&self) -> Role;

    // --- convenience helpers ---

    fn is_owner(&self) -> bool      { matches!(self.role(), Role::Owner) }
    fn is_moderator(&self) -> bool  { matches!(self.role(), Role::Moderator) }
    fn is_member(&self) -> bool     { matches!(self.role(), Role::Member) }
    fn is_banned(&self) -> bool     { self.role().is_banned() }
}

/// A channel contact – a group conversation on the boson network.
///
/// Extends [`Contact`] with channel-specific information.
pub trait Channel: Contact {
    /// The channel's join / invite permission policy.
    fn permission(&self) -> Permission;

    /// The channel's display name.
    fn channel_name(&self) -> Option<&str>;

    /// An optional announcement / notice for the channel.
    fn notice(&self) -> Option<&str>;

    /// An optional announcement text set by the owner.
    fn announcement(&self) -> Option<&str>;

    /// The `Id` of the channel owner.
    fn owner(&self) -> &Id;

    /// The session `Id` used for encryption within this channel.
    fn session_id(&self) -> Option<&Id>;

    /// The current member count, if known.
    fn member_count(&self) -> Option<usize>;
}

/// Mutable editing operations on a [`Channel`].
pub trait ChannelEditor {
    /// Update the channel's display name.
    fn set_name(&mut self, name: Option<String>);

    /// Update the channel notice.
    fn set_notice(&mut self, notice: Option<String>);

    /// Update the channel announcement.
    fn set_announcement(&mut self, announcement: Option<String>);
}
