use std::{
    time::SystemTime,
    future::Future
};

use boson_api::{Id, Result};
use crate::{
    Content,
    Message,
    MessageType,
    message::ContentDisposition,
};

/// A single message in a conversation.
pub trait MessageOps: Send + Sync {
    /// The globally unique sender-generated message ID.
    fn id(&self) -> &Id;

    /// The device-local, store-assigned row ID, or zero before persistence.
    fn rid(&self) -> i64;

    /// The conversation ID, if assigned.
    fn conversation_id(&self) -> Option<&Id>;

    /// The intended recipient's boson `Id`.
    fn recipient(&self) -> &Id;

    /// The category of this message.
    fn message_type(&self) -> MessageType;

    /// The sender's boson `Id`, if the message has been stamped for dispatch.
    fn from(&self) -> Option<&Id>;

    /// When the message was authored.
    fn created_at(&self) -> SystemTime;

    /// When this device received the message (`None` for outbound messages).
    fn received_at(&self) -> Option<SystemTime>;

    /// When this device successfully sent the message (`None` for inbound).
    fn sent_at(&self) -> Option<SystemTime>;

    /// The raw payload bytes.
    fn payload_as_bytes(&self) -> &[u8];

    /// The decoded content, if decryption succeeded.
    fn payload_as_content(&self) -> Option<&Content>;
}

// ---------------------------------------------------------------------------
// MessageBuilder trait
// ---------------------------------------------------------------------------

/// Fluent builder for composing and sending a message.
///
/// Methods that accept a `String`-like value take `&str` to remain dyn-compatible.
pub trait MessageBuilderOps: Send + Sync {
    /// Set the MIME content type.
    fn with_content_type(self, ct: &str) -> Self;

    /// Set the content disposition.
    fn with_content_disposition(self, cd: ContentDisposition) -> Self;

    /// Set a UTF-8 text body.
    fn with_text_body(self, text: &str) -> Self;

    /// Set an arbitrary binary body.
    fn with_binary_body(self, data: Vec<u8>) -> Self;

    /// Add an arbitrary header.
    fn with_header(self, key: &str, value: &str) -> Self ;

    /// Builds and sends the message.
    fn send(self) -> impl Future<Output = Result<Message>>;
}