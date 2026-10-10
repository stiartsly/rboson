use log::{info, debug, error};
use core::{f32::consts::E, ops::Not};
use std::{
    rc::Rc,
    thread::JoinHandle,
    future::Future,
    pin::Pin,
    result::Result as StdResult,
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc as std_mpsc, Arc, Mutex,
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

use crate::Id;
use crate::errors::{Result, StateError};
use crate::messaging::{
    options::Options,
    contact::Contact,
    message::Message,
    friend_request_listener::FriendRequestListener,
    connection_listener::ConnectionListener,
    channel_listener::ChannelListener,
    contact_listener::ContactListener,
    message_listener::MessageListener,
    session_listener::SessionListener,
    mqtt::{Session, SessionAgent},
    FriendRequest,
    InviteTicket,
    Channel,
    Permission,
    Role,
};
use super::internal::{
    PhotonContact,
};

const CHANNEL_REQ_CLOSED: &str = "verticle request channel closed";
const CHANNEL_RSP_CLOSED: &str = "verticle response channel closed";

fn contact_snapshot(contact: &dyn Contact) -> PhotonContact {
    PhotonContact {
        id: *contact.id(),
        contact_type: contact.contact_type(),
        name: contact.name().map(ToString::to_string),
        remark: contact.remark().map(ToString::to_string),
        tags: contact.tags().map(ToString::to_string),
        muted: contact.is_muted(),
        blocked: contact.is_blocked(),
        created_at: contact.created_at(),
        updated_at: contact.updated_at(),
        revision: contact.revision(),
    }
}

#[derive(Clone)]
pub(crate) struct VerticleClient {
    event_tx: mpsc::UnboundedSender<Event>,
    handle: Arc<Mutex<Option<JoinHandle<()>>>>,
}

type CmdResult<T> = StdResult<T, String>;
pub(crate) enum Event {
    Start {
        complete: oneshot::Sender<CmdResult<()>>,
    },
    Stop {
        complete: oneshot::Sender<CmdResult<()>>,
    },
    GetConversation {
        id: Id,
        complete: oneshot::Sender<CmdResult<Option<Box<dyn super::conversation::Conversation>>>>,
    },
    GetConversations {
        complete: oneshot::Sender<CmdResult<Vec<Box<dyn super::conversation::Conversation>>>>,
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
        complete: oneshot::Sender<CmdResult<Box<dyn Contact>>>,
    },

    CreateChannel {
        permission: Permission,
        name: String,
        notice: Option<String>,
        announcement: Option<String>,
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
        complete: oneshot::Sender<CmdResult<Option<PhotonContact>>>,
    },
    GetContacts {
        complete: oneshot::Sender<CmdResult<Vec<PhotonContact>>>,
    },
    UpdateContact {
        contact: PhotonContact,
        complete: oneshot::Sender<CmdResult<()>>,
    },
    RemoveContact {
        id: Id,
        complete: oneshot::Sender<CmdResult<()>>,
    },
    RemoveContacts {
        ids: Vec<Id>,
        complete: oneshot::Sender<CmdResult<()>>,
    },
    ClearContacts {
        complete: oneshot::Sender<CmdResult<()>>,
    },
    FriendReject {
        user_id: Id,
        complete: oneshot::Sender<CmdResult<()>>,
    },
    FriendRemove {
        user_id: Id,
        complete: oneshot::Sender<CmdResult<()>>,
    },
    FriendInfo {
        user_id: Id,
        complete: oneshot::Sender<CmdResult<()>>,
    },
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

    pub(crate) async fn start(&self) -> Result<()> {
        let (tx, rx) = oneshot::channel();
        self.event_tx
            .send(Event::Start { complete: tx })
            .map_err(|_| StateError::new(CHANNEL_REQ_CLOSED))?;

        rx.await
            .map_err(|_| StateError::new(CHANNEL_RSP_CLOSED))?
            .map_err(StateError::new)?;
        Ok(())
    }

    pub(crate) async fn stop(&self) -> Result<()> {
        let (tx, rx) = oneshot::channel();
        self.event_tx
            .send(Event::Stop { complete: tx })
            .map_err(|_| StateError::new(CHANNEL_REQ_CLOSED))?;

        rx.await
            .map_err(|_| StateError::new(CHANNEL_RSP_CLOSED))?
            .map_err(StateError::new)?;

        let handle = self.handle.lock().unwrap().take();
        if let Some(handle) = handle {
            let _ = handle.join();
        }
        Ok(())
    }

    #[inline]
    async fn result<T>(
        &self,
        rx: oneshot::Receiver<CmdResult<T>>
    ) -> Result<T> {
        match rx.await {
            Ok(Ok(v)) => Ok(v),
            Ok(Err(msg)) => Err(StateError::new(msg)),
            Err(_) => Err(StateError::new(CHANNEL_RSP_CLOSED)),
        }
    }

    async fn request<T>(
        &self,
        make_event: impl FnOnce(oneshot::Sender<CmdResult<T>>) -> Event,
    ) -> Result<T> {
        let (tx, rx) = oneshot::channel();
        self.event_tx
            .send(make_event(tx))
            .map_err(|_| StateError::new(CHANNEL_REQ_CLOSED))?;
        self.result(rx).await
    }

    pub(crate) fn sender(&self) -> mpsc::UnboundedSender<Event> {
        self.event_tx.clone()
    }

    pub(crate) async fn get_conversation(
        &self,
        id: Id,
    ) -> Result<Option<Box<dyn super::conversation::Conversation>>> {
        self.request(|complete| Event::GetConversation { id, complete }).await
    }

    pub(crate) async fn get_conversations(
        &self,
    ) -> Result<Vec<Box<dyn super::conversation::Conversation>>> {
        self.request(|complete| Event::GetConversations { complete }).await
    }

    pub(crate) async fn remove_conversation(&self, id: Id) -> Result<()> {
        self.request(|complete| Event::RemoveConversation { id, complete }).await
    }

    pub(crate) async fn remove_conversations(&self, ids: Vec<Id>) -> Result<()> {
        self.request(|complete| Event::RemoveConversations { ids, complete }).await
    }

    pub(crate) async fn get_messages(
        &self,
        conversation_id: Id,
        until: Option<i64>,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<Box<dyn Message>>> {
        self.request(|complete| Event::GetMessages {
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
        self.request(|complete| Event::GetMessagesInRange {
            conversation_id,
            begin,
            end,
            complete,
        }).await
    }

    pub(crate) async fn remove_message(&self, message_id: i64) -> Result<()> {
        self.request(|complete| Event::RemoveMessage { message_id, complete }).await
    }

    pub(crate) async fn remove_messages_by_ids(&self, message_ids: Vec<i64>) -> Result<()> {
        self.request(|complete| Event::RemoveMessagesByIds { message_ids, complete }).await
    }

    pub(crate) async fn remove_messages_in_conversation(
        &self,
        conversation_id: Id,
    ) -> Result<()> {
        self.request(|complete| Event::RemoveMessagesInConversation {
            conversation_id,
            complete,
        }).await
    }

    pub(crate) async fn get_sessions(&self) -> Result<Vec<super::session_info::SessionInfo>> {
        self.request(|complete| Event::GetSessions { complete }).await
    }

    pub(crate) async fn revoke_session(&self, device_id: Id) -> Result<()> {
        self.request(|complete| Event::RevokeSession { device_id, complete }).await
    }

    pub(crate) async fn friend_request(
        &self,
        user_id: Id,
        hello: Option<String>
    ) -> Result<()> {
        let (tx, rx) = oneshot::channel();
        self.event_tx.send(Event::FriendRequest {
            user_id,
            hello,
            complete: tx,
        })
        .map_err(|_|
            StateError::new(CHANNEL_REQ_CLOSED)
        )?;
        self.result(rx).await
    }

    pub(crate) async fn friend_accept(
        &self,
        user_id: Id
    ) -> Result<()> {
        let (tx, rx) = oneshot::channel();
        self.event_tx.send(Event::FriendAccept {
            user_id,
            complete: tx,
        })
        .map_err(|_|
            StateError::new(CHANNEL_REQ_CLOSED)
        )?;
        self.result(rx).await
    }

    pub(crate) async fn get_friend_request(
        &self,
        user_id: Id
    ) -> Result<Option<FriendRequest>> {
        let (tx, rx) = oneshot::channel();
        self.event_tx.send(Event::GetFriendRequest {
            user_id,
            complete: tx,
        })
        .map_err(|_|
            StateError::new(CHANNEL_REQ_CLOSED)
        )?;
        self.result(rx).await
    }

    pub(crate) async fn get_friend_requests(&self) -> Result<Vec<FriendRequest>> {
        let (tx, rx) = oneshot::channel();
        self.event_tx.send(Event::GetFriendRequests {
            complete: tx
        })
        .map_err(|_|
            StateError::new(CHANNEL_REQ_CLOSED)
        )?;
        self.result(rx).await
    }

    pub(crate) async fn remove_friend_request(
        &self,
        user_id: Id
    ) -> Result<()> {
        let (tx, rx) = oneshot::channel();
        self.event_tx.send(Event::RemoveFriendRequest {
            user_id,
            complete: tx,
        })
        .map_err(|_|
            StateError::new(CHANNEL_REQ_CLOSED)
        )?;
        self.result(rx).await
    }

    pub(crate) async fn remove_friend_requests(
        &self,
        user_ids: Vec<Id>
    ) -> Result<()> {
        let (tx, rx) = oneshot::channel();
        self.event_tx.send(Event::RemoveFriendRequests {
            user_ids,
            complete: tx,
        })
        .map_err(|_|
            StateError::new(CHANNEL_REQ_CLOSED)
        )?;
        self.result(rx).await
    }

    pub(crate) async fn clear_friend_requests(&self) -> Result<()> {
        let (tx, rx) = oneshot::channel();
        self.event_tx.send(Event::ClearFriendRequests {
            complete: tx
        })
        .map_err(|_|
            StateError::new(CHANNEL_REQ_CLOSED)
        )?;
        self.result(rx).await
    }

    pub(crate) async fn add_friend(
        &self,
        user_id: Id,
        session_key: Vec<u8>,
        remark: Option<String>,
    ) -> Result<()> {
        let (tx, rx) = oneshot::channel();
        self.event_tx.send(Event::RegisterFriendSession {
            user_id,
            session_key,
            remark,
            complete: tx,
        })
        .map_err(|_|
            StateError::new(CHANNEL_REQ_CLOSED)
        )?;
        self.result(rx).await
    }

    pub(crate) async fn block_user(&self, user_id: Id) -> Result<Box<dyn Contact>> {
        self.request(|complete| Event::BlockUser { user_id, complete }).await
    }

    pub(crate) async fn create_channel(
        &self,
        permission: Permission,
        name: String,
        notice: Option<String>,
        announcement: Option<String>,
    ) -> Result<Box<dyn Channel>> {
        self.request(|complete| Event::CreateChannel {
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
        let (tx, rx) = oneshot::channel();
        self.event_tx.send(Event::RemoveChannel {
            channel_id,
            complete: tx,
        })
        .map_err(|_|
            StateError::new(CHANNEL_REQ_CLOSED)
        )?;
        self.result(rx).await
    }

    pub(crate) async fn join_channel(
        &self,
        ticket: InviteTicket,
    ) -> Result<Box<dyn Channel>> {
        self.request(|complete| Event::JoinChannel { ticket, complete }).await
    }

    pub(crate) async fn leave_channel(
        &self,
        channel_id: Id
    ) -> Result<()> {
        self.request(|complete| Event::LeaveChannel { channel_id, complete }).await
    }

    pub(crate) async fn create_invite_ticket(
        &self,
        channel_id: Id,
        invitee: Option<Id>,
    ) -> Result<InviteTicket> {
        self.request(|complete| Event::CreateInviteTicket {
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
        self.request(|complete| Event::TransferChannelOwnership {
            channel_id,
            new_owner,
            complete,
        }).await
    }

    pub(crate) async fn rotate_channel_session_key(
        &self,
        channel_id: Id
    ) -> Result<()> {
        self.request(|complete| Event::RotateChannelSessionKey { channel_id, complete }).await
    }

    pub(crate) async fn update_channel_info(
        &self,
        channel: Box<dyn Channel>
    ) -> Result<()> {
        self.request(|complete| Event::UpdateChannelInfo { channel, complete }).await
    }

    pub(crate) async fn set_channel_members_role(
        &self,
        channel_id: Id,
        member_ids: Vec<Id>,
        role: Role
    ) -> Result<()> {
        self.request(|complete| Event::SetChannelMemberRole {
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
        self.request(|complete| Event::BanChannelMembers {
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
        self.request(|complete| Event::UnbanChannelMembers {
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
        let (tx, rx) = oneshot::channel();
        self.event_tx.send(Event::RemoveChannelMembers {
            channel_id,
            members,
            complete: tx,
        })
        .map_err(|_|
            StateError::new(CHANNEL_REQ_CLOSED)
        )?;
        self.result(rx).await
    }

    pub(crate) async fn get_contact(&self, id: Id) -> Result<Option<PhotonContact>> {
        let (tx, rx) = oneshot::channel();
        self.event_tx.send(Event::GetContact {
            id,
            complete: tx
        })
        .map_err(|_|
            StateError::new(CHANNEL_REQ_CLOSED)
        )?;
        self.result(rx).await
    }

    pub(crate) async fn get_contacts(&self) -> Result<Vec<PhotonContact>> {
        let (tx, rx) = oneshot::channel();
        self.event_tx.send(Event::GetContacts {
            complete: tx
        })
        .map_err(|_|
            StateError::new(CHANNEL_REQ_CLOSED)
        )?;
        self.result(rx).await
    }

    pub(crate) async fn update_contact(&self, contact: PhotonContact) -> Result<()> {
        let (tx, rx) = oneshot::channel();
        self.event_tx.send(Event::UpdateContact {
            contact,
            complete: tx,
        })
        .map_err(|_|
            StateError::new(CHANNEL_REQ_CLOSED)
        )?;
        self.result(rx).await
    }

    pub(crate) async fn remove_contact(&self, id: Id) -> Result<()> {
        let (tx, rx) = oneshot::channel();
        self.event_tx.send(Event::RemoveContact {
            id, complete: tx
        })
        .map_err(|_|
            StateError::new(CHANNEL_REQ_CLOSED)
        )?;
        self.result(rx).await
    }

    pub(crate) async fn remove_contacts(&self, ids: Vec<Id>) -> Result<()> {
        let (tx, rx) = oneshot::channel();
        self.event_tx.send(Event::RemoveContacts {
            ids, complete: tx
        })
        .map_err(|_|
            StateError::new(CHANNEL_REQ_CLOSED)
        )?;
        self.result(rx).await
    }

    pub(crate) async fn clear_contacts(&self) -> Result<()> {
        let (tx, rx) = oneshot::channel();
        self.event_tx.send(Event::ClearContacts {
            complete: tx
        })
        .map_err(|_|
            StateError::new(CHANNEL_REQ_CLOSED)
        )?;
        self.result(rx).await
    }

    #[allow(dead_code)]
    pub(crate) async fn friend_reject(&self, user_id: Id) -> Result<()> {
        let (tx, rx) = oneshot::channel();
        self.event_tx.send(Event::FriendReject {
            user_id,
            complete: tx,
        })
        .map_err(|_|
            StateError::new(CHANNEL_REQ_CLOSED)
        )?;
        self.result(rx).await
    }

    #[allow(dead_code)]
    pub(crate) async fn friend_remove(&self, user_id: Id) -> Result<()> {
        let (tx, rx) = oneshot::channel();
        self.event_tx.send(Event::FriendRemove {
            user_id,
            complete: tx,
        })
        .map_err(|_|
            StateError::new(CHANNEL_REQ_CLOSED)
        )?;
        self.result(rx).await
    }

    #[allow(dead_code)]
    pub(crate) async fn friend_info(&self, user_id: Id) -> Result<()> {
        let (tx, rx) = oneshot::channel();
        self.event_tx.send(Event::FriendInfo {
            user_id,
            complete: tx,
        })
        .map_err(|_|
            StateError::new(CHANNEL_REQ_CLOSED)
        )?;
        self.result(rx).await
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
    engine:  Rc<SessionAgent>,
    event_rx: mpsc::UnboundedReceiver<Event>,
    quit: bool,
}

impl Verticle {
    fn new(
        options: VerticleOptions,
        event_rx: mpsc::UnboundedReceiver<Event>,
    ) -> Result<Self> {
        let engine = Session::new(options)?;
        Ok(Self {
            engine,
            event_rx,
            quit: false,
        })
    }

    fn handle_events(
        &mut self,
        event: Event,
        pending: &mut FuturesUnordered<Pin<Box<dyn Future<Output = ()>>>>,
    ) {
        let engine = self.engine.clone();
        match event {
            Event::Start { complete } => {
                pending.push(
                    async move {
                        let rc = engine.start().await;
                        let _ = complete.send(rc.map_err(|e|e.to_string()));
                    }
                    .boxed_local(),
                );
            }
            Event::Stop { complete } => {
                self.quit = true;
                pending.push(
                    async move {
                        let rc = engine.stop().await;
                        let _ = complete.send(rc.map_err(|e|e.to_string()));
                    }
                    .boxed_local(),
                );
            }
            Event::GetConversation { id, complete } => {
                pending.push(async move {
                    let rc = engine.get_conversation(&id).await;
                    let _ = complete.send(rc.map_err(|error| error.to_string()));
                }.boxed_local());
            }
            Event::GetConversations { complete } => {
                pending.push(async move {
                    let rc = engine.get_conversations().await;
                    let _ = complete.send(rc.map_err(|error| error.to_string()));
                }.boxed_local());
            }
            Event::RemoveConversation { id, complete } => {
                pending.push(async move {
                    let rc = engine.remove_conversation(&id).await;
                    let _ = complete.send(rc.map_err(|error| error.to_string()));
                }.boxed_local());
            }
            Event::RemoveConversations { ids, complete } => {
                pending.push(async move {
                    let rc = engine.remove_conversations(&ids).await;
                    let _ = complete.send(rc.map_err(|error| error.to_string()));
                }.boxed_local());
            }
            Event::GetMessages {
                conversation_id,
                until,
                limit,
                offset,
                complete,
            } => {
                pending.push(async move {
                    let rc = engine.get_messages(&conversation_id, until, limit, offset).await;
                    let _ = complete.send(rc.map_err(|error| error.to_string()));
                }.boxed_local());
            }
            Event::GetMessagesInRange {
                conversation_id,
                begin,
                end,
                complete,
            } => {
                pending.push(async move {
                    let rc = engine.get_messages_in_range(&conversation_id, begin, end).await;
                    let _ = complete.send(rc.map_err(|error| error.to_string()));
                }.boxed_local());
            }
            Event::RemoveMessage { message_id, complete } => {
                pending.push(async move {
                    let rc = engine.remove_message(message_id).await;
                    let _ = complete.send(rc.map_err(|error| error.to_string()));
                }.boxed_local());
            }
            Event::RemoveMessagesByIds { message_ids, complete } => {
                pending.push(async move {
                    let rc = engine.remove_messages_by_ids(&message_ids).await;
                    let _ = complete.send(rc.map_err(|error| error.to_string()));
                }.boxed_local());
            }
            Event::RemoveMessagesInConversation { conversation_id, complete } => {
                pending.push(async move {
                    let rc = engine.remove_messages_in_conversation(&conversation_id).await;
                    let _ = complete.send(rc.map_err(|error| error.to_string()));
                }.boxed_local());
            }
            Event::GetSessions { complete } => {
                pending.push(async move {
                    let rc = engine.get_sessions().await;
                    let _ = complete.send(rc.map_err(|error| error.to_string()));
                }.boxed_local());
            }
            Event::RevokeSession { device_id, complete } => {
                pending.push(async move {
                    let rc = engine.revoke_session(&device_id).await;
                    let _ = complete.send(rc.map_err(|error| error.to_string()));
                }.boxed_local());
            }
            Event::FriendRequest {
                user_id,
                hello,
                complete,
            } => {
                pending.push(
                    async move {
                        let rc = engine.friend_request(&user_id, hello).await;
                        let _ = complete.send(rc.map_err(|e|e.to_string()));
                    }
                    .boxed_local(),
                );
            }
            Event::FriendAccept { user_id, complete } => {
                pending.push(
                    async move {
                        let rc = engine.accept_friend_request(&user_id).await;
                        let _ = complete.send(rc.map_err(|e|e.to_string()));
                    }
                    .boxed_local(),
                );
            }
            Event::GetFriendRequest { user_id, complete } => {
                pending.push(
                    async move {
                        let rc = engine.get_friend_request(&user_id).await;
                        let _ = complete.send(rc.map_err(|e|e.to_string()));
                    }
                    .boxed_local(),
                );
            }
            Event::GetFriendRequests { complete } => {
                pending.push(
                    async move {
                        let rc = engine.get_friend_requests().await;
                        let _ = complete.send(rc.map_err(|e|e.to_string()));
                    }
                    .boxed_local(),
                );
            }
            Event::RemoveFriendRequest { user_id, complete } => {
                pending.push(
                    async move {
                        let rc = engine.remove_friend_request(&user_id).await;
                        let _ = complete.send(rc.map_err(|e|e.to_string()));
                    }
                    .boxed_local(),
                );
            }
            Event::RemoveFriendRequests { user_ids, complete } => {
                pending.push(
                    async move {
                        let rc = engine.remove_friend_requests(&user_ids).await;
                        let _ = complete.send(rc.map_err(|e|e.to_string()));
                    }
                    .boxed_local(),
                );
            }
            Event::ClearFriendRequests { complete } => {
                pending.push(
                    async move {
                        let rc = engine.clear_friend_requests().await;
                        let _ = complete.send(rc.map_err(|e|e.to_string()));
                    }
                    .boxed_local(),
                );
            }
            Event::ContentMessage {
                recipient,
                headers,
                body,
                text,
                complete,
            } => {
                debug!("Verticle handling ContentMessage event for {recipient}");
                pending.push(
                    async move {
                        let mut builder = engine.message(Some(recipient));
                        for (key, value) in headers {
                            let Some(value) = value.as_str() else {
                                let _ = complete.send(Err(format!("Message header '{key}' is not text")));
                                return;
                            };
                            builder = builder.header(&key, value);
                        }
                        builder = if text {
                            match String::from_utf8(body) {
                                Ok(body) => builder.text_body(&body),
                                Err(error) => {
                                    let _ = complete.send(Err(error.to_string()));
                                    return;
                                }
                            }
                        } else {
                            builder.binary_body(body)
                        };
                        let result = builder.send().await;
                        let _ = complete.send(result.map_err(|e| e.to_string()));
                    }
                    .boxed_local(),
                );
            }
            Event::RegisterFriendSession {
                user_id,
                session_key,
                remark,
                complete,
            } => {
                pending.push(
                    async move {
                        let rc = engine.add_friend(&user_id, session_key, remark).await;
                        let _ = complete.send(rc.map_err(|error| error.to_string()));
                    }
                    .boxed_local(),
                );
            }
            Event::BlockUser { user_id, complete } => {
                pending.push(async move {
                    let rc = engine.block_user(&user_id).await;
                    let _ = complete.send(rc.map_err(|error| error.to_string()));
                }.boxed_local());
            }

            Event::CreateChannel {
                permission,
                name,
                notice,
                announcement,
                complete
            } => {
                pending.push(
                    async move {
                        let rc = engine.create_channel(permission, name, notice, announcement).await;
                        let _ = complete.send(rc.map_err(|error| error.to_string()));
                    }
                    .boxed_local(),
                );
            }
            Event::RemoveChannel { channel_id, complete } => {
                pending.push(
                    async move {
                        let rc = engine.remove_channel(&channel_id).await;
                        let _ = complete.send(rc.map_err(|error| error.to_string()));
                    }
                    .boxed_local(),
                );
            }

            Event::JoinChannel { ticket, complete } => {
                pending.push(
                    async move {
                        let rc = engine.join_channel(ticket).await;
                        let _ = complete.send(rc.map_err(|error| error.to_string()));
                    }
                    .boxed_local(),
                );
            }

            Event::LeaveChannel { channel_id, complete } => {
                pending.push(
                    async move {
                        let rc = engine.leave_channel(&channel_id).await;
                        let _ = complete.send(rc.map_err(|error| error.to_string()));
                    }
                    .boxed_local(),
                );
            }

            Event::CreateInviteTicket { channel_id, invitee, complete } => {
                pending.push(
                    async move {
                        let rc = engine.create_invite_ticket(&channel_id, invitee).await;
                        let _ = complete.send(rc.map_err(|error| error.to_string()));
                    }
                    .boxed_local(),
                );
            }

            Event::TransferChannelOwnership { channel_id, new_owner, complete } => {
                pending.push(
                    async move {
                        let rc = engine.transfer_channel_ownership(&channel_id, new_owner).await;
                        let _ = complete.send(rc.map_err(|error| error.to_string()));
                    }
                    .boxed_local(),
                );
            }

            Event::RotateChannelSessionKey { channel_id, complete } => {
                pending.push(
                    async move {
                        let rc = engine.rotate_channel_session_key(&channel_id).await;
                        let _ = complete.send(rc.map_err(|error| error.to_string()));
                    }
                    .boxed_local(),
                );
            }

            Event::UpdateChannelInfo { channel, complete } => {
                pending.push(
                    async move {
                        let rc = engine.update_channel_info(channel.as_ref()).await;
                        let _ = complete.send(rc.map_err(|error| error.to_string()));
                    }
                    .boxed_local(),
                );
            }

            Event::SetChannelMemberRole { channel_id, member_ids, role, complete } => {
                pending.push(async move {
                    let rc = engine.set_channel_members_role(&channel_id, &member_ids, role).await;
                    let _ = complete.send(rc.map_err(|error| error.to_string()));
                }.boxed_local());
            }

            Event::BanChannelMembers { channel_id, member_ids, complete } => {
                pending.push(
                    async move {
                        let rc = engine.ban_channel_members(&channel_id, &member_ids).await;
                        let _ = complete.send(rc.map_err(|error| error.to_string()));
                    }
                    .boxed_local(),
                );
            }
            Event::UnbanChannelMembers { channel_id, member_ids, complete } => {
                pending.push(
                    async move {
                        let rc = engine.unban_channel_members(&channel_id, &member_ids).await;
                        let _ = complete.send(rc.map_err(|error| error.to_string()));
                    }
                    .boxed_local(),
                );
            }
            Event::RemoveChannelMembers { channel_id, members, complete } => {
                pending.push(
                    async move {
                        let rc = engine.remove_channel_members(&channel_id, &members).await;
                        let _ = complete.send(rc.map_err(|error| error.to_string()));
                    }
                    .boxed_local(),
                );
            }

            Event::GetContact { id, complete } => {
                pending.push(
                    async move {
                        let rc = engine.get_contact(&id).await
                            .map(|contact| contact.map(|contact| contact_snapshot(contact.as_ref())));
                        let _ = complete.send(rc.map_err(|error| error.to_string()));
                    }
                    .boxed_local(),
                );
            }
            Event::GetContacts { complete } => {
                pending.push(
                    async move {
                        let rc = engine.get_contacts().await
                            .map(|contacts| contacts.into_iter()
                                .map(|contact| contact_snapshot(contact.as_ref()))
                                .collect());
                        let _ = complete.send(rc.map_err(|error| error.to_string()));
                    }
                    .boxed_local(),
                );
            }
            Event::UpdateContact { contact, complete } => {
                pending.push(
                    async move {
                        let rc = engine.update_contact(&contact).await;
                        let _ = complete.send(rc.map_err(|error| error.to_string()));
                    }
                    .boxed_local(),
                );
            }
            Event::RemoveContact { id, complete } => {
                pending.push(
                    async move {
                        let rc = engine.remove_contact(&id).await;
                        let _ = complete.send(rc.map_err(|error| error.to_string()));
                    }
                    .boxed_local(),
                );
            }
            Event::RemoveContacts { ids, complete } => {
                pending.push(
                    async move {
                        let rc = engine.remove_contacts(&ids).await;
                        let _ = complete.send(rc.map_err(|error| error.to_string()));
                    }
                    .boxed_local(),
                );
            }
            Event::ClearContacts { complete } => {
                pending.push(
                    async move {
                        let rc = engine.clear_contacts().await;
                        let _ = complete.send(rc.map_err(|error| error.to_string()));
                    }
                    .boxed_local(),
                );
            }
            Event::FriendReject { user_id, complete } => {
                pending.push(
                    async move {
                        let rc = engine.friend_reject(user_id).await;
                        let _ = complete.send(rc.map_err(|e| e.to_string()));
                    }
                    .boxed_local(),
                );
            }
            Event::FriendRemove { user_id, complete } => {
                pending.push(
                    async move {
                        let rc = engine.friend_remove(user_id).await;
                        let _ = complete.send(rc.map_err(|e| e.to_string()));
                    }
                    .boxed_local(),
                );
            }
            Event::FriendInfo { user_id, complete } => {
                pending.push(
                    async move {
                        let rc = engine.friend_info(user_id).await;
                        let _ = complete.send(rc.map_err(|e| e.to_string()));
                    }
                    .boxed_local(),
                );
            }

        }
    }

    async fn run_loop(&mut self) {
        let mut pendings = FuturesUnordered::<Pin<Box<dyn Future<Output = ()>>>>::new();
        loop {
            tokio::select! {
                event = self.event_rx.recv(), if !self.quit => {
                    match event {
                        Some(event) => self.handle_events(event, &mut pendings),
                        None => self.quit = true,
                    }
                }
                Some(_) = pendings.next(), if !pendings.is_empty() => {},
                else => break,
            }

            if self.quit && pendings.is_empty() {
                break;
            }
        }

        if let Err(e) = self.engine.stop().await {
            error!("Stopping messaging session failed: {e}");
        }
    }
}

pub(crate) fn deploy(options: VerticleOptions) -> Result<VerticleClient> {
    let (event_tx, event_rx) = mpsc::unbounded_channel::<Event>();
    let (startup_tx, startup_rx) = std_mpsc::sync_channel::<StdResult<(), String>>(1);

    let handle = std::thread::spawn(move || {
        let rt = runtime::Builder::new_current_thread()
            .enable_time()
            .enable_io()
            .build()
            .expect("Messaging verticle should be built");

        let local = task::LocalSet::new();
        rt.block_on(local.run_until(async move {
            let mut v = match Verticle::new(options, event_rx) {
                Ok(v) => v,
                Err(e) => {
                    let _ = startup_tx.send(Err(e.to_string()));
                    return;
                }
            };
            let _ = startup_tx.send(Ok(()));
            v.run_loop().await;
        }));
    });

    match startup_rx.recv() {
        Ok(Ok(())) => {
            debug!("Messaging verticle deployed successfully");
            Ok(VerticleClient::new(event_tx, handle))
        }
        Ok(Err(msg)) => {
            error!("Messaging verticle failed to deploy: {msg}");
            Err(StateError::new(msg))
        }
        Err(_) => {
            error!("Messaging verticle startup channel closed unexpectedly");
            Err(StateError::new(
                "Messaging verticle startup channel closed",
            ))
        }
    }
}
