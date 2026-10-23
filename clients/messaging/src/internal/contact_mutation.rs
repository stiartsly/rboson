use serde::{Deserialize, Serialize};
#[cfg(test)]
use boson_api::{errors::{ArgumentError, Error, Result}};
use boson_api::Id;
use super::OpaqueContact;

#[cfg(test)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(i32)]
pub(crate) enum Op {
    #[serde(rename = "a")]
    Add = 1,
    #[serde(rename = "u")]
    Update = 2,
    #[serde(rename = "r")]
    Remove = 3,
    #[serde(rename = "c")]
    Clear = 4,
}

#[cfg(test)]
impl Op {
    pub(crate) const fn value(self) -> i32 {
        self as i32
    }
}

#[cfg(test)]
impl TryFrom<i32> for Op {
    type Error = Error;

    fn try_from(value: i32) -> Result<Self> {
        match value {
            1 => Ok(Self::Add),
            2 => Ok(Self::Update),
            3 => Ok(Self::Remove),
            4 => Ok(Self::Clear),
            _ => Err(ArgumentError::new(format!("Invalid operation: {value}"))),
        }
    }
}

/// Payload of a contact mutation, flattened into the mutation envelope.
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "op", content = "d")]
pub(crate) enum MutationData {
    #[serde(rename = "a")]
    Add(OpaqueContact),
    #[serde(rename = "u")]
    Update(OpaqueContact),
    #[serde(rename = "r")]
    Remove(Vec<Id>),
    #[serde(rename = "c")]
    Clear,
}

/// A single revisioned contact mutation.
#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct ContactMutation {
    #[serde(rename = "v")]
    revision: i32,
    #[serde(flatten)]
    data: MutationData,
}

impl ContactMutation {
    pub(crate) fn revision(&self) -> i32 {
        self.revision
    }

    pub(crate) fn data(&self) -> Option<&MutationData> {
        match &self.data {
            MutationData::Clear => None,
            data => Some(data),
        }
    }

    pub(crate) fn add(revision: i32, contact: OpaqueContact) -> Self {
        Self {
            revision,
            data: MutationData::Add(contact),
        }
    }

    pub(crate) fn update(revision: i32, contact: OpaqueContact) -> Self {
        Self {
            revision,
            data: MutationData::Update(contact),
        }
    }

    pub(crate) fn remove(revision: i32, ids: Vec<Id>) -> Self {
        Self {
            revision,
            data: MutationData::Remove(ids),
        }
    }

    pub(crate) fn clear(revision: i32) -> Self {
        Self {
            revision,
            data: MutationData::Clear,
        }
    }
}
