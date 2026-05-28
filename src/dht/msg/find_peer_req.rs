use std::{
    fmt,
    result::Result as SResult
};
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

pub(crate) struct FindPeerRequest {
    data: LookupData,
    expected_seq: i32,
    expected_count: i32,
}

impl FindPeerRequest {
    pub(crate) fn new(
        target: Id, want4: bool,  want6: bool,
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
}

impl Serialize for FindPeerRequest {
    fn serialize<S>(&self, se: S) -> SResult<S::Ok, S::Error>
    where S: Serializer
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
    where D: Deserializer<'de>,
    {
        enum Field {
            Want,           // "w"
            Target,         // "t"
            ExpectedSeq,    // "cas"
            ExpectedCount,  // "e"
            Ignore          // Ignore unknown fields
        }

        impl<'de> Deserialize<'de> for Field {
            fn deserialize<D>(de: D) -> SResult<Field, D::Error>
            where D: Deserializer<'de>,
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

        struct FieldVisitor;
        impl<'de> Visitor<'de> for FieldVisitor {
            type Value = FindPeerRequest;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a FindPeerRequest struct")
            }

            fn visit_map<V>(self, mut map: V) -> SResult<Self::Value, V::Error>
            where V: MapAccess<'de>,
            {
                let mut target  : Option<Id>  = None;
                let mut want    : Option<i32> = None;
                let mut seq     : Option<i32> = None;
                let mut count   : Option<i32> = None;

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
                            if seq.is_some() {
                                return Err(de::Error::duplicate_field("cas"));
                            } else {
                                seq = Some(map.next_value()?);
                            }
                        }
                        Field::ExpectedCount => {
                            if count.is_some() {
                                return Err(de::Error::duplicate_field("e"));
                            } else {
                                count = Some(map.next_value()?);
                            }
                        }
                        Field::Ignore => {
                            let _ = map.next_value::<IgnoredAny>()?;
                        }
                    }
                }

                let want = want.unwrap_or(0);
                let expected_seq = seq.unwrap_or(-1);
                if expected_seq < -1 {
                    return Err(de::Error::custom("expected_seq must be larger than or equal to -1"));
                }
                let expected_count = count.unwrap_or_default();
                if expected_count <= 0 {
                    return Err(de::Error::custom("expected_count must be at least larger than 0"));
                }

                Ok(FindPeerRequest::new(
                    target.ok_or_else(|| de::Error::missing_field("t"))?,
                    (want & WANT4_MASK) != 0,
                    (want & WANT6_MASK) != 0,
                    expected_seq,
                    expected_count
                ))
            }
        }
        de.deserialize_map(FieldVisitor)
    }
}

impl fmt::Display for FindPeerRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "t:{},w:{}",
            self.target(),
            self.want()
        )?;
        if self.expected_seq >= 0 {
            write!(f, ",cas:{}", self.expected_seq)?;
        }
        write!(f, ",e:{}", self.expected_count)
    }
}
