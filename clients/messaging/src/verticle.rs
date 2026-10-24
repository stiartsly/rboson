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

use log::{debug, error};
use std::{
    rc::Rc,
    pin::Pin,
    future::Future,
    result::Result as StdResult,
    collections::HashMap,
    thread::JoinHandle,
    sync::{
        mpsc as std_mpsc,
        Arc,
        Mutex,
    },
};
use futures::{
    stream::{FuturesUnordered, StreamExt},
    FutureExt,
};
use tokio::{
    runtime,
    sync::{mpsc, oneshot},
    task,
};

use boson_api::Id;
use boson_api::errors::{Result, StateError};
use crate::{
    options::Options,
    contact::Contact,
    message::Message,
    conversation::Conversation,
    friend_request_listener::FriendRequestListener,
    connection_listener::ConnectionListener,
    channel_listener::ChannelListener,
    contact_listener::ContactListener,
    message_listener::MessageListener,
    session_listener::SessionListener,
    rpc_server::RpcServer,
    FriendRequest,
    InviteTicket,
    Channel,
    Permission,
    Role,
};

const CHANNEL_REQ_CLOSED: &str = "verticle request channel closed";
const CHANNEL_RSP_CLOSED: &str = "verticle response channel closed";

#[derive(Clone)]
pub(crate) struct VerticleClient {
    event_tx: mpsc::UnboundedSender<Event>,
    handle: Arc<Mutex<Option<JoinHandle<()>>>>,
}

type CmdResult<T> = StdResult<T, String>;

/// Futures driven by [`Verticle::run_loop`]: engine commands plus internal work.
type Pending = FuturesUnordered<Pin<Box<dyn Future<Output = ()>>>>;

/// Runs one engine command on the verticle and forwards its outcome to the
/// caller through `complete`, normalising every error to a message string.
fn respond<T: 'static>(
    pending: &mut Pending,
    complete: oneshot::Sender<CmdResult<T>>,
    command: impl Future<Output = Result<T>> + 'static,
) {
    pending.push(
        async move {
            let _ = complete.send(command.await.map_err(|error| error.to_string()));
        }
        .boxed_local(),
    );
}

pub(crate) enum Event {
    Start {
        complete: oneshot::Sender<CmdResult<()>>,
    },
    Stop {
        complete: oneshot::Sender<CmdResult<()>>,
    },
    GetConversation {
        id: Id,
        complete: oneshot::Sender<CmdResult<Option<Conversation>>>,
    },
    GetConversations {
        complete: oneshot::Sender<CmdResult<Vec<Conversation>>>,
    },
    RemoveConversation {
        id: Id,
        complete: oneshot::Sender<CmdResult<()>>,
    },
    RemoveConversations {
        ids: Vec<Id>,
        complete: oneshot::Sender<CmdResult<()>>,
    },
    GetMessages {
        conversation_id: Id,
        until: Option<i64>,
        limit: usize,
        offset: usize,
        complete: oneshot::Sender<CmdResult<Vec<Box<dyn Message>>>>,
    },
    GetMessagesInRange {
        conversation_id: Id,
        begin: i64,
        end: i64,
        complete: oneshot::Sender<CmdResult<Vec<Box<dyn Message>>>>,
    },
    RemoveMessage {
        message_id: i64,
        complete: oneshot::Sender<CmdResult<()>>,
    },
    RemoveMessagesByIds {
        message_ids: Vec<i64>,
        complete: oneshot::Sender<CmdResult<()>>,
    },
    RemoveMessagesInConversation {
        conversation_id: Id,
        complete: oneshot::Sender<CmdResult<()>>,
    },
    GetSessions {
        complete: oneshot::Sender<CmdResult<Vec<super::session_info::SessionInfo>>>,
    },
    RevokeSession {
        device_id: Id,
        complete: oneshot::Sender<CmdResult<()>>,
    },
    FriendRequest {
        user_id: Id,
        hello: Option<String>,
        complete: oneshot::Sender<CmdResult<()>>,
    },
    FriendAccept {
        user_id: Id,
        complete: oneshot::Sender<CmdResult<()>>,
    },
    GetFriendRequest {
        user_id: Id,
        complete: oneshot::Sender<CmdResult<Option<FriendRequest>>>,
    },
    GetFriendRequests {
        complete: oneshot::Sender<CmdResult<Vec<FriendRequest>>>,
    },
    RemoveFriendRequest {
        user_id: Id,
        complete: oneshot::Sender<CmdResult<()>>,
    },
    RemoveFriendRequests {
        user_ids: Vec<Id>,
        complete: oneshot::Sender<CmdResult<()>>,
    },
    ClearFriendRequests {
        complete: oneshot::Sender<CmdResult<()>>,
    },
    ContentMessage {
        recipient: Id,
        headers: std::collections::HashMap<String, serde_json::Value>,
        body: Vec<u8>,
        text: bool,
        complete: oneshot::Sender<CmdResult<Box<dyn Message>>>,
    },
    RegisterFriendSession {
        user_id: Id,
        session_key: Vec<u8>,
        remark: Option<String>,
        complete: oneshot::Sender<CmdResult<()>>,
    },

