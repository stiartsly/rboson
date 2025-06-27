use std::time::{Duration, SystemTime};
use unicode_normalization::UnicodeNormalization;
use serde_json::{Map, Value};

use crate::{
    as_secs,
    error::Result,
    core::crypto_identity::CryptoIdentity
};

pub(crate) trait BosonIdentityObjectBuilder {
    type BosonIdentityObject;

    fn trim_millis(date: SystemTime) -> SystemTime {
        SystemTime::UNIX_EPOCH + Duration::new(as_secs!(date), 0)
    }

    fn now() -> SystemTime {
        Self::trim_millis(SystemTime::now())
    }

    fn normalize(object: Value) -> Value {
        match object {
            Value::String(s) => Value::String(s.nfc().collect()),
            Value::Array(arr) => {
                let normalized = arr.into_iter()
                    .map(Self::normalize)
                    .collect::<Vec<Value>>();
                Value::Array(normalized)
            },
            Value::Object(obj) => {
                let normalized = obj.into_iter()
                    .map(|(k, v)| (
                        Self::normalize(Value::String(k)).as_str().unwrap().to_string(),
                        Self::normalize(v)
                    )).collect::<Map<String, Value>>();
                Value::Object(normalized)
            },
            _ => object,
        }
    }

    fn identity(&self) -> &CryptoIdentity;
    fn build(&self) -> Result<Self::BosonIdentityObject>;
}