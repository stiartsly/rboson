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

use std::{
    collections::HashMap,
    future::Future,
    pin::Pin,
    sync::Arc,
};

use boson_api::{
    errors::{
        ArgumentError,
        Result,
        StateError,
    },
    Id,
};

use crate::{
    content_type,
    ContentDisposition,
    Message,
    MessageListener,
    CONTENT_DISPOSITION_HEADER,
};

use crate::interfaces::MessageBuilderOps;

use super::{
    verticle,
    verticle::Event,
};

pub struct ClientMessageBuilder {
    recipient: Option<Id>,
    verticle_tx: Option<tokio::sync::mpsc::UnboundedSender<Event>>,
    message_listener: Arc<dyn MessageListener>,
    content_type: Option<String>,
    content_disposition: Option<ContentDisposition>,
    headers: Vec<(String, String)>,
    body: Option<Vec<u8>>,
    text: bool,
}

impl ClientMessageBuilder {
    pub(crate) fn new(
        recipient: Option<Id>,
        verticle_tx: Option<tokio::sync::mpsc::UnboundedSender<Event>>,
        message_listener: Arc<dyn MessageListener>,
    ) -> Self {
        Self {
            recipient,
            verticle_tx,
            message_listener,
            content_type: None,
            content_disposition: None,
            headers: Vec::new(),
            body: None,
            text: false,
        }
    }
}


impl MessageBuilderOps for ClientMessageBuilder {
    fn with_content_type(mut self: Box<Self>, content_type: &str) -> Box<dyn MessageBuilderOps> {
        self.content_type = Some(content_type.to_string());
        self
    }

    fn with_content_disposition(mut self: Box<Self>, disposition: ContentDisposition) -> Box<dyn MessageBuilderOps> {
        self.content_disposition = Some(disposition);
        self
    }

    fn with_text_body(mut self: Box<Self>, text: &str) -> Box<dyn MessageBuilderOps> {
        self.body = Some(text.as_bytes().to_vec());
        self.text = true;
        self
    }

    fn with_binary_body(mut self: Box<Self>, data: Vec<u8>) -> Box<dyn MessageBuilderOps> {
        self.body = Some(data);
        self.text = false;
        self
    }

    fn with_header(mut self: Box<Self>, key: &str, value: &str) -> Box<dyn MessageBuilderOps> {
        self.headers.push((key.to_string(), value.to_string()));
        self
    }

    fn send(self: Box<Self>) -> Pin<Box<dyn Future<Output = Result<Message>> + Send>> {
        Box::pin(async move {
            let recipient = self
                .recipient
                .ok_or_else(|| ArgumentError::new("Message recipient is required"))?;
            let body = self
                .body
                .ok_or_else(|| ArgumentError::new("Message content is required"))?;
            let sender = self
                .verticle_tx
                .ok_or_else(|| StateError::new("Messaging client is not running"))?;

            let text = self.text;
            let headers = merge_headers(
                self.headers,
                self.content_type,
                self.content_disposition,
                text,
            );

            let (promise, future) = verticle::Promise::pair();
            sender
                .send(verticle::Event::ContentMessage(
                    verticle::Body::new(
                        verticle::MsgContentParams {
                            conversation_id: recipient,
                            metadata: headers,
                            content: body,
                            is_encrypted: text,
                        },
                        promise,
                    ),
                ))
                .map_err(|_| StateError::new("Messaging verticle event channel closed"))?;

            let message = future.await.map_err(StateError::new)?;
            self.message_listener.on_sent(&message);
            Ok(message)
        })
    }
}

fn merge_headers(
    headers: Vec<(String, String)>,
    declared_type: Option<String>,
    disposition: Option<ContentDisposition>,
    text: bool,
) -> HashMap<String, serde_json::Value> {
    let mut merged: HashMap<String, serde_json::Value> = headers
        .into_iter()
        .map(|(key, value)| (key, serde_json::Value::String(value)))
        .collect();

    if let Some(declared_type) = declared_type {
        merged.insert(
            content_type::HEADER_NAME.into(),
            serde_json::Value::String(declared_type),
        );
    } else if !text {
        merged.insert(
            content_type::HEADER_NAME.into(),
            serde_json::Value::String(content_type::BINARY.into()),
        );
    }

    if let Some(disposition) = disposition {
        merged.insert(
            CONTENT_DISPOSITION_HEADER.into(),
            serde_json::Value::String(disposition.value()),
        );
    }

    merged
}
