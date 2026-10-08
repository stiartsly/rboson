use std::{
    collections::HashMap,
    future::Future,
    path::Path,
    pin::Pin,
    sync::{Arc, Mutex},
    sync::atomic::{AtomicBool, Ordering},
};

use crate::{Id, core::logger};
use crate::errors::{
    Result,
    NotImplemented,
    StateError,
    ArgumentError
};

use crate::messaging::{
    channel::{Channel, Permission, Role},
    channel_listener::ChannelListener,
    connection_listener::ConnectionListener,
    contact::Contact,
    contact_listener::ContactListener,
    conversation::Conversation,
    friend_request::FriendRequest,
    friend_request_listener::FriendRequestListener,
    invite_ticket::InviteTicket,
    message::{ContentDisposition, Message, MessageBuilder},
    message_listener::MessageListener,
    options::Options,
    session_info::SessionInfo,
    session_listener::SessionListener,
    MessagingClient,
    verticle::{self, VerticleClient},
};

use super::internal::PhotonContact;

/// Default maximum number of messages returned by a range query.
pub const DEFAULT_MESSAGES_LIMIT: usize = 100;

/// The Boson Messaging Client implementation.
pub struct Client {
    options: Arc<Options>,

    running: AtomicBool,
    connected: Arc<AtomicBool>,
    ready: Arc<AtomicBool>,

    connection_listener: Arc<dyn ConnectionListener>,
    message_listener: Arc<dyn MessageListener>,
    channel_listener: Arc<dyn ChannelListener>,
    contact_listener: Arc<dyn ContactListener>,
    session_listener: Arc<dyn SessionListener>,
    friend_request_listener: Arc<dyn FriendRequestListener>,
    verticle: Mutex<Option<Arc<VerticleClient>>>,
}

impl Client {
    pub fn new(options: Options) -> Self {
        if log::max_level() == log::LevelFilter::Off {
            logger::setup(options.log_level(), options.log_file());
            if options.log_console() {
                logger::enable_console_output();
            } else {
                logger::disable_console_output();
            }
        }

        let connection_listener = options.connection_listener();
        let message_listener = options.message_listener();
        let channel_listener = options.channel_listener();
        let contact_listener = options.contact_listener();
        let session_listener = options.session_listener();
        let friend_request_listener = options.friend_request_listener();

        Self {
            options: Arc::new(options),
            running: AtomicBool::new(false),
            connected: Arc::new(AtomicBool::new(false)),
            ready: Arc::new(AtomicBool::new(false)),
            connection_listener,
            message_listener,
            channel_listener,
            contact_listener,
            session_listener,
            friend_request_listener,
            verticle: Mutex::new(None),
        }
    }

    pub fn options(&self) -> &Options {
        &self.options
    }

    pub fn user_id(&self) -> &Id {
        self.options.user_id()
    }

    pub fn device_id(&self) -> &Id {
        self.options.device_id()
    }

    pub fn peer_id(&self) -> &Id {
        self.options.peer_id()
    }

    pub fn peer_endpoint(&self) -> &url::Url {
        self.options.peer_endpoint()
    }

    pub fn service_peer_id(&self) -> &Id {
        self.peer_id()
    }

    pub fn service_endpoint(&self) -> Option<&str> {
        Some(self.peer_endpoint().as_str())
    }

    pub fn data_dir(&self) -> &Path {
        self.options.data_dir()
    }

    fn verticle(&self) -> Option<Arc<VerticleClient>>  {
        self.verticle.lock().unwrap().clone()
    }

    pub async fn start(&self) -> Result<()> {
        if self.running.swap(true, Ordering::Acquire) {
            return Ok(());
        }

        let verticle = match verticle::deploy(
            self.options.clone(),
            self.connected.clone(),
            self.ready.clone(),
            self.connection_listener.clone(),
            self.message_listener.clone(),
            self.channel_listener.clone(),
            self.contact_listener.clone(),
            self.session_listener.clone(),
            self.friend_request_listener.clone(),
        ) {
            Ok(verticle) => verticle,
            Err(e) => return Err(StateError::new(&format!("{e}"))),
        };

        let _ = verticle.start().await?;
        *self.verticle.lock().unwrap() = Some(Arc::new(verticle));

        Ok(())
    }

