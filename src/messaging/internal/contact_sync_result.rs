
use serde::{Deserialize, Serialize};

use crate::messaging::Contact;
use super::contact_sequence::ContactSequence;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ContactSyncResult {
    #[serde(rename = "s")]
    #[serde(skip_serializing_if = "Option::is_none")]
    sequence: Option<ContactSequence>,

    #[serde(rename = "c")]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    contacts: Vec<Contact>,
}

#[allow(unused)]
impl ContactSyncResult {
    pub(crate) fn last_sequence(&self) -> Option<&ContactSequence> {
        self.sequence.as_ref()
    }

    pub(crate) fn contacts(&self) -> &[Contact] {
        &self.contacts
    }
}
