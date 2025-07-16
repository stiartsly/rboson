use serde::Deserialize;
use crate::messaging::{
    Contact
};

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct ContactsUpdate {
    #[serde(rename = "v")]
    #[serde(skip_serializing_if = "crate::is_none_or_empty")]
    version_id: Option<String>,

    #[serde(rename = "c")]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    contacts: Vec<Contact>,
}

impl ContactsUpdate {
    pub(crate) fn version_id(&self) -> Option<&str> {
        self.version_id.as_deref()
    }

    pub(crate) fn contacts(&self) -> &[Contact] {
        &self.contacts
    }
}
