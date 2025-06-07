use std::collections::HashMap;

use crate::Id;

use super::message::{
    ContentType,
    ContentDisposition,
};

#[allow(dead_code)]
pub(crate) struct MessageBuilder {
    to: Option<Id>,
    properties: HashMap<String, serde_json::Value>,
    content_type: Option<ContentType>,
    content_disposition: Option<ContentDisposition>,
}

#[allow(dead_code)]
impl MessageBuilder {
    pub(crate) fn new() -> Self {
        Self {
            to: None,
            properties: HashMap::new(),
            content_type: None,
            content_disposition: None,
        }
    }

    pub(crate) fn with_to(&mut self, to: Id) -> &mut Self {
        self.to = Some(to);
        self
    }

    pub(crate) fn with_property(&mut self, name: &str, value: serde_json::Value) -> &mut Self {
        self.properties.insert(name.to_string(), value);
        self
    }

    pub(crate) fn clear_property(&mut self, name: &str) -> &mut Self {
        self.properties.remove(name);
        self
    }

    pub(crate) fn clear_properties(&mut self) -> &mut Self {
        self.properties.clear();
        self
    }

    pub(crate) fn with_content_type(&mut self, content_type: ContentType) -> &mut Self {
        self.content_type = Some(content_type);
        self
    }

    pub(crate) fn with_content_disposition(&mut self, content_disposition: ContentDisposition) -> &mut Self {
        self.content_disposition = Some(content_disposition);
        self
    }

    pub fn with_body(&mut self, _body: Vec<u8>) -> &mut Self {
        // self.message.body = body.clone();
        // self.message.original_body = body;
        self
    }
}