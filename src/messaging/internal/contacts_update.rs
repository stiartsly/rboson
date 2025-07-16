use serde::Deserialize;
use crate::messaging::{
    Contact
};

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct ContactsUpdate {
    #[serde(rename = "v")]
    #[serde(skip_serializing_if = "crate::is_none_or_empty")]
    #[allow(dead_code)]
    version_id: Option<String>,

    #[serde(rename = "c")]
    #[serde(skip_serializing_if = "crate::is_none_or_empty")]
    contacts: Option<Vec<Contact>>,
}

impl ContactsUpdate {
    pub(crate) fn version_id(&mut self) -> Option<String> {
        //self.version_id.take()
        Some("1.1".to_string())
    }

    pub(crate) fn contacts(&mut self) -> Vec<Contact> {
        self.contacts.take().unwrap_or_default()
    }
}