    BlockUser {
        user_id: Id,
        complete: oneshot::Sender<CmdResult<Contact>>,
    },

    CreateChannel {
        permission: Permission,
        name: String,
        notice: Option<String>,
        announcement: bool,
        complete: oneshot::Sender<CmdResult<Box<dyn Channel>>>,
    },

    RemoveChannel {
        channel_id: Id,
        complete: oneshot::Sender<CmdResult<()>>,
    },

    JoinChannel {
        ticket: InviteTicket,
        complete: oneshot::Sender<CmdResult<Box<dyn Channel>>>,
    },

    LeaveChannel {
        channel_id: Id,
        complete: oneshot::Sender<CmdResult<()>>,
    },

    CreateInviteTicket {
        channel_id: Id,
        invitee: Option<Id>,
        complete: oneshot::Sender<CmdResult<InviteTicket>>,
    },

    TransferChannelOwnership {
        channel_id: Id,
        new_owner: Id,
        complete: oneshot::Sender<CmdResult<()>>,
    },
    RotateChannelSessionKey {
        channel_id: Id,
        complete: oneshot::Sender<CmdResult<()>>,
    },

    UpdateChannelInfo {
        channel: Box<dyn Channel>,
        complete: oneshot::Sender<CmdResult<()>>,
    },

    SetChannelMemberRole {
        channel_id: Id,
        member_ids: Vec<Id>,
        role: Role,
        complete: oneshot::Sender<CmdResult<()>>,
    },

    BanChannelMembers {
        channel_id: Id,
        member_ids: Vec<Id>,
        complete: oneshot::Sender<CmdResult<()>>,
    },

    UnbanChannelMembers {
        channel_id: Id,
        member_ids: Vec<Id>,
        complete: oneshot::Sender<CmdResult<()>>,
    },

    RemoveChannelMembers {
        channel_id: Id,
        members: Vec<Id>,
        complete: oneshot::Sender<CmdResult<()>>,
    },

    GetContact {
        id: Id,
        complete: oneshot::Sender<CmdResult<Option<Contact>>>,
    },
    GetContacts {
        complete: oneshot::Sender<CmdResult<Vec<Contact>>>,
    },
    UpdateContact {
        contact: Contact,
        complete: oneshot::Sender<CmdResult<()>>,
    },
    RemoveContacts {
        ids: Vec<Id>,
        complete: oneshot::Sender<CmdResult<()>>,
    },
    ClearContacts {
        complete: oneshot::Sender<CmdResult<()>>,
    }
}

impl VerticleClient {
    fn new(
        event_tx: mpsc::UnboundedSender<Event>,
        handle: JoinHandle<()>
    ) -> Self {
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
        make_event: impl FnOnce(oneshot::Sender<CmdResult<T>>) -> Event,
    ) -> Result<T> {
        let (tx, rx) = oneshot::channel();
        self.event_tx
            .send(make_event(tx))
            .map_err(|_| StateError::new(CHANNEL_REQ_CLOSED))?;

        match rx.await {
            Ok(Ok(v)) => Ok(v),
            Ok(Err(msg)) => Err(StateError::new(msg)),
            Err(_) => Err(StateError::new(CHANNEL_RSP_CLOSED)),
        }
    }

