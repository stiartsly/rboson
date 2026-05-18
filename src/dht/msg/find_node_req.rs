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

pub(crate) struct FindNodeRequest {
    data: LookupData,
}

impl FindNodeRequest {
    pub(crate) fn new(
        target: Id,
        want4: bool,
        want6: bool,
        want_token: bool
    ) -> Self {
        Self {
            data: LookupData::new(target, want4, want6, want_token)
        }
    }
}

impl LookupRequest for FindNodeRequest {
    fn data(&self) -> &LookupData {
        &self.data
    }

    fn data_mut(&mut self) -> &mut LookupData {
        &mut self.data
    }
}

impl Serialize for FindNodeRequest {
    fn serialize<S>(&self, se: S) -> SResult<S::Ok, S::Error>
    where
        S: Serializer
    {
        let mut s = se.serialize_map(None)?;
        s.serialize_entry("t", self.target())?;
        s.serialize_entry("w", &self.want())?;
        s.end()
    }
}

impl<'de> Deserialize<'de> for FindNodeRequest {
    fn deserialize<D>(de: D) -> SResult<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        enum Field {
            Want,           // "w"  - i32 (bitmask: 0x01 for want4, 0x02 for want6, 0x04 for want_token)
            Target,         // "t"  - Id
            Ignore          // Ignore unknown fields
        }

        impl<'de> Deserialize<'de> for Field {
            fn deserialize<D>(de: D) -> SResult<Field, D::Error>
            where
                D: Deserializer<'de>,
            {
                let key = String::deserialize(de)?;
                match key.as_str() {
                    "w" => Ok(Field::Want),
                    "t" => Ok(Field::Target),
                    _   => Ok(Field::Ignore)
                }
            }
        }

        struct FieldVisiter;
        impl<'de> Visitor<'de> for FieldVisiter {
            type Value = FindNodeRequest;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a FindNodeRequest struct")
                }

            fn visit_map<V>(self, mut map: V) -> SResult<Self::Value, V::Error>
            where
                V: MapAccess<'de>,
            {
                let mut target: Option<Id> = None;
                let mut want: i32 = 0;

                while let Some(key) = map.next_key::<Field>()? {
                    match key {
                        Field::Target   => target = Some(map.next_value::<Id>()?),
                        Field::Want     => want = map.next_value()?,
                        Field::Ignore   => _ = map.next_value::<IgnoredAny>()?,
                    }
                }

                Ok(FindNodeRequest::new(
                    target.ok_or_else(|| de::Error::missing_field("t"))?,
                    want & 0x01 != 0,
                    want & 0x02 != 0,
                    want & 0x04 != 0
                ))
            }
        }
        de.deserialize_map(FieldVisiter)
    }
}

impl fmt::Display for FindNodeRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f,
            "t:{},w:{}",
            self.target(),
            self.want()
        )?;
        Ok(())
    }
}
