use std::time::{SystemTime, UNIX_EPOCH};
use crate::Id;
use crate::messaging::contact::{
    Contact,
    ContactEditor,
    ContactType
};

#[derive(Debug, Clone)]
pub struct PhotonContact {
    pub id: Id,
    pub contact_type: ContactType,
    pub name: Option<String>,
    pub remark: Option<String>,
    pub tags: Option<String>,
    pub muted: bool,
    pub blocked: bool,
    pub created_at: i64,
    pub updated_at: i64,
    pub revision: i32,
}

impl Contact for PhotonContact {
    fn id(&self) -> &Id {
        &self.id
    }

    fn contact_type(&self) -> ContactType {
        self.contact_type
    }

    fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    fn remark(&self) -> Option<&str> {
        self.remark.as_deref()
    }

    fn tags(&self) -> Option<&str> {
        self.tags.as_deref()
    }

    fn is_muted(&self) -> bool {
        self.muted
    }

    fn is_blocked(&self) -> bool {
        self.blocked
    }

    fn created_at(&self) -> i64 {
        self.created_at
    }

    fn updated_at(&self) -> i64 {
        self.updated_at
    }

    fn revision(&self) -> i32 {
        self.revision
    }

    fn edit(&self) -> Box<dyn ContactEditor> {
        Box::new(PhotonContactEditor {
            contact: self.clone(),
        })
    }
}

/// Builder for updating a [`PhotonContact`].
pub struct PhotonContactEditor {
    contact: PhotonContact,
}

impl ContactEditor for PhotonContactEditor {
    fn remark(mut self: Box<Self>, remark: Option<String>) -> Box<dyn ContactEditor> {
        self.contact.remark = remark;
        self
    }

    fn tags(mut self: Box<Self>, tags: Option<String>) -> Box<dyn ContactEditor> {
        self.contact.tags = tags;
        self
    }

    fn muted(mut self: Box<Self>, muted: bool) -> Box<dyn ContactEditor> {
        self.contact.muted = muted;
        self
    }

    fn blocked(mut self: Box<Self>, blocked: bool) -> Box<dyn ContactEditor> {
        self.contact.blocked = blocked;
        self
    }

    fn build(mut self: Box<Self>) -> Box<dyn Contact> {
        self.contact.updated_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0);
        self.contact.revision += 1;
        Box::new(self.contact)
    }
}