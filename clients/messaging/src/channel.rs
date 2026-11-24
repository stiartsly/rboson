// Copyright (c) 2023 -      bosonnetwork.io
//
// Permission is hereby granted, free of charge, to any person obtaining a copy
// of this software and associated documentation files (the "Software"), to deal
// in the Software without restriction, including without limitation the rights
// to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
// copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions:
//
// The above copyright notice and this permission notice shall be included in all
// copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
// OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
// SOFTWARE.

use std::fmt;

use boson_api::{
    Result,
    errors::ArgumentError,
};
use std::time::SystemTime;
use boson_api::Id;
use crate::ContactHandler;

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

impl Permission {
    pub fn from_i32(value: i32) -> Self {
        Self::try_from(value).unwrap_or(Self::Public)
    }
}

impl TryFrom<i32> for Permission {
    type Error = boson_api::Error;

    fn try_from(value: i32) -> Result<Self> {
        match value {
            0 => Ok(Permission::Public),
            1 => Ok(Permission::MemberInvite),
            2 => Ok(Permission::ModeratorInvite),
            3 => Ok(Permission::OwnerInvite),
            _ => Err(ArgumentError::new(&format!("Invalid Permission {}", value))),
        }
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
    pub fn from_i32(value: i32) -> Self {
        Self::try_from(value).unwrap_or(Self::Member)
    }

    pub fn is_banned(&self) -> bool {
        return matches!(self, Role::Banned)
    }
}

impl TryFrom<i32> for Role {
    type Error = boson_api::Error;

    fn try_from(value: i32) -> Result<Self> {
        match value {
            0 => Ok(Role::Owner),
            1 => Ok(Role::Moderator),
            2 => Ok(Role::Member),
            -1 => Ok(Role::Banned),
            _ => Err(ArgumentError::new(&format!("Invalid Role {}", value))),
        }
    }
}

impl TryFrom<&str> for Role {
    type Error = boson_api::Error;

    fn try_from(value: &str) -> Result<Self> {
        match value {
            "Owner" => Ok(Role::Owner),
            "Moderator" => Ok(Role::Moderator),
            "Member" => Ok(Role::Member),
            "Banned" => Ok(Role::Banned),
            _ => Err(ArgumentError::new(&format!("Invalid Role {}", value))),
        }
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
/// Extends [`ContactHandler`] with channel-specific information.
pub trait ChannelHandler: ContactHandler + Sync + Send {
    /// The ID of the channel owner.
    fn owner(&self) -> &Id;

    /// The channel's join / invite permission policy.
    fn permission(&self) -> Permission;

    /// An optional notice for the channel.
    fn notice(&self) -> Option<&str>;

    /// Whether the channel is announced to the network.
    fn is_announced(&self) -> bool;

    /// Total number of tracked members, including banned members.
    fn size(&self) -> usize;

    /// Number of banned members.
    fn banned(&self) -> usize;

    /// Members ordered by join time, with unbanned members first.
    fn members(&self) -> Vec<Box<dyn ChannelMemberHandler>>;

    /// Look up a member by ID.
    fn member(&self, member_id: &Id) -> Option<Box<dyn ChannelMemberHandler>>;

    fn has_member(&self, member_id: &Id) -> bool {
        self.member(member_id).is_some()
    }

    /// Creates an editor that builds a new channel without mutating this one.
    fn edit_channel(&self) -> Box<dyn ChannelEditorHandler>;

    /// Clones this channel into a new boxed trait object.
    fn clone_box(&self) -> Box<dyn ChannelHandler>;
/*
    fn owner(&self) -> &Id;
    fn contact_box(&self) -> Box<dyn crate::ContactHandler>;
    fn set_contact_revision(&mut self, revision: i32);
    fn set_owner(&mut self, owner: Id);
    fn set_permission(&mut self, permission: Permission);
    fn set_notice(&mut self, notice: Option<String>);
    fn set_announced(&mut self, announce: bool);
    fn set_name(&mut self, name: Option<String>);
    fn set_updated_at(&mut self, updated_at: i64);
    fn upsert_member(&mut self, id: Id, role: Role, joined: SystemTime);
    fn update_member_roles(&mut self, ids: &[Id], role: Role) -> Vec<Box<dyn ChannelMemberHandler>>;
    fn remove_members(&mut self, ids: &[Id]) -> Vec<Box<dyn ChannelMemberHandler>>;
*/
}

impl Clone for Box<dyn ChannelHandler> {
    fn clone(&self) -> Self {
        ChannelHandler::clone_box(&**self)
    }
}

pub trait ChannelMemberHandler: Send + Sync {
    /// The ID of the channel member.
    fn id(&self) -> &Id;

    /// The role of the channel member.
    fn role(&self) -> Role;

    /// Whether the member is the channel owner.
    fn is_owner(&self) -> bool {
        self.role() == Role::Owner
    }

    /// Whether the member is a channel moderator.
    fn is_moderator(&self) -> bool {
        self.role() == Role::Moderator
    }

    /// Whether the member is banned from the channel.
    fn is_banned(&self) -> bool {
        self.role() == Role::Banned
    }

    /// The time when the member joined the channel.
    fn joined(&self) -> SystemTime;

    /// Clones this member into a new boxed trait object.
    fn clone_box(&self) -> Box<dyn ChannelMemberHandler>;
}

impl Clone for Box<dyn ChannelMemberHandler> {
    fn clone(&self) -> Self {
        self.clone_box()
    }
}

/// Builder for an immutable channel update.
pub trait ChannelEditorHandler: Send + Sync {
    /// Update the channel permission.
    fn set_permission(self: Box<Self>, permission: Permission) -> Box<dyn ChannelEditorHandler>;

    /// Update the channel's display name.
    fn set_name(self: Box<Self>, name: String) -> Box<dyn ChannelEditorHandler>;

    /// Update the channel notice.
    fn set_notice(self: Box<Self>, notice: Option<String>) -> Box<dyn ChannelEditorHandler>;

    /// Update whether the channel is announced.
    fn set_announce(self: Box<Self>, announce: bool) -> Box<dyn ChannelEditorHandler>;

    fn set_remark(self: Box<Self>, remark: Option<String>) -> Box<dyn ChannelEditorHandler>;

    /// Set the user-defined tags.
    fn set_tags(self: Box<Self>, tags: Option<String>) -> Box<dyn ChannelEditorHandler>;

    /// Toggle the muted state.
    fn set_muted(self: Box<Self>, muted: bool) -> Box<dyn ChannelEditorHandler>;

    /// Toggle the blocked state.
    fn set_blocked(self: Box<Self>, blocked: bool) -> Box<dyn ChannelEditorHandler>;

    fn set_revision(self: Box<Self>, revision: i32) -> Box<dyn ChannelEditorHandler>;
    /// Build the updated immutable channel.
    fn build(self: Box<Self>) -> Box<dyn ChannelHandler>;
}

pub type Channel = Box<dyn ChannelHandler>;
pub type ChannelMember = Box<dyn ChannelMemberHandler>;
pub type ChannelEditor = Box<dyn ChannelEditorHandler>;
