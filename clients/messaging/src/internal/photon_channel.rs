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

use std::{
    collections::HashMap,
    sync::Arc,
    time::SystemTime,
};

use serde::{Serialize, Deserialize};

use boson_api::{
    utils,
    Id,
    Result,
    errors::StateError,
};
use crate::{
    channel::{Permission, Role},
    contact::ContactType,
    internal::OpaqueContact,
    ChannelMember,
};
use crate::{
    ContactHandler,
    ContactEditorHandler,
    ChannelMemberHandler,
    ChannelHandler,
    ChannelEditorHandler
};

use super::{
    PhotonContact,
    PhotonChannelMember,
    ChannelInfo,
    ChannelInfoUpdate,
};

/// Channel implementation backed by a local contact and its membership list.
#[derive(Clone, Serialize, Deserialize)]
pub struct PhotonChannel {
    #[serde(rename = "o", with = "utils::id")]
    owner: Id,

    #[serde(rename = "p")]
    permission: i32,

    #[serde(
        rename = "nt",
        default,
        skip_serializing_if = "utils::is_default"
    )]
    notice: Option<String>,

    #[serde(
        rename = "a",
        default,
        skip_serializing_if = "utils::is_default"
    )]
    announce: bool,


    #[serde(flatten)]
    contact: PhotonContact,

    #[serde(skip)]
    members: HashMap<Id, PhotonChannelMember>,
    #[serde(skip)]
    members_loader: Option<Arc<dyn Fn() -> Vec<PhotonChannelMember> + Send + Sync>> ,
    #[serde(skip)]
    members_epoch: u64,
}

impl PhotonChannel {
    pub(crate) fn photon_contact(&self) -> PhotonContact {
        self.contact.clone()
    }

    pub(crate) fn photon_members(&self) -> Vec<PhotonChannelMember> {
        self.members.values().cloned().collect()
    }

    pub(crate) fn new(
        contact: PhotonContact,
        owner: Id,
        permission: Permission,
        notice: Option<String>,
        announce: bool,
    ) -> Self {
        Self {
            owner,
            permission: permission as i32,
            notice,
            announce,
            contact,
            members: HashMap::new(),
            members_loader: None,
            members_epoch: 0,
        }
    }

    pub(crate) fn from(info: &ChannelInfo, session_key: Option<Vec<u8>>) -> Self {
        let mut members = HashMap::new();
        for member in info.members() {
            members.insert(*member.id(), member.clone());
        }
        Self {
            contact     : PhotonContact::from(info, session_key),
            owner       : *info.owner_id(),
            permission  : info.permission() as i32,
            notice      : info.notice().map(ToString::to_string),
            announce    : info.announce(),
            members,
            members_loader: None,
            members_epoch: 0,
        }
    }

    pub(crate) fn contact(&self) -> &PhotonContact {
        &self.contact
    }

    pub(crate) fn has_session_key(&self) -> bool {
        self.contact.session_key().is_some()
    }

    /// The channel's session key, self-encrypted for this user.
    ///
    /// A channel invariantly carries a session key once it has been created or
    /// joined; unlike [`PhotonContact::session_key`], this panics instead of
    /// returning `None` if it is missing, mirroring the Java port's
    /// `PhotonChannel.getSessionKey()`.
    pub(crate) fn session_key(&self) -> &[u8] {
        self.contact
            .session_key()
            .expect("INTERNAL ERROR: channel sessionKey not set")
    }

    pub(crate) fn try_load_members(&mut self) -> Result<()> {
        // TODO:
        // Try loading members by using loader, which is trying load channel members from database.
        Ok(())
    }

    pub(crate) fn set_channel_members(&mut self, members: Vec<PhotonChannelMember>) -> Result<()> {
        let mut members_map = HashMap::new();
        for member in members {
            members_map.insert(*member.id(), member);
        }

        if !members_map.contains_key(&self.owner) {
            return Err(StateError::new("Owner is not present in the members list"));
        }

        self.members = members_map;
        Ok(())
    }

    pub(crate) fn upsert_member(&mut self, id: Id, role: Role, joined: SystemTime) {
        let joined = boson_api::as_ms!(joined) as u64;
        self.members.insert(id, PhotonChannelMember::new(id, role, joined));
    }

