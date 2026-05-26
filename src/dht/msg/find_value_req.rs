use std::fmt;
use std::result::Result as SResult;
use serde::{
    Deserialize, Serialize,
    de::{self, Deserializer, MapAccess, Visitor, IgnoredAny},
    ser::{SerializeMap, Serializer}
};

use crate::{
    Id,
    dht::msg::lookup_req::{
        LookupRequest,
        Data as LookupData
    }
};

const WANT4_MASK: i32 = 0x01;
const WANT6_MASK: i32 = 0x02;

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
}

impl Serialize for FindValueRequest {
    fn serialize<S>(&self, se: S) -> SResult<S::Ok, S::Error>
    where S: Serializer
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
    where D: Deserializer<'de>,
    {
        enum Field {
            Want,           // "w"
            Target,         // "t"
            ExpectedSeq,    // "cas"
            Ignore          // Ignore unknown fields
        }

        impl<'de> Deserialize<'de> for Field {
            fn deserialize<D>(de: D) -> SResult<Field, D::Error>
            where D: Deserializer<'de>,
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

        struct FieldVisitor;
        impl<'de> Visitor<'de> for FieldVisitor {
            type Value = FindValueRequest;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a FindValueRequest struct")
            }

            fn visit_map<V>(self, mut map: V) -> SResult<Self::Value, V::Error>
            where V: MapAccess<'de>,
            {
                let mut target: Option<Id> = None;
                let mut want: Option<i32> = None;
                    let mut expected_seq: Option<i32> = None;

                while let Some(key) = map.next_key::<Field>()? {
                    match key {
                        Field::Target => {
                            if target.is_some() {
                                return Err(de::Error::duplicate_field("t"));
                            } else {
                                target = Some(map.next_value::<Id>()?);
                            }
                        }
                        Field::Want => {
                            if want.is_some() {
                                return Err(de::Error::duplicate_field("w"));
                            } else {
                                want = Some(map.next_value()?);
                            }
                        }
                        Field::ExpectedSeq => {
                            if expected_seq.is_some() {
                                return Err(de::Error::duplicate_field("cas"));
                            } else {
                                expected_seq = Some(map.next_value()?);
                            }
                        }
                        Field::Ignore => {
                            let _ = map.next_value::<IgnoredAny>()?;
                        }
                    }
                }

                let want = want.unwrap_or_default();
                let expected_seq = expected_seq.unwrap_or(-1);
                if expected_seq < -1 {
                    return Err(de::Error::custom("expected_seq must be larger than or equal to -1"));
                }

                Ok(FindValueRequest::new(
                    target.ok_or_else(|| de::Error::missing_field("t"))?,
                    want & WANT4_MASK != 0,
                    want & WANT6_MASK != 0,
                    expected_seq
                ))
            }
        }
        de.deserialize_map(FieldVisitor)
    }
}

impl fmt::Display for FindValueRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "t:{},w:{}", self.target(), self.want())?;
        if self.expected_seq >= 0 {
            write!(f, ",cas:{}", self.expected_seq)?;
        }
        Ok(())
    }
}
