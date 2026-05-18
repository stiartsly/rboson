use std::fmt;
use std::result::Result as SResult;
use serde::{
    Deserialize, Serialize,
    de::{self, Deserializer, MapAccess, Visitor, IgnoredAny},
    ser::{SerializeMap, Serializer}
};

#[derive(Debug)]
pub(crate) struct Error {
    code: i32,
    msg: String,
}

impl Error {
    pub(crate) fn new(code: i32, msg: String) -> Self {
        Self {
            code,
            msg
        }
    }

    pub(crate) fn code(&self) -> i32 {
        self.code
    }

    pub(crate) fn msg(&self) -> &str {
        &self.msg
    }
}

impl Serialize for Error {
    fn serialize<S>(&self, se: S) -> SResult<S::Ok, S::Error>
    where
        S: Serializer
    {
        let mut s = se.serialize_map(None)?;
        s.serialize_entry("c", &self.code)?;
        if !self.msg.is_empty() {
            s.serialize_entry("m", &self.msg)?;
        }
        s.end()
    }
}

impl<'de> Deserialize<'de> for Error {
    fn deserialize<D>(de: D) -> SResult<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        enum Field {
            Code,           // "c"  - i32
            Msg,            // "m"  - String
            Ignore          // Ignore unknown fields
        }

        impl<'de> Deserialize<'de> for Field {
            fn deserialize<D>(de: D) -> SResult<Field, D::Error>
            where
                D: Deserializer<'de>,
            {
                let key = String::deserialize(de)?;
                match key.as_str() {
                    "c" => Ok(Field::Code),
                    "m" => Ok(Field::Msg),
                    _   => Ok(Field::Ignore)
                }
            }
        }

        struct FieldVisiter;
        impl<'de> Visitor<'de> for FieldVisiter {
            type Value = Error;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("Error")
            }

            fn visit_map<V>(self, mut map: V) -> SResult<Self::Value, V::Error>
            where
                V: MapAccess<'de>,
            {
                let mut code: Option<i32> = None;
                let mut msg: Option<String> = None;
                while let Some(key) = map.next_key::<Field>()? {
                    match key {
                        Field::Code    => code = Some(map.next_value()?),
                        Field::Msg     => msg = Some(map.next_value()?),
                        Field::Ignore  => _ = map.next_value::<IgnoredAny>()?,
                    }
                }

                Ok(Error::new(
                    code.ok_or_else(|| de::Error::missing_field("c"))?,
                    msg.ok_or_else(|| de::Error::missing_field("m"))?
                ))
            }
        }
        de.deserialize_map(FieldVisiter)
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f,"c:{}.m:{}", self.code(),self.msg())
    }
}
