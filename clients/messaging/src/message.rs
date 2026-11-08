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

use boson_api::{
    errors::ArgumentError,
    Error,
    Result
};
use crate::{
    MessageOps,
    MessageContentOps,
    MessageBuilderOps,
};

// ---------------------------------------------------------------------------
// MessageType
// ---------------------------------------------------------------------------

/// Top-level category of a message.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum MessageType {
    /// Initial handshake / key-exchange message.
    Handshake = 0,
    /// Regular user-visible content message.
    Content = 1,
    /// Control / signalling message (not user-visible).
    Control = 2,
    /// State-synchronisation message.
    State = 3,
}

impl TryFrom<i32> for MessageType {
    type Error = Error;

    fn try_from(value: i32) -> Result<Self> {
        match value {
            0 => Ok(MessageType::Handshake),
            1 => Ok(MessageType::Content),
            2 => Ok(MessageType::Control),
            3 => Ok(MessageType::State),
            _ => Err(ArgumentError::new(format!("invalid message type: {}",value)))
        }
    }
}

pub type Message = Box<dyn MessageOps>;
pub type MessageContent = Box<dyn MessageContentOps>;
pub type MessageBuilder = Box<dyn MessageBuilderOps>;
