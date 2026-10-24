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

#[derive(Clone, Debug, Default)]
pub struct ProfileUpdate {
    fields: Map<String, Value>,
}

impl ProfileUpdate {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_name(mut self, name: Option<String>) -> Self {
        self.fields.insert(
            "name".to_string(),
            name.map_or(Value::Null, Value::String)
        );
        self
    }

    pub fn with_email(mut self, value: Option<String>) -> Self {
        self.fields.insert(
            "email".to_string(),
            value.map_or(Value::Null, Value::String),
        );
        self
    }

    pub fn with_bio(mut self, value: Option<String>) -> Self {
        self.fields.insert(
            "bio".to_string(),
            value.map_or(Value::Null, Value::String)
        );
        self
    }

    pub fn is_empty(&self) -> bool {
        self.fields.is_empty()
    }

    pub(crate) fn fields(&self) -> Map<String, Value> {
        let mut fields = Map::new();
        for (name, value) in [
            ("name", self.fields.get("name")),
            ("email", self.fields.get("email")),
            ("bio", self.fields.get("bio")),
        ] {
            if let Some(value) = value {
                fields.insert(name.to_owned(), value.clone());
            }
        }
        fields
    }
}
