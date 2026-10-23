use serde::{Deserialize, Serialize};
use serde_repr::{Deserialize_repr, Serialize_repr};
use crate::errors::{ArgumentError, Error, Result};
use super::{ContactMutation, OpaqueContact};

#[derive(Serialize_repr, Deserialize_repr)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub(crate) enum SyncType {
    UpToDate = 0,
    Delta = 1,
    Snapshot = 2,
}

impl TryFrom<i32> for SyncType {
    type Error = Error;

    fn try_from(value: i32) -> Result<Self> {
        match value {
            0 => Ok(Self::UpToDate),
            1 => Ok(Self::Delta),
            2 => Ok(Self::Snapshot),
            _ => Err(ArgumentError::new(format!(
                "Invalid contact sync type: {value}"
            ))),
        }
    }
}

/// A validated contact sync notification or response.
#[derive(Debug, Serialize, Deserialize)]
#[serde(try_from = "ContactSyncData")]
pub(crate) struct ContactSync {
    #[serde(rename = "v")]
    revision: i32,
    #[serde(rename = "t")]
    kind: SyncType,
    #[serde(rename = "d", skip_serializing_if = "Vec::is_empty")]
    mutations: Vec<ContactMutation>,
    #[serde(rename = "s", skip_serializing_if = "Vec::is_empty")]
    contacts: Vec<OpaqueContact>,
}

/// Raw sync payload; nullable lists are accepted and normalised to empty ones.
#[derive(Deserialize)]
struct ContactSyncData {
    #[serde(rename = "v")]
    revision: i32,
    #[serde(rename = "t")]
    kind: SyncType,
    #[serde(rename = "d", default, skip_serializing_if = "crate::utils::is_default")]
    mutations: Option<Vec<ContactMutation>>,
    #[serde(rename = "s", default, skip_serializing_if = "crate::utils::is_default")]
    contacts: Option<Vec<OpaqueContact>>,
}

impl TryFrom<ContactSyncData> for ContactSync {
    type Error = Error;

    fn try_from(data: ContactSyncData) -> Result<Self> {
        Self::new(
            data.revision,
            data.kind,
            data.mutations.unwrap_or_default(),
            data.contacts.unwrap_or_default(),
        )
    }
}

impl ContactSync {
    fn new(
        revision: i32,
        kind: SyncType,
        mutations: Vec<ContactMutation>,
        contacts: Vec<OpaqueContact>,
    ) -> Result<Self> {
        if revision < 0 {
            return Err(ArgumentError::new("Contact sync revision is negative"));
        }
        if !mutations.is_empty() && !contacts.is_empty() {
            return Err(ArgumentError::new(
                "Mutations and contacts can not be both present",
            ));
        }

        match kind {
            SyncType::UpToDate if !mutations.is_empty() || !contacts.is_empty() => {
                return Err(ArgumentError::new(
                    "Up-to-date sync does not require mutations or contacts",
                ));
            }
            SyncType::Delta if mutations.is_empty() || !contacts.is_empty() => {
                return Err(ArgumentError::new(
                    "Delta sync requires mutations and no contacts",
                ));
            }
            SyncType::Snapshot if !mutations.is_empty() => {
                return Err(ArgumentError::new(
                    "Snapshot sync does not require mutations",
                ));
            }
            _ => {}
        }

        Ok(Self {
            revision,
            kind,
            mutations,
            contacts,
        })
    }

    #[cfg(test)]
    pub(crate) fn up_to_date(revision: i32) -> Result<Self> {
        Self::new(revision, SyncType::UpToDate, Vec::new(), Vec::new())
    }

    pub(crate) fn kind(&self) -> SyncType {
        self.kind
    }

    pub(crate) fn revision(&self) -> i32 {
        self.revision
    }

    pub(crate) fn mutations(&self) -> &[ContactMutation] {
        &self.mutations
    }

    pub(crate) fn contacts(&self) -> &[OpaqueContact] {
        &self.contacts
    }
}
