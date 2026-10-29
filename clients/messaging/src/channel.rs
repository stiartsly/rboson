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
use serde::{Deserialize, Serialize};
use boson_api::Id;
use crate::{
    contact::{Contact, ContactType},
    contact_editor::ContactEditor,
    ops::{ChannelEditorOps, ChannelOps, ContactOps},
    ChannelMember,
};
use boson_api::{Result, errors::ArgumentError};

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
    type Error = boson_api::Error;

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
    type Error = boson_api::Error;

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
    type Error = boson_api::Error;

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

/// Channel implementation backed by a local contact and its membership list.
#[derive(Clone)]
pub struct Channel {
    pub(crate) contact: Contact,
    pub(crate) owner: Id,
    pub(crate) session_id: Id,
    pub(crate) permission: Permission,
    pub(crate) notice: Option<String>,
    pub(crate) announced: bool,
    pub(crate) members: Vec<ChannelMember>,
}

impl From<&ChannelMember> for ChannelMember {
    fn from(member: &ChannelMember) -> Self {
        Self::new(*member.id(), member.role(), member.joined())
    }
}

impl ContactOps for Channel {
    fn id(&self) -> &Id {
        self.contact.id()
    }

    fn contact_type(&self) -> ContactType {
        self.contact.contact_type()
    }

    fn name(&self) -> Option<&str> {
        self.contact.name()
    }

    fn remark(&self) -> Option<&str> {
        self.contact.remark()
    }

    fn tags(&self) -> Option<&str> {
        self.contact.tags()
    }

    fn is_muted(&self) -> bool {
        self.contact.is_muted()
    }

    fn is_blocked(&self) -> bool {
        self.contact.is_blocked()
    }

    fn created_at(&self) -> i64 {
        self.contact.created_at()
    }

    fn updated_at(&self) -> i64 {
        self.contact.updated_at()
    }

    fn revision(&self) -> i32 {
        self.contact.revision()
    }

    fn edit(&self) -> ContactEditor {
        self.contact.edit()
    }
}

impl ChannelOps for Channel {
    fn permission(&self) -> Permission {
        self.permission
    }

    fn notice(&self) -> Option<&str> {
        self.notice.as_deref()
    }

    fn is_announced(&self) -> bool {
        self.announced
    }

    fn size(&self) -> usize {
        self.members.len()
    }

    fn banned(&self) -> usize {
        self.members.iter().filter(|member| member.is_banned()).count()
    }

    fn members(&self) -> Vec<ChannelMember> {
        self.members.clone()
    }

    fn member(&self, member_id: &Id) -> Option<ChannelMember> {
        self.members
            .iter()
            .find(|member| member.id() == member_id)
            .cloned()
    }

    fn edit_channel(&self) -> Box<ChannelEditor> {
        Box::new(ChannelEditor(self.clone()))
    }
}

impl Channel {
    pub fn id(&self) -> &Id {
        self.contact.id()
    }

    pub fn contact_type(&self) -> ContactType {
        self.contact.contact_type()
    }

    pub fn name(&self) -> Option<&str> {
        self.contact.name()
    }

    pub fn remark(&self) -> Option<&str> {
        self.contact.remark()
    }

    pub fn tags(&self) -> Option<&str> {
        self.contact.tags()
    }

    pub fn is_muted(&self) -> bool {
        self.contact.is_muted()
    }

    pub fn is_blocked(&self) -> bool {
        self.contact.is_blocked()
    }

    pub fn created_at(&self) -> i64 {
        self.contact.created_at()
    }

    pub fn updated_at(&self) -> i64 {
        self.contact.updated_at()
    }

    pub fn revision(&self) -> i32 {
        self.contact.revision()
    }

    pub fn edit(&self) -> ContactEditor {
        self.contact.edit()
    }

    pub fn permission(&self) -> Permission {
        self.permission
    }

    pub fn notice(&self) -> Option<&str> {
        self.notice.as_deref()
    }

    pub fn is_announced(&self) -> bool {
        self.announced
    }

    pub fn size(&self) -> usize {
        self.members.len()
    }

    pub fn banned(&self) -> usize {
        self.members.iter().filter(|member| member.is_banned()).count()
    }

    pub fn members(&self) -> Vec<ChannelMember> {
        self.members.clone()
    }

    pub fn member(&self, member_id: &Id) -> Option<ChannelMember> {
        self.members
            .iter()
            .find(|member| member.id() == member_id)
            .cloned()
    }

    pub fn has_member(&self, member_id: &Id) -> bool {
        self.member(member_id).is_some()
    }

    pub fn edit_channel(&self) -> Box<ChannelEditor> {
        Box::new(ChannelEditor(self.clone()))
    }
}

/// Immutable update builder for [`Channel`].
pub struct ChannelEditor(Channel);

impl ChannelEditorOps for ChannelEditor {
    fn permission(mut self: Box<Self>, permission: Permission) -> Box<ChannelEditor> {
        self.0.permission = permission;
        self
    }

    fn name(mut self: Box<Self>, name: String) -> Box<ChannelEditor> {
        self.0.contact.set_name(Some(name));
        self
    }

    fn notice(mut self: Box<Self>, notice: Option<String>) -> Box<ChannelEditor> {
        self.0.notice = notice;
        self
    }

    fn announced(mut self: Box<Self>, announced: bool) -> Box<ChannelEditor> {
        self.0.announced = announced;
        self
    }

    fn build(self: Box<Self>) -> Box<Channel> {
        Box::new(self.0)
    }
}

impl ChannelEditor {
    pub fn permission(mut self, permission: Permission) -> Self {
        self.0.permission = permission;
        self
    }

    pub fn name(mut self, name: String) -> Self {
        self.0.contact.set_name(Some(name));
        self
    }

    pub fn notice(mut self, notice: Option<String>) -> Self {
        self.0.notice = notice;
        self
    }

    pub fn announced(mut self, announced: bool) -> Self  {
        self.0.announced = announced;
        self
    }

    pub fn build(self) -> Box<Channel> {
        Box::new(self.0)
    }
}
