use std::time::{SystemTime, UNIX_EPOCH};
use crate::messaging::contact::{
    Contact,
    ContactEditor,
};
use super::photon_contact::PhotonContact;

/// Builder for updating a [`PhotonContact`].
pub struct PhotonContactEditor {
    pub(crate) contact: PhotonContact,
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