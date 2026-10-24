// Copyright (c) 2023 -      bosonnetwork.io
//
// Permission is hereby granted, free of charge, to any person obtaining a copy
// of this software and associated documentation files (the "Software"), to deal
// in the Software without restriction, including without limitation the rights
// to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
// copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions:
//
// The above copyright notice and this permission notice shall be included in all
// copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
// OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
// SOFTWARE.

use serde_json::{Map, Value};
use std::time::{Duration, SystemTime};
use unicode_normalization::UnicodeNormalization;

use crate::{did_constants, DIDUrl};
use boson_api::{
    as_secs,
    errors::StateError,
    Id, Result, CryptoIdentity,
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
                let normalized = arr.into_iter().map(Self::normalize).collect::<Vec<Value>>();
                Value::Array(normalized)
            }
            Value::Object(obj) => {
                let normalized = obj
                    .into_iter()
                    .map(|(k, v)| {
                        (
                            Self::normalize(Value::String(k))
                                .as_str()
                                .unwrap()
                                .to_string(),
                            Self::normalize(v),
                        )
                    })
                    .collect::<Map<String, Value>>();
                Value::Object(normalized)
            }
            _ => object,
        }
    }

    /// Normalizes an object id relative to a subject.
    ///
    /// Bare ids become a DID URL fragment; full DID URLs must belong to the
    /// subject and include a fragment, matching the Java builder base class.
    fn normalize_subject_id(subject: &Id, id: &str) -> Result<DIDUrl> {
        if id.is_empty() {
            return Err(StateError::new("id must not be empty").into());
        }
        if !id.starts_with(did_constants::DID_SUFFIXED_SCHEME) {
            return Ok(DIDUrl::new(subject, None, None, Some(id)));
        }

        let url = DIDUrl::parse(id)?;
        if url.id() != Some(subject) {
            return Err(StateError::new(format!(
                "id DID URL subject part must be did:boson:{subject}: {id}"
            )));
        }
        if url.fragment().is_none() {
            return Err(StateError::new(format!(
                "id DID URL must include a fragment: {id}"
            )));
        }
        Ok(url)
    }

    fn identity(&self) -> &CryptoIdentity;
    fn build(&self) -> Result<Self::BosonIdentityObject>;
}
