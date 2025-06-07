
use serde::Deserialize;
use crate::messaging::contact::Contact;

#[derive(Debug, Deserialize)]
pub(crate) struct ContactsUpdate {
    #[serde(rename = "v")]
    #[serde(skip_serializing_if = "String::is_empty")]
    version_id: Option<String>,

    #[serde(rename = "c")]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    contacts: Vec<Contact>,
}

impl ContactsUpdate {
    pub(crate) fn version_id(&self) -> Option<&str> {
        self.version_id.as_ref().map(|s| s.as_str())
    }

    pub(crate) fn contacts(&self) -> &[Contact] {
        &self.contacts
    }
}
