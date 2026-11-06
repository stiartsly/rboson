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
use crate::{
    ChannelOps,
    ChannelEditorOps,
    ChannelMemberOps,
};

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

pub type Channel = Box<dyn ChannelOps>;
pub type ChannelMember = Box<dyn ChannelMemberOps>;
pub type ChannelEditor = Box<dyn ChannelEditorOps>;
