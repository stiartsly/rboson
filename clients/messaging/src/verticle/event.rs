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
    channel::{Channel, Permission, Role},
    contact::Contact,
    conversation::Conversation,
    friend_request::FriendRequest,
    invite_ticket::InviteTicket,
    message::Message,
    session_info::SessionInfo,
};
use super::promise::Promise;

pub(crate) struct Body<Input, Output> {
    pub(crate) params: Input,
    pub(crate) result: Promise<Output>,
}

impl<Input, Output> Body<Input, Output> {
    pub(crate) fn new(params: Input, result: Promise<Output>) -> Self {
        Self { params, result }
    }
}

pub(crate) enum Event {
    Start(Body<(), ()>),
    Stop(Body<(), ()>),
    GetConversations(Body<Option<Id>, Vec<Conversation>>),
    RemoveConversations(Body<Vec<Id>, ()>),
    GetMessages(Body<MsgPaginationParams, Vec<Message>>),
    RemoveMessages(Body<MsgIdsParams, ()>),
    GetSessions(Body<(), Vec<SessionInfo>>),
    RevokeSession(Body<Id, ()>),
    FriendRequest(Body<FriendRequestParams, ()>),
    FriendAccept(Body<Id, ()>),
    GetFriendRequests(Body<Option<Id>, Vec<FriendRequest>>),
    RemoveFriendRequests(Body<Vec<Id>, ()>),
    ContentMessage(Body<MsgContentParams, Message>),
    RegisterFriendSession(Body<AddFriendParams, ()>),
    BlockUser(Body<Id, Contact>),
    CreateChannel(Body<NewChannelParams, Box<Channel>>),
    RemoveChannel(Body<Id, ()>),
    JoinChannel(Body<InviteTicket, Box<Channel>>),
    LeaveChannel(Body<Id, ()>),
    CreateInviteTicket(Body<(Id, Option<Id>), InviteTicket>),
    TransferChannelOwnership(Body<(Id, Id), ()>),
    RotateChannelSessionKey(Body<Id, ()>),
    UpdateChannelInfo(Body<Channel, ()>),
    OperateChannelMembers(Body<OperateChannelMembersParams, ()>),
    GetContacts(Body<Option<Id>, Vec<Contact>>),
    UpdateContact(Body<Contact, ()>),
    RemoveContacts(Body<Option<Vec<Id>>, ()>),
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

// Identifiers for messages to be removed
pub(crate) struct MsgIdsParams {
    pub(crate) conversation_id: Option<Id>,
    pub(crate) messages: Option<Vec<i64>>,
}

// Parameters for sending a content message
pub(crate) struct MsgContentParams {
    pub(crate) conversation_id: Id,
    pub(crate) metadata: HashMap<String, serde_json::Value>,
    pub(crate) content: Vec<u8>,
    pub(crate) is_encrypted: bool,
}

pub(crate) struct FriendRequestParams {
    pub(crate) user_id: Id,
    pub(crate) hello: Option<String>,
}

pub(crate) struct AddFriendParams {
    pub(crate) user_id: Id,
    pub(crate) session_key: Vec<u8>,
    pub(crate) remark: Option<String>,
}

pub(crate) struct NewChannelParams {
    pub(crate) permission: Permission,
    pub(crate) name: String,
    pub(crate) notice: Option<String>,
    pub(crate) announcement: bool,
}

pub(crate) const OPERATE_SET_MEMBER_ROLE: u8 = 0;
pub(crate) const OPERATE_BAN_MEMBERS: u8 = 1;
pub(crate) const OPERATE_UNBAN_MEMBERS: u8 = 2;
pub(crate) const OPERATE_REMOVE_MEMBERS: u8 = 3;

pub(crate) struct OperateChannelMembersParams {
    pub(crate) operation: u8,
    pub(crate) channel_id: Id,
    pub(crate) member_ids: Vec<Id>,
    pub(crate) role: Option<Role>,
}