    pub(crate) fn update_member_roles(&mut self, ids: &[Id], role: Role) -> Vec<ChannelMember> {
        let mut changed = Vec::new();
        for id in ids {
            if let Some(member) = self.members.get_mut(id) {
                let joined = boson_api::as_ms!(member.joined()) as u64;
                *member = PhotonChannelMember::new(*id, role, joined);
                changed.push(Box::new(member.clone()) as ChannelMember);
            }
        }
        changed
    }

    pub(crate) fn remove_members(&mut self, ids: &[Id]) -> Vec<ChannelMember> {
        let mut removed = Vec::new();
        for id in ids {
            if let Some(member) = self.members.remove(id) {
                removed.push(Box::new(member) as ChannelMember);
            }
        }
        removed
    }

    pub(crate) fn invalidate_members(&mut self) {
        self.members.clear();
        self.members_epoch += 1;
    }

    pub(crate) fn to_opaque(&self, data: Vec<u8>) -> OpaqueContact {
        self.contact.to_opaque(data)
    }

    pub(crate) fn serialize(&self) -> Vec<u8> {
        serde_cbor::to_vec(self).expect("Serializing PhotonChannel error")
    }

    pub(crate) fn edit_channel(&self) -> PhotonChannelEditor {
        PhotonChannelEditor(self.clone())
    }
}

impl Eq for PhotonChannel {}
impl PartialEq for PhotonChannel {
    fn eq(&self, other: &Self) -> bool {
        self.owner == other.owner &&
        self.permission == other.permission &&
        self.notice == other.notice &&
        self.announce == other.announce &&
        self.members == other.members &&
        self.contact == other.contact
    }
}

impl From<PhotonChannel> for Box<dyn ChannelHandler> {
    fn from(channel: PhotonChannel) -> Self {
        Box::new(channel)
    }
}

impl From<Box<PhotonChannel>> for Box<dyn ChannelHandler> {
    fn from(channel: Box<PhotonChannel>) -> Self {
        channel
    }
}

impl AsRef<dyn ChannelHandler> for PhotonChannel {
    fn as_ref(&self) -> &(dyn ChannelHandler + 'static) {
        self
    }
}

impl TryFrom<&crate::channel::Channel> for PhotonChannel {
    type Error = boson_api::Error;

    fn try_from(channel: &crate::channel::Channel) -> Result<Self> {
        let contact = PhotonContact::from_channel(channel.as_ref());
        let mut result = Self::new(
            contact,
            *channel.owner(),
            channel.permission(),
            channel.notice().map(ToString::to_string),
            channel.is_announced(),
        );
        let members = channel
            .members()
            .iter()
            .map(|member| PhotonChannelMember::new(
                *member.id(),
                member.role(),
                boson_api::as_ms!(member.joined()) as u64,
            ))
            .collect();
        result.set_channel_members(members)?;
        Ok(result)
    }
}

impl ContactHandler for PhotonChannel {
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

    fn session_key(&self) -> Option<&[u8]> {
        self.contact.session_key()
    }

    fn edit(&self) -> Box<dyn ContactEditorHandler> {
        self.contact.edit()
    }

    fn clone_box(&self) -> Box<dyn ContactHandler> {
        Box::new(self.clone())
    }

    fn set_revision(&mut self, revision: i32) {
        self.contact.set_revision(revision);
    }
}

impl ChannelHandler for PhotonChannel {
    fn owner(&self) -> &Id {
        &self.owner
    }

    fn permission(&self) -> Permission {
        Permission::from_i32(self.permission)
    }

    fn notice(&self) -> Option<&str> {
        self.notice.as_deref()
    }

    fn is_announced(&self) -> bool {
        self.announce
    }

    fn size(&self) -> usize {
        self.members.len()
    }

    fn banned(&self) -> usize {
        self.members.values().filter(|member|
            member.role() == crate::channel::Role::Banned
        ).count()
    }

    fn members(&self) -> Vec<Box<dyn ChannelMemberHandler>> {
        self.members.values().cloned().map(|m|
            Box::new(m) as Box<dyn ChannelMemberHandler>
        ).collect()
    }

    fn member(&self, member_id: &Id) -> Option<Box<dyn ChannelMemberHandler>> {
        self.members
            .get(member_id)
            .cloned()
            .map(|m| Box::new(m) as Box<dyn ChannelMemberHandler>)
    }