    pub async fn stop(&self) -> Result<()> {
        let verticle = self.verticle.lock().unwrap().take();
        if let Some(mut v) = verticle {
            v.stop().await?;
        }
        Ok(())
    }

    pub fn is_running(&self) -> bool {
        self.verticle
            .lock()
            .unwrap()
            .as_ref()
            .map_or(false, |v| v.is_running())
    }

    pub fn is_connected(&self) -> bool {
        self.connected.load(Ordering::Acquire)
    }

    pub fn is_ready(&self) -> bool {
        self.ready.load(Ordering::Acquire)
    }

    pub fn message(&self, recipient: Option<Id>) -> Box<dyn MessageBuilder> {
        Box::new(ComposedMessageBuilder::new(
            recipient,
            self.verticle.lock().unwrap().as_ref().map(|v| v.sender()),
            self.message_listener.clone(),
        ))
    }

    pub async fn friend_request(
        &self,
        user_id: Id,
        hello: Option<String>
    ) -> Result<()> {
        if !self.is_connected() {
            return Err(StateError::new("Messaging client is not connected").into());
        }
        let Some(v) = self.verticle() else {
            return Err(StateError::new("Messaging client is not connected").into());
        };
        v.friend_request(user_id, hello).await
    }

    pub async fn accept_friend_request(
        &self,
        user_id: Id,
    ) -> Result<()> {
        if !self.is_connected() {
            return Err(StateError::new("Messaging client is not connected").into());
        }
        let Some(v) = self.verticle() else {
            return Err(StateError::new("Messaging client is not connected").into());
        };
        v.friend_accept(user_id).await
    }

    pub async fn get_friend_request(
        &self,
        user_id: Id,
    ) -> Result<Option<FriendRequest>> {
        if !self.is_connected() {
            return Err(StateError::new("Messaging client is not connected").into());
        }
        let Some(v) = self.verticle() else {
            return Err(StateError::new("Messaging client is not connected").into());
        };
        v.get_friend_request(user_id).await
    }

    pub async fn get_friend_requests(&self) -> Result<Vec<FriendRequest>> {
        if !self.is_connected() {
            return Err(StateError::new("Messaging client is not connected").into());
        }
        let Some(v) = self.verticle() else {
            return Err(StateError::new("Messaging client is not connected").into());
        };
        v.get_friend_requests().await
    }

    pub async fn remove_friend_request(
        &self,
        user_id: Id
    ) -> Result<()> {
        if !self.is_connected() {
            return Err(StateError::new("Messaging client is not connected").into());
        }
        let Some(v) = self.verticle() else {
            return Err(StateError::new("Messaging client is not connected").into());
        };
        v.remove_friend_request(user_id).await
    }

    pub async fn remove_friend_requests(
        &self,
        user_ids: &[Id]
    ) -> Result<()> {

        if !self.is_connected() {
            return Err(StateError::new("Messaging client is not connected").into());
        }
        let Some(v) = self.verticle() else {
            return Err(StateError::new("Messaging client is not connected").into());
        };
        v.remove_friend_requests(user_ids.to_vec()).await
    }

    pub async fn clear_friend_requests(&self) -> Result<()> {
        if !self.is_connected() {
            return Err(StateError::new("Messaging client is not connected").into());
        }
        let Some(v) = self.verticle() else {
            return Err(StateError::new("Messaging client is not connected").into());
        };
        v.clear_friend_requests().await
    }

    pub async fn add_friend(
        &self,
        user_id: Id,
        session_key: Vec<u8>,
        remark: Option<String>,
    ) -> Result<()> {

        if !self.is_connected() {
            return Err(StateError::new("Messaging client is not connected").into());
        }
        let Some(v) = self.verticle() else {
            return Err(StateError::new("Messaging client is not connected").into());
        };
        v.add_friend(user_id, session_key, remark).await
    }

    pub async fn get_contact(&self, id: &Id) -> Result<Option<Box<dyn Contact>>> {
        <Self as MessagingClient>::get_contact(self, id).await
    }

    pub async fn get_contacts(&self) -> Result<Vec<Box<dyn Contact>>> {
        <Self as MessagingClient>::get_contacts(self).await
    }

