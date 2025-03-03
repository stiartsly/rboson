use std::time::SystemTime;

use crate::{
    Id,
    Error,
    error::Result
};

use super::{
    group_permission::GroupPermission,
};

pub enum Type {
    Unknown = 0,
    Contact = 1,
    Group   = 2,
}

#[allow(dead_code)]
pub struct ContactBuilder<'a> {
    id:         &'a Id,
    type_:      Type,

    remark:     Option<&'a str>,
    tags:       Option<&'a str>,
    muted:      bool,
    blocked:    bool,
    created:    SystemTime,
    last_modified:  SystemTime,

    name:       Option<&'a str>,
    avatar:     Option<&'a str>,
    notice:     Option<&'a str>,
    // privateKey,
    owner:      Option<&'a Id>,
    permission: Option<GroupPermission>,
}

#[allow(dead_code)]
impl<'a> ContactBuilder<'a> {
    pub fn new(id: &'a Id, contact_type: Type) -> Self {
        Self {
            id,
            type_:      contact_type,

            remark:     None,
            tags:       None,
            muted:      false,
            blocked:    false,
            created:        SystemTime::UNIX_EPOCH,
            last_modified:  SystemTime::UNIX_EPOCH,

            name:       None,
            avatar:     None,
            notice:     None,

            owner:      None,
            permission: None
        }
    }

    pub fn with_remark(&mut self, remark: Option<&'a str>) -> &mut Self {
        self.remark = remark.filter(|v| !v.is_empty());
        self
    }

    pub fn with_tags(&mut self, tags: Option<&'a str>) -> &mut Self {
        self.tags = tags.filter(|v| !v.is_empty());
        self
    }

    pub fn with_muted(&mut self, muted: bool) -> &mut Self {
        self.muted = muted;
        self
    }

    pub fn with_blocked(&mut self, blocked: bool) -> &mut Self {
        self.blocked = blocked;
        self
    }

    pub fn with_created(&mut self, created: SystemTime) -> &mut Self {
        self.created = created;
        self
    }

    pub fn with_last_modified(&mut self, modified: SystemTime) -> &mut Self {
        self.last_modified = modified;
        self
    }

    pub fn with_name(&mut self, name: Option<&'a str>) -> &mut Self {
        self.name = name.filter(|v|!v.is_empty());
        self
    }

    pub fn with_avatar(&mut self, avatar: Option<&'a str>) -> &mut Self {
        self.avatar = avatar.filter(|v| !v.is_empty());
        self
    }

    pub fn with_notice(&mut self, notice: Option<&'a str>) -> &mut Self {
        self.notice = notice.filter(|v| !v.is_empty());
        self
    }

    // pub fn with_private_key(&mut self, privatekey: XXX)

    pub fn with_owner(&mut self, owner: &'a Id) -> &mut Self {
        self.owner = Some(owner);
        self
    }

    pub fn with_permission(&mut self, permission: GroupPermission) -> &mut Self {
        self.permission = Some(permission);
        self
    }

    pub fn build(&self) -> Result<Contact> {
        match self.type_ {
            Type::Unknown => return Err(Error::Argument(format!("Invalid contact type"))),
            Type::Contact => Ok(Contact::new(self)),
            Type::Group => Ok(Contact::new_group(self))
        }
    }
}

#[allow(dead_code)]
pub struct Contact {
    id:             Id,
    remark:         Option<String>,
    tags:           Option<String>,
    muted:          bool,
    blocked:        bool,
    created:        SystemTime,
    last_modified:  SystemTime,

    name:           Option<String>,
    avatar:         Option<String>,

    display_name:   Option<String>,
}

impl Contact {
    pub(crate) fn new(b: &ContactBuilder) -> Self {
        Self {
            id:         b.id.clone(),
            remark:     b.remark.map(|v| v.to_string()),
            tags:       b.tags.map(|v| v.to_string()),
            muted:      b.muted,
            blocked:    b.blocked,
            created:    b.created.clone(),
            last_modified:  b.last_modified.clone(),

            name:       None,
            avatar:     None,

            display_name:   None,
        }
    }

    pub(crate) fn new_group(b: &ContactBuilder) -> Self {
        Self {
            id:         b.id.clone(),
            remark:     b.remark.map(|v| v.to_string()),
            tags:       b.tags.map(|v| v.to_string()),
            muted:      b.muted,
            blocked:    b.blocked,
            created:    b.created.clone(),
            last_modified:  b.last_modified.clone(),

            name:       b.name.map(|v| v.to_string()),
            avatar:     b.avatar.map(|v| v.to_string()),

            display_name:   None,
        }
    }

    pub fn id(&self) -> &Id {
        &self.id
    }

    pub fn name(&self) -> Option<&str> {
        self.name.as_ref().map(|v| v.as_str())
    }

    pub fn set_name(&mut self, name: String) {
        if name.is_empty() {
            return;
        }

        self.name = Some(name);
        self.display_name = None;
    }

    pub fn avatar(&self) ->Option<&str> {
        self.avatar.as_ref().map(|v| v.as_str())
    }

    pub fn set_avatar(&mut self, avatar: String) {
        if avatar.is_empty() {
            return;
        }

        self.avatar = Some(avatar);
    }

    pub fn remark(&self) -> Option<&str> {
        self.remark.as_ref().map(|v| v.as_str())
    }

    pub fn set_remark(&mut self, remark: String) {
        if remark.is_empty() {
            return;
        }

        self.remark = Some(remark);
    }

    pub fn tags(&self) -> Option<&str> {
        self.tags.as_ref().map(|v| v.as_str())
    }

    pub fn set_tags(&mut self, tags: String) {
        if tags.is_empty() {
            return;
        }

        self.tags = Some(tags);
    }

    pub fn is_muted(&self) -> bool {
        self.muted
    }

    pub fn set_muted(&mut self, muted: bool) {
        self.muted = muted;
    }

    pub fn is_blocked(&self) -> bool {
        self.blocked
    }

    pub fn set_blocked(&mut self, blocked: bool) {
        self.blocked = blocked;
    }

    pub fn created(&self) -> SystemTime {
        self.created.clone()
    }

    pub fn last_modified(&self) -> SystemTime {
        self.last_modified.clone()
    }

    pub fn touch(&mut self) {
        self.last_modified = SystemTime::now()
    }

    pub fn display_name(&self) -> String {
        if let Some(remark) = self.remark.as_ref() {
            return remark.to_string()
        }

        if let Some(name) = self.name.as_ref() {
            return name.to_string()
        }

        unimplemented!()
    }
}

impl PartialEq for Contact {
    fn eq(&self, _other: &Self) -> bool {
        unimplemented!()
    }
}
