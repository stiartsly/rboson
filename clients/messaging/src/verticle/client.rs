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
    signature
};

use crate::{
    channel::{Permission, Role, Channel},
    contact::Contact,
    invite_ticket::InviteTicket,
    Message,
    FriendRequestHandler,
};
use crate::internal::{
    ChannelMembersRole,
    PhotonConversation,
    PhotonFriendRequest,
    PhotonContact,
    NewChannelInfo
};
use super::{
    promise::Promise,
    event::{
        Event, Body,
        MsgPaginationParams
    }
};

const CHANNEL_CLOSED: &str = "verticle event channel closed";

pub(crate) struct VerticleClient {
    event_tx: mpsc::UnboundedSender<Event>,
    handle: Option<JoinHandle<()>>,
}

impl VerticleClient {
    pub(super) fn new(
        event_tx: mpsc::UnboundedSender<Event>,
        handle: JoinHandle<()>
    ) -> Self {
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
        self.call(|promise| {
            Event::Start(Body((), promise))
        }).await
    }

    pub(crate) async fn stop(&mut self) -> Result<()> {
        let result = self.call(|promise| {
            Event::Stop(Body((), promise))
        }).await;

        let Some(handle) = self.handle.take() else  {
            return result;
        };

        let _ = handle.join().map_err(|e| {
            warn!("Joining messaging verticle task failed: {:?}", e);
        });

        Ok(())
    }

    pub(crate) async fn get_conversation(&self, id: Id) -> Result<Option<PhotonConversation>> {
        self.call(|promise| {
            Event::GetConversation(Body(id, promise))
        }).await
    }

    pub(crate) async fn get_conversation_list(&self) -> Result<Vec<PhotonConversation>> {
        self.call(|promise| {
            Event::GetConversationList(Body((), promise))
        }).await
    }

    pub(crate) async fn remove_conversation(&self, id: Id) -> Result<()> {
        self.call(|promise| {
            Event::RemoveConversation(Body(id, promise))
        }).await
    }

    pub(crate) async fn remove_conversation_list(&self, ids: Vec<Id>) -> Result<()> {
        self.call(|promise| {
            Event::RemoveConversationList(Body(ids, promise))
        }).await
    }

