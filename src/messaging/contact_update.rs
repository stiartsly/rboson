
use serde::{Deserialize, Serialize};

use crate::messaging::{
    contact::Contact
};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub(crate) struct ContactsUpdate {
    #[serde(rename = "v")]
    #[serde(skip_serializing_if = "String::is_empty")]
    version_id: String,

    #[serde(rename = "c")]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    contacts: Vec<Contact>,
}

#[allow(unused)]
impl ContactsUpdate {
    pub(crate) fn version_id(&self) -> &str {
        &self.version_id
    }

    pub(crate) fn contacts(&self) -> &[Contact] {
        &self.contacts
    }
}
