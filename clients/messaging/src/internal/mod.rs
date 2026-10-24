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

mod codec;

pub(crate) mod photon_channel;
pub(crate) mod handshake;
pub(crate) mod notification;
pub(crate) mod photon_message;
pub(crate) mod message_content;
pub(crate) mod contac_data;
pub(crate) mod contact_mutation;
pub(crate) mod contact_sync;
pub(crate) mod contact_opaque;
pub(crate) mod topic;
pub(crate) mod channel_member;
pub(crate) mod channel_info;
pub(crate) mod channel_members_role;
pub(crate) mod channel_session_rotation;
pub(crate) mod new_channel_info;

pub(crate) mod sessoin_context;

pub(crate) mod connection_listener;

mod channel {
    pub(crate) use super::{
        channel_info::ChannelInfo,
        channel_member::ChannelMember,
        channel_members_role::ChannelMembersRole,
    };
}

pub(crate) mod contact {
    pub(crate) use super::{
        contac_data::ContactData,
        contact_sync::ContactSync,
    };
}

pub(crate) use {
    photon_channel::{PhotonChannel, PhotonChannelMember},
    photon_message::PhotonMessage,
    message_content::{ContentFormat, MessageContent},
    handshake::{Handshake, HandshakeType},
    notification::{Body as NotificationBody, Event as NotificationEvent, Notification},
    contac_data::ContactData,
    contact_mutation::{ContactMutation, MutationData},
    contact_sync::{ContactSync, SyncType},
    contact_opaque::OpaqueContact,
    channel_member::ChannelMember,
    channel_info::ChannelInfo,
    channel_session_rotation::ChannelSessionKeyRotation,
    connection_listener::ClientConnectionListener,
    new_channel_info::NewChannelInfo,
    topic::Topic,
};