    pub(crate) async fn get_messages(
        &self,
        conversation_id: Id,
        until: Option<i64>,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<Message>> {
        self.call(|promise| {
            Event::GetMessages(Body(
                MsgPaginationParams {
                    conversation_id,
                    until,
                    limit: Some(limit),
                    offset: Some(offset),
                    begin: None,
                    end: None,
                },
                promise,
            ))
        }).await
    }

    pub(crate) async fn get_messages_in_range(
        &self,
        conversation_id: Id,
        begin: i64,
        end: i64,
    ) -> Result<Vec<Message>> {
        self.call(|promise| {
            Event::GetMessages(Body(
                MsgPaginationParams {
                    conversation_id,
                    until: None,
                    limit: None,
                    offset: None,
                    begin: Some(begin),
                    end: Some(end),
                },
                promise,
            ))
        }).await
    }

    pub(crate) async fn remove_messages(&self, message_ids: Vec<i64>) -> Result<()> {
        self.call(|promise| {
            Event::RemoveMessageList(Body(message_ids, promise))
        }).await
    }

    pub(crate) async fn remove_messages_by_conversation(&self, id: Id) -> Result<()> {
        self.call(|promise| {
            Event::RemoveMessagesByConversation(Body(id, promise))
        }).await
    }

    pub(crate) async fn get_sessions(&self) -> Result<Vec<crate::session_info::SessionInfo>> {
        self.call(|promise| {
            Event::GetSessions(Body((), promise))
        }).await
    }

    pub(crate) async fn revoke_session(&self, device_id: Id) -> Result<()> {
        self.call(|promise| {
            Event::RevokeSession(Body(device_id, promise))
        }).await
    }

    pub(crate) async fn friend_request(&self, user_id: Id, hello: Option<String>) -> Result<()> {
        self.call(|promise| {
            Event::FriendRequest(Body((user_id, hello), promise))
        }).await
    }

    pub(crate) async fn friend_accept(&self, user_id: Id) -> Result<()> {
        self.call(|promise| {
            Event::FriendAccept(Body(user_id, promise))
        }).await
    }

    pub(crate) async fn get_friend_request(&self, user_id: Id) -> Result<Option<PhotonFriendRequest>> {
        let requests = self.call(|promise| {
            Event::GetFriendRequests(Body(Some(user_id), promise))
        }).await?;
        Ok(requests.into_iter().next())
    }

    pub(crate) async fn get_friend_requests(&self) -> Result<Vec<PhotonFriendRequest>> {
        self.call(|promise| {
            Event::GetFriendRequests(Body(None, promise))
        }).await
    }

    pub(crate) async fn remove_friend_request(&self, user_id: Id) -> Result<()> {
        self.remove_friend_requests(vec![user_id]).await
    }

    pub(crate) async fn remove_friend_requests(&self, user_ids: Vec<Id>) -> Result<()> {
        self.call(|promise| {
            Event::RemoveFriendRequests(Body(user_ids, promise))
        }).await
    }

    pub(crate) async fn clear_friend_requests(&self) -> Result<()> {
        let requests = self.get_friend_requests().await?;
        if requests.is_empty() {
            return Ok(());
        }
        let user_ids = requests.iter().map(|request| *request.user_id()).collect();
        self.remove_friend_requests(user_ids).await
    }

    pub(crate) async fn add_friend(
        &self,
        user_id: Id,
        session_key: Vec<u8>,
        remark: Option<String>,
    ) -> Result<()> {
        self.call(|promise| {
            Event::RegisterFriendSession(Body((user_id, session_key, remark), promise))
        }).await
    }

    pub(crate) async fn block_user(&self, user_id: Id) -> Result<Contact> {
        self.call(|promise| {
            Event::BlockUser(Body(user_id, promise))
        }).await
    }

    pub(crate) async fn create_channel(
        &self,
        permission: Permission,
        name: String,
        notice: Option<String>,
        announcement: bool,
    ) -> Result<Channel> {
        let session_key = signature::KeyPair::random();
        let session_id = Id::from(session_key.public_key());
        let session_sk = session_key.private_key().as_bytes().to_vec();
        let new_ci = NewChannelInfo::new(
            session_id,
            session_sk,
            permission,
            Some(name),
            notice,
            announcement
        );
        self.call(|promise| {
            Event::CreateChannel(Body(new_ci, promise))
        }).await
        .map(Into::into)
    }

    pub(crate) async fn remove_channel(&self, channel_id: Id) -> Result<()> {
        self.call(|promise| {
            Event::RemoveChannel(Body(channel_id, promise))
        }).await
    }

    pub(crate) async fn join_channel(&self, ticket: InviteTicket) -> Result<Channel> {
        self.call(|promise| {
            Event::JoinChannel(Body(ticket, promise))
        }).await
        .map(Into::into)
    }

    pub(crate) async fn leave_channel(&self, channel_id: Id) -> Result<()> {
        self.call(|promise| {
            Event::LeaveChannel(Body(channel_id, promise))
        }).await
    }

    pub(crate) async fn create_invite_ticket(&self, id: Id, invitee: Option<Id>) -> Result<InviteTicket> {
        self.call(|promise| {
            Event::CreateInviteTicket(Body((id, invitee), promise))
        }).await
    }

    pub(crate) async fn transfer_channel_ownership(&self, id: Id, owner: Id) -> Result<()> {
        self.call(|promise| {
            Event::TransferChannelOwnership(Body((id, owner), promise))
        }).await
    }

    pub(crate) async fn rotate_channel_session_key(&self, id: Id, new_key: Option<Vec<u8>>) -> Result<()> {
        self.call(|promise| {
            Event::RotateChannelSessionKey(Body((id, new_key), promise))
        }).await
    }

    pub(crate) async fn update_channel_info(&self, channel: Channel) -> Result<()> {
        self.call(|promise| {
            Event::UpdateChannelInfo(Body(channel, promise))
        }).await
    }

    pub(crate) async fn set_channel_members_role(&self, id: Id, ids: Vec<Id>, role: Role) -> Result<()> {
        let cmr = ChannelMembersRole::new(ids, role);
        self.call(|promise| {
            Event::SetChannelMembersRole(Body((id, cmr), promise))
        }).await
    }

    pub(crate) async fn ban_channel_members(&self, id: Id, ids: Vec<Id>) ->Result<()> {
        self.call(|promise| {
            Event::BanChannelMembers(Body((id, ids), promise))
        }).await
    }

    pub(crate) async fn unban_channel_members(&self, id: Id, ids: Vec<Id>) ->Result<()> {
        self.call(|promise| {
            Event::UnbanChannelMembers(Body((id, ids), promise))
        }).await
    }

    pub(crate) async fn remove_channel_members(&self, id: Id, ids: Vec<Id>) ->Result<()> {
        self.call(|promise| {
            Event::RemoveChannelMembers(Body((id, ids), promise))
        }).await
    }

    pub(crate) async fn get_contact(&self, id: &Id) -> Result<Option<PhotonContact>> {
        let contacts = self.call(|promise| {
            Event::GetContacts(Body(Some(*id), promise))
        }).await?;
        Ok(contacts.into_iter().next())
    }

    pub(crate) async fn get_contacts(&self) -> Result<Vec<PhotonContact>> {
        self.call(|promise| {
            Event::GetContacts(Body(None, promise))
        }).await
    }

    pub(crate) async fn update_contact(&self, contact: PhotonContact) -> Result<()> {
        self.call(|promise| {
            Event::UpdateContact(Body(contact, promise))
        }).await
    }

    pub(crate) async fn remove_contacts(&self, ids: Vec<Id>) -> Result<()> {
        self.call(|promise| {
            Event::RemoveContacts(Body(ids, promise))
        }).await
    }

    pub(crate) async fn clear_contacts(&self) -> Result<()> {
        self.call(|promise| {
            Event::ClearContacts(Body((), promise))
        }).await
    }
}