    pub async fn remove_contact(&self, id: &Id) -> Result<()> {
        <Self as MessagingClient>::remove_contact(self, id).await
    }

    pub async fn update_contact(&self, contact: &dyn Contact) -> Result<()> {
        <Self as MessagingClient>::update_contact(self, contact).await
    }

    pub async fn remove_contacts(&self, ids: &[Id]) -> Result<()> {
        <Self as MessagingClient>::remove_contacts(self, ids).await
    }

    pub async fn clear_contacts(&self) -> Result<()> {
        <Self as MessagingClient>::clear_contacts(self).await
    }
}

impl MessagingClient for Client {
    fn user_id(&self) -> &Id {
        self.user_id()
    }

    fn device_id(&self) -> &Id {
        self.device_id()
    }

    fn peer_id(&self) -> &Id {
        self.peer_id()
    }

    fn peer_endpoint(&self) -> &str {
        self.options.peer_endpoint().as_str()
    }

    fn data_dir(&self) -> &Path {
        self.options.data_dir()
    }

    async fn start(&self) -> Result<()> {
        self.start().await
    }

    async fn stop(&self) -> Result<()> {
        self.stop().await
    }

    fn is_running(&self) -> bool {
        self.is_running()
    }

    fn is_connected(&self) -> bool {
        self.is_connected()
    }

    fn is_ready(&self) -> bool {
        self.is_ready()
    }

    fn message(&self, recipient: Option<Id>) -> Box<dyn MessageBuilder> {
        self.message(recipient)
    }

    async fn get_conversation(
        &self,
        _id: &Id
    ) -> Result<Option<Box<dyn Conversation>>> {
        Err(NotImplemented::new("get_conversation"))
    }

    async fn get_conversations(&self) -> Result<Vec<Box<dyn Conversation>>> {
        Err(NotImplemented::new("get_conversations"))
    }

    async fn remove_conversation(&self, _id: &Id) -> Result<()> {
        Err(NotImplemented::new("remove_conversation"))
    }

    async fn remove_conversations(&self, _ids: &[Id]) -> Result<()> {
        Err(NotImplemented::new("remove_conversations"))
    }

    async fn get_messages(
        &self,
        _conversation_id: &Id,
        _until: Option<i64>,
        _limit: usize,
        _offset: usize,
    ) -> Result<Vec<Box<dyn Message>>> {
        Err(NotImplemented::new("get_messages"))
    }

    async fn get_messages_in_range(
        &self,
        _conversation_id: &Id,
        _begin: i64,
        _end: i64,
    ) -> Result<Vec<Box<dyn Message>>> {
        Err(NotImplemented::new("get_messages_in_range"))
    }

    async fn remove_message(&self, _message_id: i64) -> Result<()> {
        Err(NotImplemented::new("remove_message"))
    }

    async fn remove_messages_by_ids(&self, _message_ids: &[i64]) -> Result<()> {
        Err(NotImplemented::new("remove_messages_by_ids"))
    }

    async fn remove_messages_in_conversation(&self, _conversation_id: &Id) -> Result<()> {
        Err(NotImplemented::new("remove_messages_in_conversation"))
    }

    async fn get_sessions(&self) -> Result<Vec<SessionInfo>> {
        Err(NotImplemented::new("get_sessions"))
    }

    async fn revoke_session(&self, _device_id: &Id) -> Result<()> {
        Err(NotImplemented::new("revoke_session"))
    }

    async fn friend_request(&self, userid: &Id, hello: Option<String>) -> Result<()> {
        self.friend_request(*userid, hello).await
    }

    async fn accept_friend_request(&self, userid: &Id) -> Result<()> {
        self.accept_friend_request(*userid).await
    }

    async fn get_friend_request(&self,
        user_id: &Id,
    ) -> Result<Option<FriendRequest>> {
        self.get_friend_request(*user_id).await
    }

    async fn get_friend_requests(&self) -> Result<Vec<FriendRequest>> {
        self.get_friend_requests().await
    }

    async fn remove_friend_request(&self, user_id: &Id) -> Result<()> {
        self.remove_friend_request(*user_id).await
    }

    async fn remove_friend_requests(&self, user_ids: &[Id]) -> Result<()> {
        self.remove_friend_requests(user_ids).await
    }

