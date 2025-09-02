use std::fmt;
use std::str::FromStr;
use std::collections::HashMap;
use std::time::{SystemTime, Duration, UNIX_EPOCH};
use serde::{Deserialize, Serialize};

use crate::{
    as_ms,
    Id,
    core::{
        Error,
        Result,
        CryptoContext
    }
};

#[allow(unused)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MessageType {
    Message= 0,
    Call = 1,
    Notification = 2,
}

impl TryFrom<i32> for MessageType {
    type Error = Error;

    fn try_from(value: i32) -> Result<Self> {
        match value {
            0 => Ok(MessageType::Message),
            1 => Ok(MessageType::Call),
            2 => Ok(MessageType::Notification),
            _ => Err(Error::Argument("Invalid integer for MessageType".into())),
        }
    }
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

impl FromStr for ContentType {
    type Err = Error;

    fn from_str(value: &str) -> Result<Self> {
        match value {
            "text/plain"        => Ok(Self::Text),
            "application/json"  => Ok(Self::Json),

            "image/jpeg"        => Ok(Self::ImageJpeg),
            "image/png"         => Ok(Self::ImagePng),
            "image/webp"        => Ok(Self::ImageWebp),

            "audio/aac"         => Ok(Self::AudioAac),
            "audio/mpeg"        => Ok(Self::AudioMp3),
            "audio/webm"        => Ok(Self::AudioWebm),

            "video/mp4"         => Ok(Self::VideoMp4),
            "video/webm"        => Ok(Self::VideoWebm),

            "application/octet-stream" => Ok(Self::Binary),

            _ => Err(Error::Argument("Unknown message content type".into())),
        }
    }
}

impl fmt::Display for ContentType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(match *self {
            Self::Text       => "text/plain",
            Self::Json       => "application/json",

            Self::ImageJpeg  => "image/jpeg",
            Self::ImagePng   => "image/png",
            Self::ImageWebp  => "image/webp",

            Self::AudioAac   => "audio/aac",
            Self::AudioMp3   => "audio/mpeg",
            Self::AudioWebm  => "audio/webm",

            Self::VideoMp4   => "video/mp4",
            Self::VideoWebm  => "video/webm",

            Self::Binary     => "application/octet-stream",
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContentDisposition {
    Inline,
    Attachment,
}

impl FromStr for ContentDisposition {
    type Err = Error;

    fn from_str(value: &str) -> Result<Self> {
        match value {
            "inline"            => Ok(Self::Inline),
            "attachment"        => Ok(Self::Attachment),

            _ => Err(Error::Argument("Unknown message content disposition".into())),
        }
    }
}

impl fmt::Display for ContentDisposition {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(match *self {
            Self::Inline       => "inline",
            Self::Attachment   => "attachment",
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
    #[serde(skip_serializing_if = "Option::is_none")]
    properties: Option<serde_json::Map<String, serde_json::Value>>,

    // Optional, default None[means: text/plain]
    #[serde(rename = "m")] // alias: mime type
    #[serde(skip_serializing_if = "crate::is_none_or_empty")]
    content_type: Option<String>,

    // Optional, default None means INLINE
    #[serde(rename = "d")]
    #[serde(skip_serializing_if = "crate::is_none_or_empty")]
    content_disposition: Option<String>,

    #[serde(rename = "b")]
    #[serde(skip_serializing_if = "crate::is_none_or_empty")]
    body: Option<Vec<u8>>,

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
        UNIX_EPOCH + Duration::from_millis(self.created)
    }

    pub fn message_type(&self) -> Result<MessageType> {
        MessageType::try_from(self.message_type)
    }

    pub fn is_valid(&self) -> bool {
        self.version == VERSION &&
            self.message_type >= MessageType::Message as i32 &&
                self.message_type <= MessageType::Notification as i32 &&
                    self.created > 0
    }

    pub(crate) fn properties(&self) -> &HashMap<String, serde_json::Value> {
        unimplemented!()
    }

    pub(crate) fn content_type(&self) -> String {
        self.content_type.clone().unwrap_or_else(|| ContentType::Text.to_string())
    }

    pub(crate) fn content_disposition(&self) -> String {
        self.content_disposition.clone().unwrap_or_else(|| ContentDisposition::Inline.to_string())
    }

    pub(crate) fn body(&self) -> Option<&[u8]> {
        self.body.as_deref()
    }

    pub(crate) fn body_as_text(&self) -> Option<String> {
        self.body.as_ref().map(|b| {
            String::from_utf8_lossy(b).to_string()
        })
    }

    pub(crate) fn completed(&self) -> u64 {
        self.completed
    }

    pub(crate) fn mark_completed(&mut self, completed: SystemTime) {
        self.completed = as_ms!(completed) as u64
    }

    pub(crate) fn is_encrypted(&self) -> bool {
        self.encrypted
    }

    pub(crate) fn mark_encrypted(&mut self, encrypted: bool) {
        self.encrypted = encrypted;
    }

    pub(crate) fn on_sent(&self) {
        // TODO: unimplemented!()
    }

    pub(crate) fn decrypt_body(&mut self, ctx: &CryptoContext) -> Result<()> {
        self.body = self.body.as_ref().map(|v| {
            ctx.decrypt_into(v)
        }).transpose()?;
        self.encrypted = false;
        Ok(())
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
    message: Option<Message>,
}

#[allow(dead_code)]
impl Builder {
    pub(crate) fn clone_from(message: &Message) -> Self {
        Self {
            message: Some(message.clone())
        }
    }

    fn message_mut(&mut self) -> &mut Message {
        self.message.as_mut().expect("Message is not set")
    }

    pub(crate) fn with_to(&mut self, to: Id) -> &mut Self {
        self.message_mut().to = to;
        self
    }

    pub(crate) fn with_property(&mut self, name: &str, value: Option<serde_json::Value>) -> &mut Self {
        let map = self.message_mut().properties
            .get_or_insert_with(serde_json::Map::new);

        _ = match value {
            Some(v) => map.insert(name.to_string(), v),
            None => map.remove(name)
        };
        self
    }

    pub(crate) fn clear_properties(&mut self) -> &mut Self {
        self.message_mut().properties.as_mut().map(|map| {
            map.clear();
        });
        self.message_mut().properties = None;
        self
    }

    pub(crate) fn with_content_type(&mut self, content_type: ContentType) -> &mut Self {
        self.message_mut().content_type = Some(content_type.to_string());
        self
    }

    pub(crate) fn with_body(&mut self, body: Vec<u8>) -> &mut Self {
        self.message_mut().body = Some(body);
        // TODO: self.message.orginal_body = None;
        self
    }

    pub(crate) fn build(&mut self) -> Message {
        self.message.take().expect("Message is not set")
    }
}