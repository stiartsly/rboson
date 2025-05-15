use std::time::SystemTime;

use crate::{
    Id,
};

use serde::{
    Deserialize,
    Serialize,
};

#[derive(Debug, Clone, Deserialize, Serialize, Hash)]
#[allow(unused)]
pub struct Message {
    #[serde(rename = "v")]
    version:    i32,
    #[serde(rename = "f")]
    from:       Id,
    #[serde(rename = "r")]
    to:         Id,

    #[serde(rename = "s")]
    serial_number: i32,
    #[serde(rename = "c")]
    created:    i32,

    #[serde(rename = "t")]
    message_type: i32,

    // #[serde(rename = "p")]
    // properties:

    // Optional, default null[means: text/plain]
    #[serde(rename = "m")] // alias: mime type
    #[serde(skip_serializing_if = "String::is_empty")]
    content_type: String,

    #[serde(rename = "d")]
    #[serde(skip_serializing_if = "String::is_empty")]
    content_disposition: String,

    #[serde(rename = "b")]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    body:       Vec<u8>,

    #[serde(skip)]
    rid:        i32, // local message id
    #[serde(skip)]
    conversation_id: Id,

    #[serde(skip)]
    encrypted: bool,
    #[serde(skip)]
    completed: u64 // local sent or received timestamp
}

static VERSION: i32 = 1;

#[allow(dead_code)]
impl Message {
    pub fn version(&self) -> i32 {
        self.version
    }

    pub fn rid(&self) -> i32 {
        self.rid
    }

    pub(crate) fn set_rid(&mut self, rid: i32) {
        self.rid = rid;
    }

    pub fn conversation_id(&self) -> &Id {
        &self.conversation_id
    }

    pub(crate) fn set_conversation_id(&mut self, conversation_id: &Id) {
        self.conversation_id = conversation_id.clone();
    }

    pub fn from(&self) -> &Id {
        &self.from
    }

    pub fn to(&self) -> &Id {
        &self.to
    }

    pub fn serial_number(&self) -> i32 {
        self.serial_number
    }

    pub fn created(&self) -> SystemTime {
        unimplemented!()
    }

    pub fn message_type(&self) -> i32 {
        self.message_type
    }

    pub fn is_valid(&self) -> bool {
        self.version == VERSION &&
            self.message_type >= 0 && self.message_type <= 0 &&
            self.created > 0
    }

    pub fn content_type(&self) -> String {
        "todo".to_string()
    }

    pub fn body_as_text(&self) -> String {
        "todo".to_string()
    }
}

#[allow(dead_code)]
struct Builder {
    message: Message,
}

#[allow(dead_code)]
impl Builder {
    pub fn with_to(&mut self, to: Id) -> &mut Self {
        self.message.to = to;
        self
    }

    pub fn with_properties<T>(&mut self, _properties: Vec<(String, T)>) -> &mut Self
    where T: Serialize {
        // self.message.properties = properties;
        self
    }

    pub fn clear_properties(&mut self) -> &mut Self {
        // self.message.properties.clear();
        self
    }

    pub fn with_body(&mut self, body: Vec<u8>) -> &mut Self {
        self.message.body = body.clone();
        // self.message.original_body = body;
        self
    }
}