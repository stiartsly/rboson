use std::fmt;
use std::result::Result as SResult;
use serde::{
    Deserialize, Serialize,
    de::{self, Deserializer, MapAccess, Visitor, IgnoredAny},
    ser::{SerializeMap, Serializer}
};

use crate::Id;
use super::lookup_req::{
    LookupRequest,
    Data as LookupData
};

pub(crate) struct FindValueRequest {
    data: LookupData,
    expected_seq: i32,
}

impl FindValueRequest {
    pub(crate) fn new(
        target: Id,
        want4: bool,
        want6: bool,
        expected_seq: i32,
    ) -> Self {
        Self {
            data: LookupData::new(target, want4, want6, false),
            expected_seq,
        }
    }

    pub(crate) fn expected_seq(&self) -> i32 {
        self.expected_seq
    }
}

impl LookupRequest for FindValueRequest {
    fn data(&self) -> &LookupData {
        &self.data
    }

    fn data_mut(&mut self) -> &mut LookupData {
        &mut self.data
    }
}

impl Serialize for FindValueRequest {
    fn serialize<S>(&self, se: S) -> SResult<S::Ok, S::Error>
    where
        S: Serializer
    {
        let mut s = se.serialize_map(None)?;
        s.serialize_entry("t", self.target())?;
        s.serialize_entry("w", &self.want())?;
        if self.expected_seq >= 0 {
            s.serialize_entry("cas", &self.expected_seq)?;
        }
        s.end()
    }
}

impl<'de> Deserialize<'de> for FindValueRequest {
    fn deserialize<D>(de: D) -> SResult<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        enum Field {
            Want,           // "w"
            Target,         // "t"
            ExpectedSeq,    // "cas"
            Ignore          // Ignore unknown fields
        }

        impl<'de> Deserialize<'de> for Field {
            fn deserialize<D>(de: D) -> SResult<Field, D::Error>
            where
                D: Deserializer<'de>,
            {
                let key = String::deserialize(de)?;
                match key.as_str() {
                    "w"     => Ok(Field::Want),
                    "t"     => Ok(Field::Target),
                    "cas"   => Ok(Field::ExpectedSeq),
                    _       => Ok(Field::Ignore)
                }
            }
        }

        struct FieldVisiter;

        impl<'de> Visitor<'de> for FieldVisiter {
            type Value = FindValueRequest;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("FindValueRequest")
            }

            fn visit_map<V>(self, mut map: V) -> SResult<Self::Value, V::Error>
            where
                V: MapAccess<'de>,
            {
                let mut target: Option<Id> = None;
                let mut want: i32 = 0;
                let mut expected_seq: i32 = -1;

                while let Some(key) = map.next_key::<Field>()? {
                    match key {
                        Field::Target   => target = Some(map.next_value::<Id>()?),
                        Field::Want     => want = map.next_value()?,
                        Field::ExpectedSeq => expected_seq = map.next_value()?,
                        Field::Ignore => _ = map.next_value::<IgnoredAny>()?,
                    }
                }

                Ok(FindValueRequest::new(
                    target.ok_or_else(|| de::Error::missing_field("t"))?,
                    want & 0x01 != 0,
                    want & 0x02 != 0,
                    expected_seq
                ))
            }
        }
        de.deserialize_map(FieldVisiter)
    }
}

impl fmt::Display for FindValueRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        unimplemented!()
    }
}
