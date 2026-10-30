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

pub(crate) struct NoParams<T> {
    pub(crate) promise: Promise<T>,
}
pub(crate) struct WithParams<D, T> {
    pub(crate) data: D,
    pub(crate) promise: Promise<T>,
}

pub(crate) enum Event {
    Start(NoParams<()>),
    Stop(NoParams<()>),
    GetConversation(WithParams<Id, Option<Conversation>>),
    GetConversations(NoParams<Vec<Conversation>>),
    RemoveConversation(WithParams<Id, ()>),
    RemoveConversations(WithParams<Vec<Id>, ()>),
    GetMessages(WithParams<(Id, Option<i64>, usize, usize), Vec<Box<dyn Message>>>),
    GetMessagesInRange(WithParams<(Id, i64, i64), Vec<Box<dyn Message>>>),
    RemoveMessage(WithParams<i64, ()>),
    RemoveMessagesByIds(WithParams<Vec<i64>, ()>),
    RemoveMessagesInConversation(WithParams<Id, ()>),
    GetSessions(NoParams<Vec<SessionInfo>>),
    RevokeSession(WithParams<Id, ()>),
    FriendRequest(WithParams<(Id, Option<String>), ()>),
    FriendAccept(WithParams<Id, ()>),
    GetFriendRequest(WithParams<Id, Option<FriendRequest>>),
    GetFriendRequests(NoParams<Vec<FriendRequest>>),
    RemoveFriendRequest(WithParams<Id, ()>),
    RemoveFriendRequests(WithParams<Vec<Id>, ()>),
    ClearFriendRequests(NoParams<()>),
    ContentMessage(
        WithParams<
            (
                Id,
                std::collections::HashMap<String, serde_json::Value>,
                Vec<u8>,
                bool,
            ),
            Box<dyn Message>,
        >,
    ),
    RegisterFriendSession(WithParams<(Id, Vec<u8>, Option<String>), ()>),
    BlockUser(WithParams<Id, Contact>),
    CreateChannel(WithParams<(Permission, String, Option<String>, bool), Box<Channel>>),
    RemoveChannel(WithParams<Id, ()>),
    JoinChannel(WithParams<InviteTicket, Box<Channel>>),
    LeaveChannel(WithParams<Id, ()>),
    CreateInviteTicket(WithParams<(Id, Option<Id>), InviteTicket>),
    TransferChannelOwnership(WithParams<(Id, Id), ()>),
    RotateChannelSessionKey(WithParams<Id, ()>),
    UpdateChannelInfo(WithParams<Box<Channel>, ()>),
    SetChannelMemberRole(WithParams<(Id, Vec<Id>, Role), ()>),
    BanChannelMembers(WithParams<(Id, Vec<Id>), ()>),
    UnbanChannelMembers(WithParams<(Id, Vec<Id>), ()>),
    RemoveChannelMembers(WithParams<(Id, Vec<Id>), ()>),
    GetContact(WithParams<Id, Option<Contact>>),
    GetContacts(NoParams<Vec<Contact>>),
    UpdateContact(WithParams<Contact, ()>),
    RemoveContacts(WithParams<Vec<Id>, ()>),
    ClearContacts(NoParams<()>),
}
