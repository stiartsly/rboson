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

use crate::{
    channel::{Channel, ChannelEditor, Permission},
    contact::{Contact, ContactOps, ContactType},
    contact_editor::ContactEditor,
    ChannelMember,
};
use boson_api::Id;

/// Channel implementation backed by a local contact and its membership list.
#[derive(Clone)]
pub(crate) struct PhotonChannel {
    pub(crate) contact: Contact,
    pub(crate) owner: Id,
    pub(crate) session_id: Id,
    pub(crate) permission: Permission,
    pub(crate) notice: Option<String>,
    pub(crate) announced: bool,
    pub(crate) members: Vec<PhotonChannelMember>,
}

/// Alias kept for call sites that distinguish wire members from public ones.
pub(crate) use super::ChannelMember as PhotonChannelMember;

impl From<&PhotonChannelMember> for ChannelMember {
    fn from(member: &PhotonChannelMember) -> Self {
        Self::new(*member.id(), member.role(), member.joined())
    }
}

impl ContactOps for PhotonChannel {
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

impl Channel for PhotonChannel {
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
        self.members.iter().map(ChannelMember::from).collect()
    }

    fn member(&self, member_id: &Id) -> Option<ChannelMember> {
        self.members
            .iter()
            .find(|member| member.id() == member_id)
            .map(ChannelMember::from)
    }

    fn edit_channel(&self) -> Box<dyn ChannelEditor> {
        Box::new(PhotonChannelEditor(self.clone()))
    }
}

/// Immutable update builder for [`PhotonChannel`].
struct PhotonChannelEditor(PhotonChannel);

impl ChannelEditor for PhotonChannelEditor {
    fn permission(mut self: Box<Self>, permission: Permission) -> Box<dyn ChannelEditor> {
        self.0.permission = permission;
        self
    }

    fn name(mut self: Box<Self>, name: String) -> Box<dyn ChannelEditor> {
        self.0.contact.set_name(Some(name));
        self
    }

    fn notice(mut self: Box<Self>, notice: Option<String>) -> Box<dyn ChannelEditor> {
        self.0.notice = notice;
        self
    }

    fn announced(mut self: Box<Self>, announced: bool) -> Box<dyn ChannelEditor> {
        self.0.announced = announced;
        self
    }

    fn build(self: Box<Self>) -> Box<dyn Channel> {
        Box::new(self.0)
    }
}
