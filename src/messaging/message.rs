use std::fmt;
use std::collections::HashMap;
use std::time::{SystemTime, Duration, UNIX_EPOCH};

use crate::{
    Id,
};

use serde::{
    Deserialize,
    Serialize,
};

#[allow(unused)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MessageType {
    Message= 0,
    Call = 1,
    Notification = 2,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContentType {
    Text,
    Json,

    ImageJpeg,
    ImagePng,
    ImageWebp,

    AudioAac,
    AudioMp3,
    AudioWebm,

    VideoMp4,
    VideoWebm,

    Binary,
}

impl TryFrom<&str> for ContentType {
    type Error = &'static str;
    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "text/plain"        => Ok(ContentType::Text),
            "application/json"  => Ok(ContentType::Json),

            "image/jpeg"        => Ok(ContentType::ImageJpeg),
            "image/png"         => Ok(ContentType::ImagePng),
            "image/webp"        => Ok(ContentType::ImageWebp),

            "audio/aac"         => Ok(ContentType::AudioAac),
            "audio/mpeg"        => Ok(ContentType::AudioMp3),
            "audio/webm"        => Ok(ContentType::AudioWebm),

            "video/mp4"         => Ok(ContentType::VideoMp4),
            "video/webm"        => Ok(ContentType::VideoWebm),

            "application/octet-stream" => Ok(ContentType::Binary),

            _ => Err("Unknown content type for Message"),
        }
    }
}

impl fmt::Display for ContentType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(match *self {
            ContentType::Text       => "text/plain",
            ContentType::Json       => "application/json",

            ContentType::ImageJpeg  => "image/jpeg",
            ContentType::ImagePng   => "image/png",
            ContentType::ImageWebp  => "image/webp",

            ContentType::AudioAac   => "audio/aac",
            ContentType::AudioMp3   => "audio/mpeg",
            ContentType::AudioWebm  => "audio/webm",

            ContentType::VideoMp4   => "video/mp4",
            ContentType::VideoWebm  => "video/webm",

            ContentType::Binary     => "application/octet-stream",
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContentDisposition {
    Inline,
    Attachment,
}

impl TryFrom<&str> for ContentDisposition {
    type Error = &'static str;
    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "inline"            => Ok(ContentDisposition::Inline),
            "attachment"        => Ok(ContentDisposition::Attachment),

            _ => Err("Unknown content disposition for Message"),
        }
    }
}

impl fmt::Display for ContentDisposition {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(match *self {
            ContentDisposition::Inline       => "inline",
            ContentDisposition::Attachment   => "attachment",
        })
    }
}

#[allow(unused)]
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Message {
    #[serde(rename = "v")]
    version:    i32,
    #[serde(rename = "f")]
    from:       Id,
    #[serde(rename = "r")]
    to:         Id,         // alias: recipient

    #[serde(rename = "s")]
    serial_number: i32,
    #[serde(rename = "c")]
    created: u64,         // timestamp in seconds
    #[serde(rename = "t")]
    message_type: i32,

    #[serde(rename = "p")]
    #[serde(skip_serializing_if = "HashMap::is_empty")]
    properties: HashMap<String, serde_json::Value>,

    // Optional, default None[means: text/plain]
    #[serde(rename = "m")] // alias: mime type
    #[serde(skip_serializing_if = "Option::is_none")]
    content_type: Option<String>,

    // Optional, default None means INLINE
    #[serde(rename = "d")]
    #[serde(skip_serializing_if = "Option::is_none")]
    content_disposition: Option<String>,

    #[serde(rename = "b")]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    body: Vec<u8>,

    // Available only for locally sent messages (originating from the message builder).
    #[serde(skip)]
    orginal_body:   Option<serde_json::Value>,

    #[serde(skip)]
    rid             : u64, // local message id
    #[serde(skip)]
    conversation_id : Id,
    #[serde(skip)]
    encrypted       : bool,
    #[serde(skip)]
    completed       : u64 // local sent or received timestamp
}

static VERSION: i32 = 1;

#[allow(dead_code)]
impl Message {
    // TODO: Constructor

    pub fn version(&self) -> i32 {
        self.version
    }

    pub fn rid(&self) -> u64 {
        self.rid
    }

    pub(crate) fn set_rid(&mut self, rid: u64) {
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
        UNIX_EPOCH + Duration::from_secs(self.created)
    }

    pub fn message_type(&self) -> i32 {
        self.message_type
    }

    pub fn is_valid(&self) -> bool {
        self.version == VERSION &&
            self.message_type >= MessageType::Message as i32 &&
                self.message_type <= MessageType::Notification as i32 &&
                    self.created > 0
    }

    pub(crate) fn properties(&self) -> &HashMap<String, serde_json::Value> {
        &self.properties
    }

    pub(crate) fn content_type(&self) -> String {
        self.content_type.clone().unwrap_or_else(|| ContentType::Text.to_string())
    }

    pub(crate) fn content_disposition(&self) -> String {
        self.content_disposition.clone().unwrap_or_else(|| ContentDisposition::Inline.to_string())
    }

    pub(crate) fn body(&self) -> &[u8] {
        &self.body
    }

    pub(crate) fn body_as_text(&self) -> Option<String> {
        unimplemented!()
    }

    pub(crate) fn completed(&self) -> u64 {
        self.completed
    }

    pub(crate) fn is_encrypted(&self) -> bool {
        self.encrypted
    }
}

impl fmt::Display for Message {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Message[from={}, to={}, type={} serialNumber={}]",
            self.from,
            self.to,
            self.message_type,
            self.serial_number
        )
    }
}

#[allow(dead_code)]
pub(crate) struct Builder {
    message: Message,
}

#[allow(dead_code)]
impl Builder {
    pub(crate) fn with_to(&mut self, to: Id) -> &mut Self {
        self.message.to = to;
        self
    }

    pub(crate) fn with_property(&mut self, name: &str, value: serde_json::Value) -> &mut Self {
        self.message.properties.insert(name.to_string(), value);
        self
    }

    pub(crate) fn clear_property(&mut self, name: &str) -> &mut Self {
        self.message.properties.remove(name);
        self
    }

    pub(crate) fn clear_properties(&mut self) -> &mut Self {
        self.message.properties.clear();
        self
    }

    pub(crate) fn with_content_type(&mut self, content_type: ContentType) -> &mut Self {
        self.message.content_type = Some(content_type.to_string());
        self
    }

    pub(crate) fn with_content_disposition(&mut self, content_disposition: ContentDisposition) -> &mut Self {
        self.message.content_disposition = Some(content_disposition.to_string());
        self
    }

    pub fn with_body(&mut self, body: Vec<u8>) -> &mut Self {
        self.message.body = body.clone();
        // self.message.original_body = body;
        self
    }
}