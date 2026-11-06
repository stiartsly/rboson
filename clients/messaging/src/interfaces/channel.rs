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

use std::time::SystemTime;

use boson_api::Id;
use crate::{
    channel::{Permission, Role},
    interfaces::ContactOps,
};

/// A channel contact – a group conversation on the boson network.
///
/// Extends [`ContactOps`] with channel-specific information.
pub trait Channel: ContactOps + Sync + Send {
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
    fn members(&self) -> Vec<Box<dyn ChannelMember>>;

    /// Look up a member by ID.
    fn member(&self, member_id: &Id) -> Option<Box<dyn ChannelMember>>;

    fn has_member(&self, member_id: &Id) -> bool {
        self.member(member_id).is_some()
    }

    /// Creates an editor that builds a new channel without mutating this one.
    fn edit_channel(&self) -> Box<dyn ChannelEditor>;

    /// Clones this channel into a new boxed trait object.
    fn clone_box(&self) -> Box<dyn Channel>;

    fn owner(&self) -> Id;
    fn session_id(&self) -> Id;
    fn contact_box(&self) -> Box<dyn crate::ContactOps>;
    fn set_contact_revision(&mut self, revision: i32);
    fn set_owner(&mut self, owner: Id);
    fn set_session_id(&mut self, session_id: Id);
    fn set_permission(&mut self, permission: Permission);
    fn set_notice(&mut self, notice: Option<String>);
    fn set_announced(&mut self, announced: bool);
    fn set_name(&mut self, name: Option<String>);
    fn set_updated_at(&mut self, updated_at: i64);
    fn upsert_member(&mut self, id: Id, role: Role, joined: SystemTime);
    fn update_member_roles(&mut self, ids: &[Id], role: Role) -> Vec<Box<dyn ChannelMember>>;
    fn remove_members(&mut self, ids: &[Id]) -> Vec<Box<dyn ChannelMember>>;
}

impl Clone for Box<dyn Channel> {
    fn clone(&self) -> Self {
        Channel::clone_box(&**self)
    }
}

pub trait ChannelMember: Send + Sync {
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
    fn clone_box(&self) -> Box<dyn ChannelMember>;
}

impl Clone for Box<dyn ChannelMember> {
    fn clone(&self) -> Self {
        self.clone_box()
    }
}

/// Builder for an immutable channel update.
pub trait ChannelEditor: Send + Sync {
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
