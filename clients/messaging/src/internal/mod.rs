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

pub(crate) mod serde;
pub(crate) mod handshake;
pub(crate) mod notification;
pub(crate) mod photon_content;
pub(crate) mod contact_data;
pub(crate) mod contact_mutation;
pub(crate) mod contact_sync;
pub(crate) mod contact_opaque;
pub(crate) mod topic;
pub(crate) mod channel_info;
pub(crate) mod channel_info_update;
pub(crate) mod channel_members_role;
pub(crate) mod channel_session_rotation;
pub(crate) mod new_channel_info;
pub(crate) mod join_channel;
pub(crate) mod session_context;
pub(crate) mod connection_listener;

pub(crate) mod photon_message;
pub(crate) mod message_builder;

pub(crate) mod photon_channel;
pub(crate) mod photon_contact;
pub(crate) mod photon_conversation;
pub(crate) mod channel_member;

pub(crate) mod messaging_repository;
pub(crate) mod transport;
pub(crate) mod verticle;

pub(crate) use {
    handshake::{
        Handshake,
        HandshakeType,
    },
    notification::{
        Body as NotificationBody,
        Event as NotificationEvent,
        Notification,
    },

    channel_session_rotation::SessionKeyRotation as ChannelSessionKeyRotation,
    new_channel_info::NewChannelInfo,
    channel_info::ChannelInfo,
    channel_members_role::ChannelMembersRole,
    channel_info_update::ChannelInfoUpdate,

    contact_data::ContactData,
    contact_sync::{ContactSync, SyncType},
    contact_mutation::{ContactMutation, MutationData},
    contact_opaque::OpaqueContact,

    join_channel::JoinChannel,

    photon_channel::PhotonChannel,
    photon_contact::PhotonContact,
    channel_member::PhotonChannelMember,
    photon_conversation::PhotonConversation,
    photon_message::PhotonMessage,
    photon_content::{ContentFormat, PhotonContent},

    connection_listener::ClientConnectionListener,
    message_builder::ClientMessageBuilder,

    session_context::SessionContext,
    serde as serde_utils,
    topic::Topic,
    messaging_repository::MessagingRepository
};