    fn edit_channel(&self) -> Box<dyn ChannelEditorHandler> {
        Box::new(PhotonChannelEditor(self.clone())) as Box<dyn ChannelEditorHandler>
    }

    fn clone_box(&self) -> Box<dyn ChannelHandler> {
        Box::new(self.clone())
    }
}

pub struct PhotonChannelEditor(PhotonChannel);

impl ChannelEditorHandler for PhotonChannelEditor {
    fn set_permission(mut self: Box<Self>, permission: Permission) -> Box<dyn ChannelEditorHandler> {
        self.0.permission = permission as i32;
        self
    }

    fn set_name(mut self: Box<Self>, name: String) -> Box<dyn ChannelEditorHandler> {
        self.0.contact.set_name(Some(name));
        self
    }

    fn set_notice(mut self: Box<Self>, notice: Option<String>) -> Box<dyn ChannelEditorHandler> {
        self.0.notice = notice;
        self
    }

    fn set_announce(mut self: Box<Self>, announce: bool) -> Box<dyn ChannelEditorHandler> {
        self.0.announce = announce;
        self
    }

    fn set_remark(mut self: Box<Self>, remark: Option<String>) -> Box<dyn ChannelEditorHandler> {
        self.0.contact.set_remark(remark);
        self
    }

    /// Set the user-defined tags.
    fn set_tags(mut self: Box<Self>, tags: Option<String>) -> Box<dyn ChannelEditorHandler> {
        self.0.contact.set_tags(tags);
        self
    }

    /// Toggle the muted state.
    fn set_muted(mut self: Box<Self>, muted: bool) -> Box<dyn ChannelEditorHandler> {
        self.0.contact.set_muted(muted);
        self
    }

    /// Toggle the blocked state.
    fn set_blocked(mut self: Box<Self>, blocked: bool) -> Box<dyn ChannelEditorHandler> {
        self.0.contact.set_blocked(blocked);
        self
    }

    fn set_revision(mut self: Box<Self>, revision: i32) -> Box<dyn ChannelEditorHandler> {
        self.0.set_revision(revision);
        self
    }

    /// Build the updated immutable channel.
    fn build(self: Box<Self>) -> Box<dyn ChannelHandler> {
        Box::new(self.0) as Box<dyn ChannelHandler>
    }
}

impl PhotonChannelEditor {
    pub(crate) fn set_owner(mut self, owner: Id) -> Self {
        self.0.owner = owner;
        self
    }

    pub(crate) fn set_permission(mut self, permission: Permission) -> Self {
        self.0.permission = permission as i32;
        self
    }

    pub(crate) fn set_name(mut self, name: Option<String>) -> Self {
        self.0.contact.set_name(name);
        self
    }

    pub(crate) fn set_notice(mut self, notice: Option<String>) -> Self {
        self.0.notice = notice;
        self
    }

    pub(crate) fn set_announce(mut self, announce: bool) -> Self {
        self.0.announce = announce;
        self
    }

    pub(crate) fn set_remark(mut self, remark: Option<String>) -> Self {
        self.0.contact.set_remark(remark);
        self
    }

    pub(crate) fn set_tags(mut self, tags: Option<String>) -> Self {
        self.0.contact.set_tags(tags);
        self
    }

    pub(crate) fn set_muted(mut self, muted: bool) -> Self {
        self.0.contact.set_muted(muted);
        self
    }

    pub(crate) fn set_blocked(mut self, blocked: bool) -> Self {
        self.0.contact.set_blocked(blocked);
        self
    }

    pub(crate) fn set_session_key(mut self, session_key: Vec<u8>) -> Self {
        self.0.contact.set_session_key(Some(session_key));
        self
    }

    pub(crate) fn set_revision(mut self, revision: i32) -> Self {
        self.0.set_revision(revision);
        self
    }

    pub(crate) fn patch(mut self, changes: &ChannelInfoUpdate) -> Self {
        if let Some(permission) = changes.permission() {
            self = self.set_permission(permission);
        }
        if let Some(name) = changes.name() {
            self = self.set_name(name.map(ToString::to_string));
        }
        if let Some(notice) = changes.notice() {
            self = self.set_notice(notice.map(ToString::to_string));
        }
        if let Some(announce) = changes.announce() {
            self = self.set_announce(announce);
        }
        self
    }

    pub(crate) fn build(self) -> PhotonChannel {
        self.0
    }
}
