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
            name.map_or(Value::Null, Value::String),
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
            value.map_or(Value::Null, Value::String),
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
                /*fields.insert(
                    name.to_owned(),
                    value.clone().map_or(Value::Null, Value::String),
                );*/
                fields.insert(
                    name.to_owned(),
                    value.clone()
                );
            }
        }
        fields
    }
}