    async fn clear_friend_requests(&self) -> Result<()> {
        self.clear_friend_requests().await
    }

    async fn add_friend(
        &self,
        user_id: &Id,
        session_key: Vec<u8>,
        remark: Option<String>,
    ) -> Result<()> {
        self.add_friend(*user_id, session_key, remark).await
    }

    async fn block_user(
        &self,
        _user_id: &Id
    ) -> Result<Box<dyn Contact>> {
        Err(NotImplemented::new("block_user"))
    }

    async fn create_channel(
        &self,
        _permission: Permission,
        _name: String,
        _notice: Option<String>,
        _announcement: Option<String>,
    ) -> Result<Box<dyn Channel>> {
        Err(NotImplemented::new("create_channel"))
    }

    async fn remove_channel(&self, _channel_id: &Id) -> Result<()> {
        Err(NotImplemented::new("remove_channel"))
    }

    async fn join_channel(&self, _ticket: InviteTicket) -> Result<Box<dyn Channel>> {
        Err(NotImplemented::new("join_channel"))
    }

    async fn leave_channel(&self, _channel_id: &Id) -> Result<()> {
        Err(NotImplemented::new("leave_channel"))
    }

    async fn create_invite_ticket(
        &self,
        _channel_id: &Id,
        _invitee: Option<Id>,
    ) -> Result<InviteTicket> {
        Err(NotImplemented::new("create_invite_ticket"))
    }

    async fn transfer_channel_ownership(
        &self,
        _channel_id: &Id,
        _new_owner: Id,
    ) -> Result<()> {
        Err(NotImplemented::new("transfer_channel_ownership"))
    }

    async fn rotate_channel_session_key(&self, _channel_id: &Id) -> Result<()> {
        Err(NotImplemented::new("rotate_channel_session_key"))
    }

    async fn update_channel_info(&self, _channel: &dyn Channel) -> Result<()> {
        Err(NotImplemented::new("update_channel_info"))
    }

    async fn set_channel_members_role(
        &self,
        _channel_id: &Id,
        _members: &[Id],
        _role: Role,
    ) -> Result<()> {
        Err(NotImplemented::new("set_channel_members_role"))
    }

    async fn ban_channel_members(&self, _channel_id: &Id, _members: &[Id]) -> Result<()> {
        Err(NotImplemented::new("ban_channel_members"))
    }

    async fn unban_channel_members(
        &self,
        _channel_id: &Id,
        _members: &[Id],
    ) -> Result<()> {
        Err(NotImplemented::new("unban_channel_members"))
    }

    async fn remove_channel_members(
        &self,
        _channel_id: &Id,
        _members: &[Id],
    ) -> Result<()> {
        Err(NotImplemented::new("remove_channel_members"))
    }

    async fn get_contact(&self, id: &Id) -> Result<Option<Box<dyn Contact>>> {
        let verticle = self.verticle.lock().unwrap();
        let verticle = verticle
            .as_ref()
            .ok_or_else(|| StateError::new("Messaging client is not running"))?;
        Ok(verticle
            .get_contact(*id)
            .await?
            .map(|contact| Box::new(contact) as Box<dyn Contact>))
    }

    async fn get_contacts(&self) -> Result<Vec<Box<dyn Contact>>> {
        let verticle = self.verticle.lock().unwrap();
        let verticle = verticle
            .as_ref()
            .ok_or_else(|| StateError::new("Messaging client is not running"))?;
        Ok(verticle
            .get_contacts()
            .await?
            .into_iter()
            .map(|contact| Box::new(contact) as Box<dyn Contact>)
            .collect())
    }

    async fn update_contact(&self, contact: &dyn Contact) -> Result<()> {
        let updated = PhotonContact {
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
        };
        let verticle = self.verticle.lock().unwrap();
        let verticle = verticle
            .as_ref()
            .ok_or_else(|| StateError::new("Messaging client is not running"))?;
        verticle.update_contact(updated).await
    }

    async fn remove_contact(&self, id: &Id) -> Result<()> {
        let verticle = self.verticle.lock().unwrap();
        let verticle = verticle
            .as_ref()
            .ok_or_else(|| StateError::new("Messaging client is not running"))?;
        verticle.remove_contact(*id).await
    }

