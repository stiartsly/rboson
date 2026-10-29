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
    sync::{Arc, Mutex},
    thread::JoinHandle,
};

use tokio::{
    sync::mpsc,
    task,
};

use boson_api::{
    Id,
    errors::{Result, StateError},
};

use crate::{
    channel::{Permission, Role, Channel},
    contact::Contact,
    conversation::Conversation,
    friend_request::FriendRequest,
    invite_ticket::InviteTicket,
    message::Message,
};

use super::event::{Event, NoParams, WithParams};
use super::promise::Promise;

const CHANNEL_REQ_CLOSED: &str = "verticle request channel closed";
const CHANNEL_RSP_CLOSED: &str = "verticle response channel closed";

#[derive(Clone)]
pub(crate) struct VerticleClient {
    event_tx: mpsc::UnboundedSender<Event>,
    handle: Arc<Mutex<Option<JoinHandle<()>>>>,
}

impl VerticleClient {
    pub(super) fn new(event_tx: mpsc::UnboundedSender<Event>, handle: JoinHandle<()>) -> Self {
        Self {
            event_tx,
            handle: Arc::new(Mutex::new(Some(handle))),
        }
    }

    pub(crate) fn sender(&self) -> mpsc::UnboundedSender<Event> {
        self.event_tx.clone()
    }

    async fn call<T>(
        &self,
        make_event: impl FnOnce(Promise<T>) -> Event,
    ) -> Result<T> {
        let (promise, future) = Promise::pair();
        self.event_tx
            .send(make_event(promise))
            .map_err(|_| StateError::new(CHANNEL_REQ_CLOSED))?;

        future.await.map_err(|msg| StateError::new(msg).into())
    }

    pub(crate) async fn start(&self) -> Result<()> {
        self.call(|promise| {
            Event::Start(NoParams {promise})
        })
        .await
    }

    pub(crate) async fn stop(&self) -> Result<()> {
        let result = self
            .call(|promise| {
                Event::Stop(NoParams {promise})
            })
            .await;

        // A poisoned lock still holds a usable handle; never panic on shutdown.
        let handle = self
            .handle
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .take();
        if let Some(handle) = handle {
            let joined = task::spawn_blocking(move || handle.join())
                .await
                .map_err(|error| {
                    StateError::new(format!("Joining messaging verticle task failed: {error}"))
                })?;
            joined.map_err(|_| StateError::new("Messaging verticle thread panicked"))?;
        }
        result
    }

    pub(crate) async fn get_conversation(&self, id: Id) -> Result<Option<Conversation>> {
        self.call(|promise| {
            Event::GetConversation(WithParams {
                data: id,
                promise,
            })
        })
        .await
    }

    pub(crate) async fn get_conversations(&self) -> Result<Vec<Conversation>> {
        self.call(|promise| {
            Event::GetConversations(NoParams { promise})
        })
        .await
    }

    pub(crate) async fn remove_conversation(&self, id: Id) -> Result<()> {
        self.call(|promise| {
            Event::RemoveConversation(WithParams {
                data: id,
                promise,
            })
        })
        .await
    }

    pub(crate) async fn remove_conversations(&self, ids: Vec<Id>) -> Result<()> {
        self.call(|promise| {
            Event::RemoveConversations(WithParams {
                data: (ids),
                promise,
            })
        })
        .await
    }