    pub(crate) async fn start(&self) -> Result<()> {
        self.call(|complete| Event::Start { complete }).await
    }

    pub(crate) async fn stop(&self) -> Result<()> {
        let result = self.call(|complete| Event::Stop { complete }).await;

        // A poisoned lock still holds a usable handle; never panic on shutdown.
        let handle = self
            .handle
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .take();
        if let Some(handle) = handle {
            // `join` parks the calling thread, so keep it off the async worker.
            let joined = task::spawn_blocking(move || handle.join())
                .await
                .map_err(|error| {
                    StateError::new(format!("Joining messaging verticle task failed: {error}"))
                })?;
            joined.map_err(|_| StateError::new("Messaging verticle thread panicked"))?;
        }
        result
    }

    pub(crate) async fn get_conversation(
        &self,
        id: Id,
    ) -> Result<Option<Conversation>> {
        self.call(|complete| Event::GetConversation {
            id,
            complete
        }).await
    }

    pub(crate) async fn get_conversations(
        &self,
    ) -> Result<Vec<Conversation>> {
        self.call(|complete| Event::GetConversations {
            complete
        }).await
    }

    pub(crate) async fn remove_conversation(&self, id: Id) -> Result<()> {
        self.call(|complete| Event::RemoveConversation {
            id,
            complete
        }).await
    }

    pub(crate) async fn remove_conversations(&self, ids: Vec<Id>) -> Result<()> {
        self.call(|complete| Event::RemoveConversations {
            ids,
            complete
        }).await
    }

