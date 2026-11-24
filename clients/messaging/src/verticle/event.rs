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

use std::collections::HashMap;
use boson_api::Id;
use crate::{
    channel::Channel,
    contact::Contact,
    InviteTicket,
    message::Message,
    SessionInfo,
};
use crate::internal::{
    ChannelMembersRole,
    PhotonChannel,
    PhotonConversation,
    PhotonFriendRequest,
    PhotonContact,
    NewChannelInfo,
};
use super::promise::Promise;

pub(crate) struct Body<I,O>(pub(crate) I, pub(crate) Promise<O>);

pub(crate) enum Event {
    Start(Body<(), ()>),
    Stop(Body<(), ()>),

    GetConversation(Body<Id, Option<PhotonConversation>>),
    GetConversationList(Body<(), Vec<PhotonConversation>>),
    RemoveConversation(Body<Id, ()>),
    RemoveConversationList(Body<Vec<Id>, ()>),

    GetMessages(Body<MsgPaginationParams, Vec<Message>>),
    RemoveMessageList(Body<Vec<i64>, ()>),
    RemoveMessagesByConversation(Body<Id, ()>),

    GetSessions(Body<(), Vec<SessionInfo>>),
    RevokeSession(Body<Id, ()>),

    FriendRequest(Body<(Id, Option<String>), ()>),
    FriendAccept(Body<Id, ()>),
    GetFriendRequests(Body<Option<Id>, Vec<PhotonFriendRequest>>),
    RemoveFriendRequests(Body<Vec<Id>, ()>),

    ContentMessage(Body<MsgContentParams, Message>),
    RegisterFriendSession(Body<(Id, Vec<u8>, Option<String>), ()>),
    BlockUser(Body<Id, Contact>),

    CreateChannel(Body<NewChannelInfo, PhotonChannel>),
    RemoveChannel(Body<Id, ()>),
    JoinChannel(Body<InviteTicket, PhotonChannel>),
    LeaveChannel(Body<Id, ()>),

    CreateInviteTicket(Body<(Id, Option<Id>), InviteTicket>),
    TransferChannelOwnership(Body<(Id, Id), ()>),
    RotateChannelSessionKey(Body<(Id, Option<Vec<u8>>), ()>),

    UpdateChannelInfo(Body<Channel, ()>),
    SetChannelMembersRole(Body<(Id, ChannelMembersRole), ()>),
    BanChannelMembers(Body<(Id, Vec<Id>), ()>),
    UnbanChannelMembers(Body<(Id, Vec<Id>), ()>),
    RemoveChannelMembers(Body<(Id, Vec<Id>), ()>),

    GetContacts(Body<Option<Id>, Vec<PhotonContact>>),
    UpdateContact(Body<PhotonContact, ()>),
    RemoveContacts(Body<Vec<Id>, ()>),
    ClearContacts(Body<(), ()>)
}

// Pagination for fetching messages within a conversation
pub(crate) struct MsgPaginationParams {
    pub(crate) conversation_id: Id,
    pub(crate) until: Option<i64>,
    pub(crate) limit: Option<usize>,
    pub(crate) offset: Option<usize>,
    pub(crate) begin: Option<i64>,
    pub(crate) end : Option<i64>,
}

// Parameters for sending a content message
pub(crate) struct MsgContentParams {
    pub(crate) conversation_id: Id,
    pub(crate) metadata: HashMap<String, serde_json::Value>,
    pub(crate) content: Vec<u8>,
    pub(crate) is_encrypted: bool,
}
