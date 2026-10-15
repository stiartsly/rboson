use std::time::SystemTime;
use crate::Id;
use crate::messaging::{
    channel::{Channel, ChannelEditor, ChannelMember, Permission, Role},
    contact::{Contact, ContactOps, ContactType},
    contact_editor::ContactEditor
};

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

#[derive(Clone, serde::Deserialize)]
pub(crate) struct PhotonChannelMember {
    pub(crate) id: Id,
    #[serde(rename = "r")]
    pub(crate) role: i32,
    #[serde(rename = "j")]
    pub(crate) joined: u64,
}

impl ChannelMember for PhotonChannelMember {
    fn id(&self) -> &Id { &self.id }
    fn role(&self) -> Role {
        match self.role {
            0 => Role::Owner,
            1 => Role::Moderator,
            2 => Role::Member,
            _ => Role::Banned,
        }
    }
    fn joined(&self) -> SystemTime {
        std::time::UNIX_EPOCH + std::time::Duration::from_millis(self.joined)
    }
}

impl ContactOps for PhotonChannel {
    fn id(&self) -> &Id { self.contact.id() }
    fn contact_type(&self) -> ContactType { self.contact.contact_type() }
    fn name(&self) -> Option<&str> { self.contact.name() }
    fn remark(&self) -> Option<&str> { self.contact.remark() }
    fn tags(&self) -> Option<&str> { self.contact.tags() }
    fn is_muted(&self) -> bool { self.contact.is_muted() }
    fn is_blocked(&self) -> bool { self.contact.is_blocked() }
    fn created_at(&self) -> i64 { self.contact.created_at() }
    fn updated_at(&self) -> i64 { self.contact.updated_at() }
    fn revision(&self) -> i32 { self.contact.revision() }
    fn edit(&self) -> ContactEditor { self.contact.edit() }
}

impl Channel for PhotonChannel {
    fn permission(&self) -> Permission { self.permission }
    fn notice(&self) -> Option<&str> { self.notice.as_deref() }
    fn is_announced(&self) -> bool { self.announced }
    fn size(&self) -> usize { self.members.len() }
    fn banned(&self) -> usize { self.members.iter().filter(|member| member.role == -1).count() }
    fn members(&self) -> Vec<&dyn ChannelMember> {
        self.members.iter().map(|member| member as &dyn ChannelMember).collect()
    }
    fn member(&self, member_id: &Id) -> Option<&dyn ChannelMember> {
        self.members.iter().find(|member| member.id == *member_id)
            .map(|member| member as &dyn ChannelMember)
    }
    fn edit_channel(&self) -> Box<dyn ChannelEditor> {
        Box::new(PhotonChannelEditor(self.clone()))
    }
}

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
    fn build(self: Box<Self>) -> Box<dyn Channel> { Box::new(self.0) }
}