    pub(crate) async fn get_messages(
        &self,
        conversation_id: Id,
        until: Option<i64>,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<Box<dyn Message>>> {
        self.call(|complete| Event::GetMessages {
            conversation_id,
            until,
            limit,
            offset,
            complete,
        }).await
    }

    pub(crate) async fn get_messages_in_range(
        &self,
        conversation_id: Id,
        begin: i64,
        end: i64,
    ) -> Result<Vec<Box<dyn Message>>> {
        self.call(|complete| Event::GetMessagesInRange {
            conversation_id,
            begin,
            end,
            complete,
        }).await
    }

    pub(crate) async fn remove_message(
        &self,
        message_id: i64
    ) -> Result<()> {
        self.call(|complete| Event::RemoveMessage {
            message_id, complete }).await
    }

    pub(crate) async fn remove_messages_by_ids(
        &self,
        message_ids: Vec<i64>
    ) -> Result<()> {
        self.call(|complete| Event::RemoveMessagesByIds {
            message_ids,
            complete
        }).await
    }

    pub(crate) async fn remove_messages_in_conversation(
        &self,
        conversation_id: Id,
    ) -> Result<()> {
        self.call(|complete| Event::RemoveMessagesInConversation {
            conversation_id,
            complete,
        }).await
    }

    pub(crate) async fn get_sessions(
        &self
    ) -> Result<Vec<super::session_info::SessionInfo>> {
        self.call(|complete| Event::GetSessions {
            complete
        }).await
    }

    pub(crate) async fn revoke_session(
        &self,
        device_id: Id
    ) -> Result<()> {
        self.call(|complete| Event::RevokeSession {
            device_id,
            complete
        }).await
    }

    pub(crate) async fn friend_request(
        &self,
        user_id: Id,
        hello: Option<String>
    ) -> Result<()> {
        self.call(|complete| Event::FriendRequest {
            user_id,
            hello,
            complete,
        }).await
    }

    pub(crate) async fn friend_accept(
        &self,
        user_id: Id
    ) -> Result<()> {
        self.call(|complete| Event::FriendAccept {
            user_id,
            complete
        }).await
    }

    pub(crate) async fn get_friend_request(
        &self,
        user_id: Id
    ) -> Result<Option<FriendRequest>> {
        self.call(|complete| Event::GetFriendRequest {
            user_id,
            complete,
        }).await
    }

    pub(crate) async fn get_friend_requests(
        &self
    ) -> Result<Vec<FriendRequest>> {
        self.call(|complete| Event::GetFriendRequests {
            complete,
        }).await
    }

    pub(crate) async fn remove_friend_request(
        &self,
        user_id: Id
    ) -> Result<()> {
        self.call(|complete| Event::RemoveFriendRequest {
            user_id,
            complete,
        }).await
    }

    pub(crate) async fn remove_friend_requests(
        &self,
        user_ids: Vec<Id>
    ) -> Result<()> {
        self.call(|complete| Event::RemoveFriendRequests {
            user_ids,
            complete,
        }).await
    }

    pub(crate) async fn clear_friend_requests(&self) -> Result<()> {
        self.call(|complete| Event::ClearFriendRequests {
            complete,
        }).await
    }

    pub(crate) async fn add_friend(
        &self,
        user_id: Id,
        session_key: Vec<u8>,
        remark: Option<String>,
    ) -> Result<()> {
        self.call(|complete| Event::RegisterFriendSession {
            user_id,
            session_key,
            remark,
            complete,
        }).await
    }

    pub(crate) async fn block_user(
        &self,
        user_id: Id
    ) -> Result<Contact> {
        self.call(|complete| Event::BlockUser {
            user_id,
            complete
        }).await
    }

    pub(crate) async fn create_channel(
        &self,
        permission: Permission,
        name: String,
        notice: Option<String>,
        announcement: bool,
    ) -> Result<Box<dyn Channel>> {
        self.call(|complete| Event::CreateChannel {
            permission,
            name,
            notice,
            announcement,
            complete,
        }).await
    }

    pub(crate) async fn remove_channel(
        &self,
        channel_id: Id
    ) -> Result<()> {
        self.call(|complete| Event::RemoveChannel {
            channel_id,
            complete,
        }).await
    }

    pub(crate) async fn join_channel(
        &self,
        ticket: InviteTicket,
    ) -> Result<Box<dyn Channel>> {
        self.call(|complete| Event::JoinChannel {
            ticket, complete
        }).await
    }

    pub(crate) async fn leave_channel(
        &self,
        channel_id: Id
    ) -> Result<()> {
        self.call(|complete| Event::LeaveChannel {
            channel_id,
            complete
        }).await
    }

    pub(crate) async fn create_invite_ticket(
        &self,
        channel_id: Id,
        invitee: Option<Id>,
    ) -> Result<InviteTicket> {
        self.call(|complete| Event::CreateInviteTicket {
            channel_id,
            invitee,
            complete,
        }).await
    }

    pub(crate) async fn transfer_channel_ownership(
        &self,
        channel_id: Id,
        new_owner: Id
    ) -> Result<()> {
        self.call(|complete| Event::TransferChannelOwnership {
            channel_id,
            new_owner,
            complete,
        }).await
    }

    pub(crate) async fn rotate_channel_session_key(
        &self,
        channel_id: Id
    ) -> Result<()> {
        self.call(|complete| Event::RotateChannelSessionKey {
            channel_id,
            complete
        }).await
    }

    pub(crate) async fn update_channel_info(
        &self,
        channel: Box<dyn Channel>
    ) -> Result<()> {
        self.call(|complete| Event::UpdateChannelInfo {
            channel,
            complete
        }).await
    }

    pub(crate) async fn set_channel_members_role(
        &self,
        channel_id: Id,
        member_ids: Vec<Id>,
        role: Role
    ) -> Result<()> {
        self.call(|complete| Event::SetChannelMemberRole {
            channel_id,
            member_ids,
            role,
            complete,
        }).await
    }

    pub(crate) async fn ban_channel_members(
        &self,
        channel_id: Id,
        member_ids: Vec<Id>
    ) -> Result<()> {
        self.call(|complete| Event::BanChannelMembers {
            channel_id,
            member_ids,
            complete,
        }).await
    }

    pub(crate) async fn unban_channel_members(
        &self,
        channel_id: Id,
        member_ids: Vec<Id>
    ) -> Result<()> {
        self.call(|complete| Event::UnbanChannelMembers {
            channel_id,
            member_ids,
            complete,
        }).await
    }

    pub(crate) async fn remove_channel_members(
        &self,
        channel_id: Id,
        members: Vec<Id>
    ) -> Result<()> {
        self.call(|complete| Event::RemoveChannelMembers {
            channel_id,
            members,
            complete,
        }).await
    }

    pub(crate) async fn get_contact(&self, id: Id) -> Result<Option<Contact>> {
        self.call(|complete| Event::GetContact {
            id,
            complete
        }).await
    }

    pub(crate) async fn get_contacts(&self) -> Result<Vec<Contact>> {
        self.call(|complete| Event::GetContacts {
            complete
        }).await
    }

    pub(crate) async fn update_contact(&self, contact: Contact) -> Result<()> {
        self.call(|complete| Event::UpdateContact {
            contact,
            complete,
        }).await
    }

    pub(crate) async fn remove_contacts(&self, ids: Vec<Id>) -> Result<()> {
        self.call(|complete| Event::RemoveContacts {
            ids,
            complete,
        }).await
    }

    pub(crate) async fn clear_contacts(&self) -> Result<()> {
        self.call(|complete| Event::ClearContacts {
            complete,
        }).await
    }
}

pub(crate) struct VerticleOptions {
    pub(crate) options: Arc<Options>,

