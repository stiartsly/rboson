use serde::{Deserialize, Serialize};
use crate::{Id, errors::{ArgumentError, Error, Result}};
use crate::messaging::errors::EncodingError;
use super::OpaqueContact;

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

impl Op {
    pub(crate) fn value(self) -> i32 { self as i32 }
}

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

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct ContactMutation {
    #[serde(rename = "v")]
    revision: i32,
    #[serde(flatten)]
    data: MutationData,
}

impl ContactMutation {
    pub(crate) fn revision(&self) -> i32 { self.revision }

    pub(crate) fn op(&self) -> Op {
        match &self.data {
            MutationData::Add(_) => Op::Add,
            MutationData::Update(_) => Op::Update,
            MutationData::Remove(_) => Op::Remove,
            MutationData::Clear => Op::Clear,
        }
    }

    pub(crate) fn data(&self) -> Option<&MutationData> {
        match &self.data {
            MutationData::Clear => None,
            data => Some(data),
        }
    }

    pub(crate) fn data_as_bytes(&self) -> Result<Option<Vec<u8>>> {
        let bytes = match &self.data {
            MutationData::Add(contact) | MutationData::Update(contact) => serde_cbor::to_vec(contact),
            MutationData::Remove(ids) => serde_cbor::to_vec(ids),
            MutationData::Clear => return Ok(None),
        };
        bytes.map(Some)
            .map_err(|error| EncodingError::new(format!("Encoding contact mutation data failed: {error}")).into())
    }

    pub(crate) fn add(revision: i32, contact: OpaqueContact) -> Self {
        Self { revision, data: MutationData::Add(contact) }
    }

    pub(crate) fn update(revision: i32, contact: OpaqueContact) -> Self {
        Self { revision, data: MutationData::Update(contact) }
    }

    pub(crate) fn remove(revision: i32, ids: Vec<Id>) -> Self {
        Self { revision, data: MutationData::Remove(ids) }
    }

    pub(crate) fn clear(revision: i32) -> Self {
        Self { revision, data: MutationData::Clear }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_cbor::Value;

    #[test]
    fn mutations_roundtrip_with_java_wire_format() {
        let id = Id::random();
        let mutations = [
            ContactMutation::add(12, OpaqueContact::new(id, 13, Value::Bytes(vec![1, 2]))),
            ContactMutation::update(12, OpaqueContact::new(id, 13, Value::Bytes(vec![3, 4]))),
            ContactMutation::remove(12, vec![id]),
            ContactMutation::clear(12),
        ];
        for (mutation, tag) in mutations.iter().zip(["a", "u", "r", "c"]) {
            let bytes = serde_cbor::to_vec(mutation).unwrap();
            let Value::Map(wire) = serde_cbor::from_slice::<Value>(&bytes).unwrap() else {
                panic!("Contact mutation must be a map");
            };
            assert_eq!(wire.get(&Value::Text("v".into())), Some(&Value::Integer(12.into())));
            assert_eq!(wire.get(&Value::Text("op".into())), Some(&Value::Text(tag.into())));
            assert!(!wire.contains_key(&Value::Text("r".into())));
            if let Some(data) = mutation.data_as_bytes().unwrap() {
                assert!(mutation.data().is_some());
                assert_eq!(wire.get(&Value::Text("d".into())), Some(&serde_cbor::from_slice::<Value>(&data).unwrap()));
            } else {
                assert!(mutation.data().is_none());
                assert!(!wire.contains_key(&Value::Text("d".into())));
            }
            if tag == "r" {
                assert_eq!(wire.get(&Value::Text("d".into())), Some(&Value::Array(vec![Value::Bytes(id.as_bytes().to_vec())])));
            }
            let decoded: ContactMutation = serde_cbor::from_slice(&bytes).unwrap();
            assert_eq!(decoded.revision(), mutation.revision());
            assert_eq!(decoded.op(), mutation.op());
            assert_eq!(decoded.data_as_bytes().unwrap(), mutation.data_as_bytes().unwrap());
        }
    }

    #[test]
    fn operation_values_match_java() {
        for (value, op) in [(1, Op::Add), (2, Op::Update), (3, Op::Remove), (4, Op::Clear)] {
            assert_eq!(Op::try_from(value).unwrap(), op);
            assert_eq!(op.value(), value);
        }
        assert!(Op::try_from(0).is_err());
        assert!(Op::try_from(5).is_err());
    }
}
