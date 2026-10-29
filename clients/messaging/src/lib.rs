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

// Migrated code that is not documented yet. Remove this allow as the crate is brought in line
// with the workspace lints.
#![allow(missing_docs)]

pub(crate) mod internal;
pub(crate) mod rpc;
pub mod ops;

pub(crate) mod mqtt;
pub(crate) mod rpc_server;
pub(crate) mod verticle;

pub mod client;
pub mod options;
pub mod channel;
pub mod channel_member;
pub mod contact;
pub mod contact_editor;
pub mod errors;
pub mod conversation;
pub mod friend_request;
pub mod invite_ticket;
pub mod message;
pub mod content_type;
pub mod content_disposition;

pub mod session_info;
pub mod messaging_client;

pub mod channel_listener;
pub mod connection_listener;
pub mod contact_listener;
pub mod friend_request_listener;
pub mod message_listener;
pub mod session_listener;

pub use {
    messaging_client::MessagingClient,
    options::{Options, OptionsBuilder},
    client::{Client},
    friend_request::FriendRequest,
    friend_request_listener::FriendRequestListener,
    connection_listener::ConnectionListener,
    contact_listener::ContactListener,
    message_listener::MessageListener,
    session_listener::SessionListener,
    channel_listener::ChannelListener,
    channel::{Permission, Role},
    channel_member::ChannelMember,
    contact::{Contact, ContactType},
    contact_editor::ContactEditor,
    ops::{ChannelEditorOps, ChannelOps, ContactEditorOps, ContactOps, ConversationOps},
    invite_ticket::InviteTicket,
    session_info::SessionInfo,
};

pub use message::{
    Content, ContentDisposition, Type, Message, MessageBuilder,
    MessageType, CONTENT_DISPOSITION_HEADER,
};

pub(crate) use internal::ClientConnectionListener;