    pub(crate) connection_listener: Arc<dyn ConnectionListener>,
    pub(crate) message_listener: Arc<dyn MessageListener>,
    pub(crate) channel_listener: Arc<dyn ChannelListener>,
    pub(crate) contact_listener: Arc<dyn ContactListener>,
    pub(crate) session_listener: Arc<dyn SessionListener>,
    pub(crate) friend_request_listener: Arc<dyn FriendRequestListener>,
}

pub(crate) struct Verticle {
    rpc_server:  Rc<RpcServer>,
    event_rx: mpsc::UnboundedReceiver<Event>,
    quit: bool,
}

impl Verticle {
    fn new(
        options: VerticleOptions,
        event_rx: mpsc::UnboundedReceiver<Event>,
    ) -> Result<Self> {
        let rpc_server = RpcServer::new(options)?;
        Ok(Self {
            rpc_server,
            event_rx,
            quit: false,
        })
    }

    fn handle_events(&mut self, event: Event, pending: &mut Pending) {
        let rpc = self.rpc_server.clone();
        match event {
            Event::Start { complete } => {
                respond(pending, complete, async move { rpc.start().await });
            }
            Event::Stop { complete } => {
                self.quit = true;
                respond(pending, complete, async move { rpc.stop().await });
            }
            Event::GetConversation { id, complete } => {
                respond(pending, complete, async move { rpc.get_conversation(&id).await });
            }
            Event::GetConversations { complete } => {
                respond(pending, complete, async move { rpc.get_conversations().await });
            }
            Event::RemoveConversation { id, complete } => {
                respond(pending, complete, async move { rpc.remove_conversation(&id).await });
            }
            Event::RemoveConversations { ids, complete } => {
                respond(pending, complete, async move { rpc.remove_conversations(&ids).await });
            }
            Event::GetMessages {
                conversation_id,
                until,
                limit,
                offset,
                complete,
            } => {
                respond(pending, complete, async move {
                    rpc.get_messages(&conversation_id, until, limit, offset).await
                });
            }
            Event::GetMessagesInRange {
                conversation_id,
                begin,
                end,
                complete,
            } => {
                respond(pending, complete, async move {
                    rpc.get_messages_in_range(&conversation_id, begin, end).await
                });
            }
            Event::RemoveMessage { message_id, complete } => {
                respond(pending, complete, async move { rpc.remove_message(message_id).await });
            }
            Event::RemoveMessagesByIds { message_ids, complete } => {
                respond(pending, complete, async move {
                    rpc.remove_messages_by_ids(&message_ids).await
                });
            }
            Event::RemoveMessagesInConversation { conversation_id, complete } => {
                respond(pending, complete, async move {
                    rpc.remove_messages_in_conversation(&conversation_id).await
                });
            }
            Event::GetSessions { complete } => {
                respond(pending, complete, async move { rpc.get_sessions().await });
            }
            Event::RevokeSession { device_id, complete } => {
                respond(pending, complete, async move { rpc.revoke_session(&device_id).await });
            }
            Event::FriendRequest {
                user_id,
                hello,
                complete,
            } => {
                respond(pending, complete, async move {
                    rpc.friend_request(&user_id, hello).await
                });
            }
            Event::FriendAccept { user_id, complete } => {
                respond(pending, complete, async move {
                    rpc.accept_friend_request(&user_id).await
                });
            }
            Event::GetFriendRequest { user_id, complete } => {
                respond(pending, complete, async move {
                    rpc.get_friend_request(&user_id).await
                });
            }
            Event::GetFriendRequests { complete } => {
                respond(pending, complete, async move { rpc.get_friend_requests().await });
            }
            Event::RemoveFriendRequest { user_id, complete } => {
                respond(pending, complete, async move {
                    rpc.remove_friend_request(&user_id).await
                });
            }
            Event::RemoveFriendRequests { user_ids, complete } => {
                respond(pending, complete, async move {
                    rpc.remove_friend_requests(&user_ids).await
                });
            }
            Event::ClearFriendRequests { complete } => {
                respond(pending, complete, async move { rpc.clear_friend_requests().await });
            }
            Event::ContentMessage {
                recipient,
                headers,
                body,
                text,
                complete,
            } => {
                respond(pending, complete, async move {
                    send_content_message(&rpc, recipient, headers, body, text).await
                });
            }
            Event::RegisterFriendSession {
                user_id,
                session_key,
                remark,
                complete,
            } => {
                respond(pending, complete, async move {
                    rpc.add_friend(&user_id, session_key, remark).await
                });
            }
            Event::BlockUser { user_id, complete } => {
                respond(pending, complete, async move { rpc.block_user(&user_id).await });
            }

            Event::CreateChannel {
                permission,
                name,
                notice,
                announcement,
                complete
            } => {
                respond(pending, complete, async move {
                    rpc.create_channel(permission, name, notice, announcement).await
                });
            }
            Event::RemoveChannel { channel_id, complete } => {
                respond(pending, complete, async move { rpc.remove_channel(&channel_id).await });
            }

            Event::JoinChannel { ticket, complete } => {
                respond(pending, complete, async move { rpc.join_channel(ticket).await });
            }

            Event::LeaveChannel { channel_id, complete } => {
                respond(pending, complete, async move { rpc.leave_channel(&channel_id).await });
            }

            Event::CreateInviteTicket { channel_id, invitee, complete } => {
                respond(pending, complete, async move {
                    rpc.create_invite_ticket(&channel_id, invitee).await
                });
            }

            Event::TransferChannelOwnership { channel_id, new_owner, complete } => {
                respond(pending, complete, async move {
                    rpc.transfer_channel_ownership(&channel_id, new_owner).await
                });
            }

            Event::RotateChannelSessionKey { channel_id, complete } => {
                respond(pending, complete, async move {
                    rpc.rotate_channel_session_key(&channel_id).await
                });
            }

            Event::UpdateChannelInfo { channel, complete } => {
                respond(pending, complete, async move {
                    rpc.update_channel_info(channel.as_ref()).await
                });
            }

            Event::SetChannelMemberRole { channel_id, member_ids, role, complete } => {
                respond(pending, complete, async move {
                    rpc.set_channel_members_role(&channel_id, &member_ids, role).await
                });
            }

            Event::BanChannelMembers { channel_id, member_ids, complete } => {
                respond(pending, complete, async move {
                    rpc.ban_channel_members(&channel_id, &member_ids).await
                });
            }
            Event::UnbanChannelMembers { channel_id, member_ids, complete } => {
                respond(pending, complete, async move {
                    rpc.unban_channel_members(&channel_id, &member_ids).await
                });
            }
            Event::RemoveChannelMembers { channel_id, members, complete } => {
                respond(pending, complete, async move {
                    rpc.remove_channel_members(&channel_id, &members).await
                });
            }

            Event::GetContact { id, complete } => {
                respond(pending, complete, async move { rpc.get_contact(&id).await });
            }
            Event::GetContacts { complete } => {
                respond(pending, complete, async move { rpc.get_contacts().await });
            }
            Event::UpdateContact { contact, complete } => {
                respond(pending, complete, async move { rpc.update_contact(&contact).await });
            }
            Event::RemoveContacts { ids, complete } => {
                respond(pending, complete, async move { rpc.remove_contacts(&ids).await });
            }
            Event::ClearContacts { complete } => {
                respond(pending, complete, async move { rpc.clear_contacts().await });
            }
        }
    }