    async fn remove_contacts(&self, ids: &[Id]) -> Result<()> {
        let verticle = self.verticle.lock().unwrap();
        let verticle = verticle
            .as_ref()
            .ok_or_else(|| StateError::new("Messaging client is not running"))?;
        verticle.remove_contacts(ids.to_vec()).await
    }

    async fn clear_contacts(&self) -> Result<()> {
        let verticle = self.verticle.lock().unwrap();
        let verticle = verticle
            .as_ref()
            .ok_or_else(|| StateError::new("Messaging client is not running"))?;
        verticle.clear_contacts().await
    }
}

#[allow(dead_code)]
struct ComposedMessageBuilder {
    recipient: Option<Id>,
    verticle_tx: Option<tokio::sync::mpsc::UnboundedSender<verticle::VerticleEvent>>,
    message_listener: Arc<dyn MessageListener>,
    content_type: Option<String>,
    content_disposition: Option<ContentDisposition>,
    headers: Vec<(String, String)>,
    body: Option<Vec<u8>>,
    text: bool,
}

impl ComposedMessageBuilder {
    fn new(
        recipient: Option<Id>,
        verticle_tx: Option<tokio::sync::mpsc::UnboundedSender<verticle::VerticleEvent>>,
        message_listener: Arc<dyn MessageListener>,
    ) -> Self {
        Self {
            recipient,
            verticle_tx,
            message_listener,
            content_type: None,
            content_disposition: None,
            headers: Vec::new(),
            body: None,
            text: false,
        }
    }
}

impl MessageBuilder for ComposedMessageBuilder {
    fn content_type(mut self: Box<Self>, content_type: &str) -> Box<dyn MessageBuilder> {
        self.content_type = Some(content_type.to_string());
        self
    }

    fn content_disposition(
        mut self: Box<Self>,
        disposition: ContentDisposition,
    ) -> Box<dyn MessageBuilder> {
        self.content_disposition = Some(disposition);
        self
    }

    fn text_body(mut self: Box<Self>, text: &str) -> Box<dyn MessageBuilder> {
        self.body = Some(text.as_bytes().to_vec());
        self.text = true;
        self
    }

    fn binary_body(mut self: Box<Self>, data: Vec<u8>) -> Box<dyn MessageBuilder> {
        self.body = Some(data);
        self.text = false;
        self
    }

    fn header(mut self: Box<Self>, key: &str, value: &str) -> Box<dyn MessageBuilder> {
        self.headers.push((key.to_string(), value.to_string()));
        self
    }

    fn send(
        self: Box<Self>,
    ) -> Pin<Box<dyn Future<Output = Result<Box<dyn Message>>> + Send + 'static>> {
        Box::pin(async move {
            let recipient = self.recipient
                .ok_or_else(|| ArgumentError::new("Message recipient is required"))?;
            let body = self.body
                .ok_or_else(|| ArgumentError::new("Message content is required"))?;
            let tx = self.verticle_tx
                .ok_or_else(|| StateError::new("Messaging client is not running"))?;
            let mut headers = HashMap::new();
            for (key, value) in self.headers {
                headers.insert(key, serde_json::Value::String(value));
            }
            if let Some(content_type) = self.content_type {
                headers.insert("Content-Type".into(), serde_json::Value::String(content_type));
            } else if !self.text {
                headers.insert(
                    "Content-Type".into(),
                    serde_json::Value::String(crate::messaging::message::content_type::BINARY.into()),
                );
            }
            if let Some(disposition) = self.content_disposition {
                headers.insert(
                    crate::messaging::message::CONTENT_DISPOSITION_HEADER.into(),
                    serde_json::Value::String(disposition.value()),
                );
            }

            let (reply_tx, reply_rx) = tokio::sync::oneshot::channel();
            tx.send(verticle::VerticleEvent::ContentMessage {
                recipient,
                headers,
                body,
                text: self.text,
                complete: reply_tx,
            })
            .map_err(|_| StateError::new("Messaging verticle event channel closed"))?;
            let message = reply_rx
                .await
                .map_err(|_| StateError::new("Messaging verticle response channel closed"))?
                .map_err(StateError::new)?;
            self.message_listener.on_sent(message.as_ref());
            Ok(message)
        })
    }
}
