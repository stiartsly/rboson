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

pub(crate) struct FindPeerRequest {
    data: LookupData,
    expected_seq: i32,
    expected_count: i32,
}

impl FindPeerRequest {
    pub(crate) fn new(
        target: Id,
        want4: bool,
        want6: bool,
        expected_seq: i32,
        expected_count: i32,
    ) -> Self {
        Self {
            data: LookupData::new(target, want4, want6, false),
            expected_seq,
            expected_count,
        }
    }

    pub(crate) fn expected_seq(&self) -> i32 {
        self.expected_seq
    }

    pub(crate) fn expected_count(&self) -> i32 {
        self.expected_count
    }
}

impl LookupRequest for FindPeerRequest {
    fn data(&self) -> &LookupData {
        &self.data
    }

    fn data_mut(&mut self) -> &mut LookupData {
        &mut self.data
    }
}

impl Serialize for FindPeerRequest {
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
        if self.expected_count > 0 {
            s.serialize_entry("e", &self.expected_count)?;
        }
        s.end()
    }
}

impl<'de> Deserialize<'de> for FindPeerRequest {
    fn deserialize<D>(de: D) -> SResult<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Debug)]
        enum Field {
            Want,           // "w"
            Target,         // "t"
            ExpectedSeq,    // "cas"
            ExpectedCount,  // "e"
            Ignore          // Ignore unknown fields
        }

        impl<'de> Deserialize<'de> for Field {
            fn deserialize<D>(de: D) -> SResult<Field, D::Error>
            where
                D: Deserializer<'de>,
            {
                let key = String::deserialize(de)?;
                match key.as_str() {
                    "t"     => Ok(Field::Target),
                    "w"     => Ok(Field::Want),
                    "cas"   => Ok(Field::ExpectedSeq),
                    "e"     => Ok(Field::ExpectedCount),
                    _       => Ok(Field::Ignore)
                }
            }
        }

        struct FieldVisiter;
        impl<'de> Visitor<'de> for FieldVisiter {
            type Value = FindPeerRequest;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("FindPeerRequest")
            }

            fn visit_map<V>(self, mut map: V) -> SResult<Self::Value, V::Error>
            where
                V: MapAccess<'de>,
            {
                let mut target: Option<Id> = None;
                let mut want: i32 = 0;
                let mut expected_seq = -1;
                let mut expected_count = 0;

                while let Some(key) = map.next_key::<Field>()? {
                    match key {
                        Field::Target => target = Some(map.next_value::<Id>()?),
                        Field::Want => want = map.next_value()?,
                        Field::ExpectedSeq  => expected_seq = map.next_value()?,
                        Field::ExpectedCount => expected_count = map.next_value()?,
                        Field::Ignore => _ = map.next_value::<IgnoredAny>()?,
                    }
                }

                if expected_seq < -1 {
                    return Err(de::Error::custom("expected_seq must be larger than or equal to -1"));
                }
                if expected_count <= 0 {
                    return Err(de::Error::custom("expected_count must be at least larger than 0"));
                }

                Ok(FindPeerRequest::new(
                    target.ok_or_else(|| de::Error::missing_field("t"))?,
                    (want & 0x01) != 0,
                    (want & 0x02) != 0,
                    expected_seq,
                    expected_count
                ))
            }
        }
        de.deserialize_map(FieldVisiter)
    }
}

impl fmt::Display for FindPeerRequest {
    fn fmt(&self, _f: &mut fmt::Formatter<'_>) -> fmt::Result {
        unimplemented!()
    }
}
