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
use std::thread::JoinHandle;
use tokio::sync::mpsc;
use log::warn;

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

const CHANNEL_CLOSED: &str = "verticle event channel closed";

pub(crate) struct VerticleClient {
    event_tx: mpsc::UnboundedSender<Event>,
    handle: Option<JoinHandle<()>>,
}

impl VerticleClient {
    pub(super) fn new(event_tx: mpsc::UnboundedSender<Event>, handle: JoinHandle<()>) -> Self {
        Self {
            event_tx,
            handle: Some(handle),
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
            .map_err(|_| StateError::new(CHANNEL_CLOSED))?;

        future.await.map_err(|msg|
            StateError::new(msg).into()
        )
    }

    pub(crate) async fn start(&self) -> Result<()> {
        let make_event = |promise| {
            Event::Start(NoParams {promise})
        };
        self.call(make_event).await
    }

    pub(crate) async fn stop(&mut self) -> Result<()> {
        let make_event = |promise| {
            Event::Stop(NoParams {promise})
        };
        let result = self.call(make_event).await;

        let Some(handle) = self.handle.take() else  {
            return result;
        };

        let _ = handle.join().map_err(|e| {
            warn!("Joining messaging verticle task failed: {:?}", e);
        });

        Ok(())
    }

    pub(crate) async fn get_conversation(&self, id: Id) -> Result<Option<Conversation>> {
        let make_event = |promise| {
            Event::GetConversation(WithParams {
                data: id,
                promise,
            })
        };
        self.call(make_event).await
    }

    pub(crate) async fn get_conversations(&self) -> Result<Vec<Conversation>> {
        let make_event = |promise| {
            Event::GetConversations(NoParams { promise})
        };
        self.call(make_event).await
    }

    pub(crate) async fn remove_conversation(&self, id: Id) -> Result<()> {
        let make_event = |promise| {
            Event::RemoveConversation(WithParams {
                data: id,
                promise,
            })
        };
        self.call(make_event).await
    }

    pub(crate) async fn remove_conversations(&self, ids: Vec<Id>) -> Result<()> {
        let make_event = |promise| {
            Event::RemoveConversations(WithParams {
                data: (ids),
                promise,
            })
        };
        self.call(make_event).await
    }

    pub(crate) async fn get_messages(
        &self,
        conversation_id: Id,
        until: Option<i64>,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<Box<dyn Message>>> {
        let make_event = |promise| {
            Event::GetMessages(WithParams {
                data: (conversation_id, until, limit, offset),
                promise,
            })
        };
        self.call(make_event).await
    }

    pub(crate) async fn get_messages_in_range(
        &self,
        conversation_id: Id,
        begin: i64,
        end: i64,
    ) -> Result<Vec<Box<dyn Message>>> {
        let make_event = |promise| {
            Event::GetMessagesInRange(WithParams {
                data: (conversation_id, begin, end),
                promise,
            })
        };
        self.call(make_event).await
    }

    pub(crate) async fn remove_message(&self, message_id: i64) -> Result<()> {
        let make_event = |promise| {
            Event::RemoveMessage(WithParams {
                data: (message_id),
                promise,
            })
        };
        self.call(make_event).await
    }

    pub(crate) async fn remove_messages_by_ids(&self, message_ids: Vec<i64>) -> Result<()> {
        let make_event = |promise| {
            Event::RemoveMessagesByIds(WithParams {
                data: (message_ids),
                promise,
            })
        };
        self.call(make_event).await
    }

    pub(crate) async fn remove_messages_in_conversation(&self, conversation_id: Id) -> Result<()> {
        let make_event = |promise| {
            Event::RemoveMessagesInConversation(WithParams {
                data: conversation_id,
                promise,
            })
        };
        self.call(make_event).await
    }

    pub(crate) async fn get_sessions(&self) -> Result<Vec<crate::session_info::SessionInfo>> {
        let make_event = |promise| {
            Event::GetSessions(NoParams { promise})
        };
        self.call(make_event).await
    }

    pub(crate) async fn revoke_session(&self, device_id: Id) -> Result<()> {
        let make_event = |promise| {
            Event::RevokeSession(WithParams {
                data: device_id,
                promise,
            })
        };
        self.call(make_event).await
    }

    pub(crate) async fn friend_request(&self, user_id: Id, hello: Option<String>) -> Result<()> {
        let make_event = |promise| {
            Event::FriendRequest(WithParams {
                data: (user_id, hello),
                promise,
            })
        };
        self.call(make_event).await
    }

    pub(crate) async fn friend_accept(&self, user_id: Id) -> Result<()> {
        let make_event = |promise| {
            Event::FriendAccept(WithParams {
                data: user_id,
                promise,
            })
        };
        self.call(make_event).await
    }

    pub(crate) async fn get_friend_request(&self, user_id: Id) -> Result<Option<FriendRequest>> {
        let make_event = |promise| {
            Event::GetFriendRequest(WithParams {
                data: user_id,
                promise,
            })
        };
        self.call(make_event).await
    }

    pub(crate) async fn get_friend_requests(&self) -> Result<Vec<FriendRequest>> {
        let make_event = |promise| {
            Event::GetFriendRequests(NoParams { promise})
        };
        self.call(make_event).await
    }

    pub(crate) async fn remove_friend_request(&self, user_id: Id) -> Result<()> {
        let make_event = |promise| {
            Event::RemoveFriendRequest(WithParams {
                data: user_id,
                promise,
            })
        };
        self.call(make_event).await
    }

    pub(crate) async fn remove_friend_requests(&self, user_ids: Vec<Id>) -> Result<()> {
        let make_event = |promise| {
            Event::RemoveFriendRequests(WithParams {
                data: (user_ids),
                promise,
            })
        };
        self.call(make_event).await
    }

    pub(crate) async fn clear_friend_requests(&self) -> Result<()> {
        let make_event = |promise| {
            Event::ClearFriendRequests(NoParams { promise})
        };
        self.call(make_event).await
    }

    pub(crate) async fn add_friend(
        &self,
        user_id: Id,
        session_key: Vec<u8>,
        remark: Option<String>,
    ) -> Result<()> {
        let make_event = |promise| {
            Event::RegisterFriendSession(WithParams {
                data: (user_id, session_key, remark),
                promise,
            })
        };
        self.call(make_event).await
    }

    pub(crate) async fn block_user(&self, user_id: Id) -> Result<Contact> {
        let make_event = |promise| {
            Event::BlockUser(WithParams {
                data: user_id,
                promise,
            })
        };
        self.call(make_event).await
    }

    pub(crate) async fn create_channel(
        &self,
        permission: Permission,
        name: String,
        notice: Option<String>,
        announcement: bool,
    ) -> Result<Box<Channel>> {
        let make_event = |promise| {
            Event::CreateChannel(WithParams {
                data: (permission, name, notice, announcement),
                promise,
            })
        };
        self.call(make_event).await
    }

    pub(crate) async fn remove_channel(&self, channel_id: Id) -> Result<()> {
        let make_event = |promise| {
            Event::RemoveChannel(WithParams {
                data: channel_id,
                promise,
            })
        };
        self.call(make_event).await
    }

    pub(crate) async fn join_channel(&self, ticket: InviteTicket) -> Result<Box<Channel>> {
        let make_event = |promise| {
            Event::JoinChannel(WithParams {
                data: (ticket),
                promise,
            })
        };
        self.call(make_event).await
    }

    pub(crate) async fn leave_channel(&self, channel_id: Id) -> Result<()> {
        let make_event = |promise| {
            Event::LeaveChannel(WithParams {
                data: channel_id,
                promise,
            })
        };
        self.call(make_event).await
    }

    pub(crate) async fn create_invite_ticket(
        &self,
        channel_id: Id,
        invitee: Option<Id>,
    ) -> Result<InviteTicket> {
        let make_event = |promise| {
            Event::CreateInviteTicket(WithParams {
                data: (channel_id, invitee),
                promise,
            })
        };
        self.call(make_event).await
    }

    pub(crate) async fn transfer_channel_ownership(
        &self,
        channel_id: Id,
        new_owner: Id,
    ) -> Result<()> {
        let make_event = |promise| {
            Event::TransferChannelOwnership(WithParams {
                data: (channel_id, new_owner),
                promise,
            })
        };
        self.call(make_event).await
    }

    pub(crate) async fn rotate_channel_session_key(&self, channel_id: Id) -> Result<()> {
        let make_event = |promise| {
            Event::RotateChannelSessionKey(WithParams {
                data: channel_id,
                promise,
            })
        };
        self.call(make_event).await
    }

    pub(crate) async fn update_channel_info(&self, channel: Box<Channel>) -> Result<()> {
        let make_event = |promise| {
            Event::UpdateChannelInfo(WithParams {
                data: (channel),
                promise,
            })
        };
        self.call(make_event).await
    }

    pub(crate) async fn set_channel_members_role(
        &self,
        channel_id: Id,
        member_ids: Vec<Id>,
        role: Role,
    ) -> Result<()> {
        let make_event = |promise| {
            Event::SetChannelMemberRole(WithParams {
                data: (channel_id, member_ids, role),
                promise,
            })
        };
        self.call(make_event).await
    }

    pub(crate) async fn ban_channel_members(
        &self,
        channel_id: Id,
        member_ids: Vec<Id>,
    ) -> Result<()> {
        let make_event = |promise| {
            Event::BanChannelMembers(WithParams {
                data: (channel_id, member_ids),
                promise,
            })
        };
        self.call(make_event).await
    }

    pub(crate) async fn unban_channel_members(
        &self,
        channel_id: Id,
        member_ids: Vec<Id>,
    ) -> Result<()> {
        let make_event = |promise| {
            Event::UnbanChannelMembers(WithParams {
                data: (channel_id, member_ids),
                promise,
            })
        };
        self.call(make_event).await
    }

    pub(crate) async fn remove_channel_members(
        &self,
        channel_id: Id,
        members: Vec<Id>,
    ) -> Result<()> {
        let make_event = |promise| {
            Event::RemoveChannelMembers(WithParams {
                data: (channel_id, members),
                promise,
            })
        };
        self.call(make_event).await
    }

    pub(crate) async fn get_contact(&self, id: Id) -> Result<Option<Contact>> {
        let make_event = |promise| {
            Event::GetContact(WithParams {
                data: id,
                promise,
            })
        };
        self.call(make_event).await
    }

    pub(crate) async fn get_contacts(&self) -> Result<Vec<Contact>> {
        let make_event = |promise| {
            Event::GetContacts(NoParams { promise })
        };
        self.call(make_event).await
    }

    pub(crate) async fn update_contact(&self, contact: Contact) -> Result<()> {
        let make_event = |promise| {
            Event::UpdateContact(WithParams {
                data: (contact),
                promise,
            })
        };
        self.call(make_event).await
    }

    pub(crate) async fn remove_contacts(&self, ids: Vec<Id>) -> Result<()> {
        let make_event = |promise| {
            Event::RemoveContacts(WithParams {
                data: (ids),
                promise,
            })
        };
        self.call(make_event).await
    }

    pub(crate) async fn clear_contacts(&self) -> Result<()> {
        let make_event = |promise| {
            Event::ClearContacts(NoParams { promise })
        };
        self.call(make_event).await
    }
}