    async fn run_loop(&mut self) {
        let mut pendings = Pending::new();
        loop {
            // Nothing can make progress once the command channel is closed and
            // every command already in flight has settled.
            if self.quit && pendings.is_empty() {
                break;
            }
            if !self.quit {
                self.rpc_server.schedule_contact_sync(&mut pendings);
            }
            let rpc = self.rpc_server.clone();
            tokio::select! {
                event = self.event_rx.recv(), if !self.quit => {
                    match event {
                        Some(event) => self.handle_events(event, &mut pendings),
                        // Every sender is gone: drain in-flight commands, then
                        // let the shutdown below stop the session exactly once.
                        None => self.quit = true,
                    }
                }
                Some(()) = pendings.next(), if !pendings.is_empty() => {},
                event = rpc.receive_event(), if rpc.is_running() => {
                    rpc.handle_mqtt_event(event, &mut pendings);
                }
                else => break,
            }
        }

        if let Err(error) = self.rpc_server.stop().await {
            error!("Stopping messaging session failed: {error}");
        }
    }
}

async fn send_content_message(
    engine: &Rc<RpcServer>,
    recipient: Id,
    headers: HashMap<String, serde_json::Value>,
    body: Vec<u8>,
    text: bool,
) -> Result<Box<dyn Message>> {
    let mut builder = engine.message(Some(recipient));
    for (key, value) in headers {
        let value = value
            .as_str()
            .ok_or_else(|| StateError::new(format!("Message header '{key}' is not text")))?;
        builder = builder.header(&key, value);
    }
    builder = if text {
        let content = String::from_utf8(body)
            .map_err(|error| StateError::new(error.to_string()))?;
        builder.text_body(&content)
    } else {
        builder.binary_body(body)
    };
    builder.send().await
}

pub(crate) fn deploy(options: VerticleOptions) -> Result<VerticleClient> {
    let (event_tx, event_rx) = mpsc::unbounded_channel::<Event>();
    let (startup_tx, startup_rx) = std_mpsc::sync_channel::<StdResult<(), String>>(1);

    let handle = std::thread::Builder::new()
        .name("messaging-verticle".into())
        .spawn(move || {
            let rt = match runtime::Builder::new_current_thread()
                .enable_time()
                .enable_io()
                .build()
            {
                Ok(rt) => rt,
                Err(error) => {
                    let _ = startup_tx.send(Err(format!(
                        "Building messaging verticle runtime failed: {error}"
                    )));
                    return;
                }
            };

            let local = task::LocalSet::new();
            rt.block_on(local.run_until(async move {
                let mut verticle = match Verticle::new(options, event_rx) {
                    Ok(verticle) => verticle,
                    Err(error) => {
                        let _ = startup_tx.send(Err(error.to_string()));
                        return;
                    }
                };
                let _ = startup_tx.send(Ok(()));
                verticle.run_loop().await;
            }));
        })
        .map_err(|error| {
            error!("Spawning messaging verticle thread failed: {error}");
            StateError::new(format!("Spawning messaging verticle thread failed: {error}"))
        })?;

    match startup_rx.recv() {
        Ok(Ok(())) => {
            debug!("Messaging verticle deployed successfully");
            Ok(VerticleClient::new(event_tx, handle))
        }
        Ok(Err(msg)) => {
            error!("Messaging verticle failed to deploy: {msg}");
            let _ = handle.join();
            Err(StateError::new(msg))
        }
        Err(_) => {
            error!("Messaging verticle startup channel closed unexpectedly");
            let _ = handle.join();
            Err(StateError::new(
                "Messaging verticle startup channel closed",
            ))
        }
    }
}