    pub(crate) async fn get_messages(
        &self,
        conversation_id: Id,
        until: Option<i64>,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<Box<dyn Message>>> {
        self.call(|promise| {
            Event::GetMessages(WithParams {
                data: (conversation_id, until, limit, offset),
                promise,
            })
        })
        .await
    }

    pub(crate) async fn get_messages_in_range(
        &self,
        conversation_id: Id,
        begin: i64,
        end: i64,
    ) -> Result<Vec<Box<dyn Message>>> {
        self.call(|promise| {
            Event::GetMessagesInRange(WithParams {
                data: (conversation_id, begin, end),
                promise,
            })
        })
        .await
    }

    pub(crate) async fn remove_message(&self, message_id: i64) -> Result<()> {
        self.call(|promise| {
            Event::RemoveMessage(WithParams {
                data: (message_id),
                promise,
            })
        })
        .await
    }

    pub(crate) async fn remove_messages_by_ids(&self, message_ids: Vec<i64>) -> Result<()> {
        self.call(|promise| {
            Event::RemoveMessagesByIds(WithParams {
                data: (message_ids),
                promise,
            })
        })
        .await
    }

    pub(crate) async fn remove_messages_in_conversation(&self, conversation_id: Id) -> Result<()> {
        self.call(|promise| {
            Event::RemoveMessagesInConversation(WithParams {
                data: conversation_id,
                promise,
            })
        })
        .await
    }

    pub(crate) async fn get_sessions(&self) -> Result<Vec<crate::session_info::SessionInfo>> {
        self.call(|promise| {
            Event::GetSessions(NoParams { promise})
        })
        .await
    }

    pub(crate) async fn revoke_session(&self, device_id: Id) -> Result<()> {
        self.call(|promise| {
            Event::RevokeSession(WithParams {
                data: device_id,
                promise,
            })
        })
        .await
    }

    pub(crate) async fn friend_request(&self, user_id: Id, hello: Option<String>) -> Result<()> {
        self.call(|promise| {
            Event::FriendRequest(WithParams {
                data: (user_id, hello),
                promise,
            })
        })
        .await
    }

    pub(crate) async fn friend_accept(&self, user_id: Id) -> Result<()> {
        self.call(|promise| {
            Event::FriendAccept(WithParams {
                data: user_id,
                promise,
            })
        })
        .await
    }

    pub(crate) async fn get_friend_request(&self, user_id: Id) -> Result<Option<FriendRequest>> {
        self.call(|promise| {
            Event::GetFriendRequest(WithParams {
                data: user_id,
                promise,
            })
        })
        .await
    }

    pub(crate) async fn get_friend_requests(&self) -> Result<Vec<FriendRequest>> {
        self.call(|promise| {
            Event::GetFriendRequests(NoParams { promise})
        })
        .await
    }

    pub(crate) async fn remove_friend_request(&self, user_id: Id) -> Result<()> {
        self.call(|promise| {
            Event::RemoveFriendRequest(WithParams {
                data: user_id,
                promise,
            })
        })
        .await
    }

    pub(crate) async fn remove_friend_requests(&self, user_ids: Vec<Id>) -> Result<()> {
        self.call(|promise| {
            Event::RemoveFriendRequests(WithParams {
                data: (user_ids),
                promise,
            })
        })
        .await
    }

    pub(crate) async fn clear_friend_requests(&self) -> Result<()> {
        self.call(|promise| {
            Event::ClearFriendRequests(NoParams { promise})
        })
        .await
    }

    pub(crate) async fn add_friend(
        &self,
        user_id: Id,
        session_key: Vec<u8>,
        remark: Option<String>,
    ) -> Result<()> {
        self.call(|promise| {
            Event::RegisterFriendSession(WithParams {
                data: (user_id, session_key, remark),
                promise,
            })
        })
        .await
    }

    pub(crate) async fn block_user(&self, user_id: Id) -> Result<Contact> {
        self.call(|promise| {
            Event::BlockUser(WithParams {
                data: user_id,
                promise,
            })
        })
        .await
    }

    pub(crate) async fn create_channel(
        &self,
        permission: Permission,
        name: String,
        notice: Option<String>,
        announcement: bool,
    ) -> Result<Box<Channel>> {
        self.call(|promise| {
            Event::CreateChannel(WithParams {
                data: (permission, name, notice, announcement),
                promise,
            })
        })
        .await
    }

    pub(crate) async fn remove_channel(&self, channel_id: Id) -> Result<()> {
        self.call(|promise| {
            Event::RemoveChannel(WithParams {
                data: channel_id,
                promise,
            })
        })
        .await
    }

    pub(crate) async fn join_channel(&self, ticket: InviteTicket) -> Result<Box<Channel>> {
        self.call(|promise| {
            Event::JoinChannel(WithParams {
                data: (ticket),
                promise,
            })
        })
        .await
    }

    pub(crate) async fn leave_channel(&self, channel_id: Id) -> Result<()> {
        self.call(|promise| {
            Event::LeaveChannel(WithParams {
                data: channel_id,
                promise,
            })
        })
        .await
    }

    pub(crate) async fn create_invite_ticket(
        &self,
        channel_id: Id,
        invitee: Option<Id>,
    ) -> Result<InviteTicket> {
        self.call(|promise| {
            Event::CreateInviteTicket(WithParams {
                data: (channel_id, invitee),
                promise,
            })
        })
        .await
    }

    pub(crate) async fn transfer_channel_ownership(
        &self,
        channel_id: Id,
        new_owner: Id,
    ) -> Result<()> {
        self.call(|promise| {
            Event::TransferChannelOwnership(WithParams {
                data: (channel_id, new_owner),
                promise,
            })
        })
        .await
    }

    pub(crate) async fn rotate_channel_session_key(&self, channel_id: Id) -> Result<()> {
        self.call(|promise| {
            Event::RotateChannelSessionKey(WithParams {
                data: channel_id,
                promise,
            })
        })
        .await
    }

    pub(crate) async fn update_channel_info(&self, channel: Box<Channel>) -> Result<()> {
        self.call(|promise| {
            Event::UpdateChannelInfo(WithParams {
                data: (channel),
                promise,
            })
        })
        .await
    }

    pub(crate) async fn set_channel_members_role(
        &self,
        channel_id: Id,
        member_ids: Vec<Id>,
        role: Role,
    ) -> Result<()> {
        self.call(|promise| {
            Event::SetChannelMemberRole(WithParams {
                data: (channel_id, member_ids, role),
                promise,
            })
        })
        .await
    }

    pub(crate) async fn ban_channel_members(
        &self,
        channel_id: Id,
        member_ids: Vec<Id>,
    ) -> Result<()> {
        self.call(|promise| {
            Event::BanChannelMembers(WithParams {
                data: (channel_id, member_ids),
                promise,
            })
        })
        .await
    }

    pub(crate) async fn unban_channel_members(
        &self,
        channel_id: Id,
        member_ids: Vec<Id>,
    ) -> Result<()> {
        self.call(|promise| {
            Event::UnbanChannelMembers(WithParams {
                data: (channel_id, member_ids),
                promise,
            })
        })
        .await
    }

    pub(crate) async fn remove_channel_members(
        &self,
        channel_id: Id,
        members: Vec<Id>,
    ) -> Result<()> {
        self.call(|promise| {
            Event::RemoveChannelMembers(WithParams {
                data: (channel_id, members),
                promise,
            })
        })
        .await
    }

    pub(crate) async fn get_contact(&self, id: Id) -> Result<Option<Contact>> {
        self.call(|promise| {
            Event::GetContact(WithParams {
                data: id,
                promise,
            })
        })
        .await
    }

    pub(crate) async fn get_contacts(&self) -> Result<Vec<Contact>> {
        self.call(|promise| {
            Event::GetContacts(NoParams { promise })
        })
        .await
    }

    pub(crate) async fn update_contact(&self, contact: Contact) -> Result<()> {
        self.call(|promise| {
            Event::UpdateContact(WithParams {
                data: (contact),
                promise,
            })
        })
        .await
    }

    pub(crate) async fn remove_contacts(&self, ids: Vec<Id>) -> Result<()> {
        self.call(|promise| {
            Event::RemoveContacts(WithParams {
                data: (ids),
                promise,
            })
        })
        .await
    }

    pub(crate) async fn clear_contacts(&self) -> Result<()> {
        self.call(|promise| {
            Event::ClearContacts(NoParams { promise })
        })
        .await
    }
}
