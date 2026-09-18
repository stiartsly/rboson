use std::{
    path::Path,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex, RwLock,
    },
    time::Duration,
};

use rand::Rng;
use rumqttc::{AsyncClient, Event, Incoming, MqttOptions, QoS, SubscribeFilter, Transport};

use crate::{
    messaging::{
        channel::{Channel, Permission, Role},
        channel_listener::ChannelListener,
        client::{BoxFuture, MessagingClient},
        config::Configuration,
        connection_listener::ConnectionListener,
        contact::Contact,
        contact_listener::ContactListener,
        conversation::Conversation,
        errors::{Error, Result},
        friend_request::FriendRequest,
        friend_request_listener::FriendRequestListener,
        invite_ticket::InviteTicket,
        message::{ContentDisposition, Message, MessageBuilder},
        message_listener::MessageListener,
        session_info::SessionInfo,
        session_listener::SessionListener,
    },
    Id,
};

const USER_INBOX: &str = "u/i";
const USER_OUTBOX: &str = "u/o";
const DEVICE_INBOX: &str = "d/i";
const MAX_MESSAGE_SIZE: usize = 256 * 1024;

#[derive(Default)]
pub(crate) struct InitialListeners {
    pub connection: Option<Arc<dyn ConnectionListener>>,
    pub message: Option<Arc<dyn MessageListener>>,
    pub channel: Option<Arc<dyn ChannelListener>>,
    pub contact: Option<Arc<dyn ContactListener>>,
    pub session: Option<Arc<dyn SessionListener>>,
    pub friend_request: Option<Arc<dyn FriendRequestListener>>,
}

struct Shared {
    running: AtomicBool,
    connected: AtomicBool,
    ready: AtomicBool,
    mqtt: Mutex<Option<AsyncClient>>,
    worker: Mutex<Option<tokio::task::JoinHandle<()>>>,
    connection_listeners: RwLock<Vec<Arc<dyn ConnectionListener>>>,
    message_listeners: RwLock<Vec<Arc<dyn MessageListener>>>,
    channel_listeners: RwLock<Vec<Arc<dyn ChannelListener>>>,
    contact_listeners: RwLock<Vec<Arc<dyn ContactListener>>>,
    session_listeners: RwLock<Vec<Arc<dyn SessionListener>>>,
    friend_request_listeners: RwLock<Vec<Arc<dyn FriendRequestListener>>>,
}

/// MQTT-backed implementation of the public [`MessagingClient`] API.
///
/// Connection setup and authentication match the Java Photon client. Service
/// RPC, encrypted envelopes, and persistence are deliberately not reported as
/// successful until their updated wire formats have been ported.
pub struct PhotonMessagingClient {
    config: Configuration,
    user_id: Id,
    device_id: Id,
    shared: Arc<Shared>,
}

impl PhotonMessagingClient {
    pub(crate) fn new(config: Configuration, listeners: InitialListeners) -> Self {
        let shared = Shared {
            running: AtomicBool::new(false),
            connected: AtomicBool::new(false),
            ready: AtomicBool::new(false),
            mqtt: Mutex::new(None),
            worker: Mutex::new(None),
            connection_listeners: RwLock::new(listeners.connection.into_iter().collect()),
            message_listeners: RwLock::new(listeners.message.into_iter().collect()),
            channel_listeners: RwLock::new(listeners.channel.into_iter().collect()),
            contact_listeners: RwLock::new(listeners.contact.into_iter().collect()),
            session_listeners: RwLock::new(listeners.session.into_iter().collect()),
            friend_request_listeners: RwLock::new(listeners.friend_request.into_iter().collect()),
        };

        Self {
            user_id: Id::from(config.user_key.public_key()),
            device_id: Id::from(config.device_key.public_key()),
            config,
            shared: Arc::new(shared),
        }
    }

    fn unavailable<T>(&self, operation: &str) -> Result<T> {
        let state = if self.is_running() {
            "the updated encrypted RPC/persistence runtime has not been ported"
        } else {
            "the messaging client is not running"
        };
        Err(Error::State(format!("{operation} is unavailable: {state}")))
    }

    fn password(&self) -> Result<String> {
        let mut nonce = [0_u8; 16];
        rand::rng().fill_bytes(&mut nonce);
        let signature = self
            .config
            .device_key
            .private_key()
            .sign_into(&nonce)
            .map_err(|error| Error::Auth(error.to_string()))?;
        let mut password = Vec::with_capacity(nonce.len() + signature.len());
        password.extend_from_slice(&nonce);
        password.extend_from_slice(&signature);
        Ok(bs58::encode(password).into_string())
    }

    fn notify_connection(&self, callback: impl Fn(&dyn ConnectionListener)) {
        let listeners = self.shared.connection_listeners.read().unwrap().clone();
        for listener in listeners {
            callback(listener.as_ref());
        }
    }

    fn remove_listener<T: ?Sized>(listeners: &RwLock<Vec<Arc<T>>>, target: &Arc<T>) {
        listeners
            .write()
            .unwrap()
            .retain(|listener| !Arc::ptr_eq(listener, target));
    }

    fn unsupported<'a, T: Send + 'a>(
        &'a self,
        operation: &'static str,
    ) -> BoxFuture<'a, Result<T>> {
        Box::pin(async move { self.unavailable(operation) })
    }
}

impl MessagingClient for PhotonMessagingClient {
    fn user_id(&self) -> &Id {
        &self.user_id
    }

    fn device_id(&self) -> &Id {
        &self.device_id
    }

    fn service_peer_id(&self) -> &Id {
        &self.config.service_peer_id
    }

    fn service_endpoint(&self) -> Option<&str> {
        self.config.service_endpoint.as_ref().map(url::Url::as_str)
    }

    fn data_dir(&self) -> &Path {
        &self.config.data_dir
    }

    fn start(&self) -> BoxFuture<'_, Result<()>> {
        Box::pin(async move {
            if self
                .shared
                .running
                .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
                .is_err()
            {
                return Ok(());
            }

            let result = async {
                let endpoint = self.config.service_endpoint.as_ref().ok_or_else(|| {
                    Error::State(
                        "service.endpoint is required: DHT service discovery is not yet wired \
                         into the updated Rust messaging Configuration"
                            .into(),
                    )
                })?;
                tokio::fs::create_dir_all(&self.config.data_dir).await?;

                let host = endpoint
                    .host_str()
                    .ok_or_else(|| Error::Argument("service endpoint has no hostname".into()))?;
                let port = endpoint
                    .port()
                    .ok_or_else(|| Error::Argument("service endpoint has no port".into()))?;

                self.notify_connection(|listener| listener.on_connecting());

                let mut options =
                    MqttOptions::new(self.device_id.to_string(), host.to_string(), port);
                options
                    .set_credentials(
                        self.user_id.to_string(),
                        format!("{}?contactsRevision=0", self.password()?),
                    )
                    .set_keep_alive(Duration::from_secs(30))
                    .set_clean_session(false)
                    .set_max_packet_size(MAX_MESSAGE_SIZE, MAX_MESSAGE_SIZE);
                if endpoint.scheme() == "mqtts" {
                    options.set_transport(Transport::tls_with_default_config());
                }

                let (mqtt, mut eventloop) = AsyncClient::new(options, 32);
                mqtt.subscribe_many([
                    SubscribeFilter::new(USER_INBOX.to_string(), QoS::AtLeastOnce),
                    SubscribeFilter::new(USER_OUTBOX.to_string(), QoS::AtLeastOnce),
                    SubscribeFilter::new(DEVICE_INBOX.to_string(), QoS::AtLeastOnce),
                ])
                .await
                .map_err(|error| Error::Io(std::io::Error::other(error)))?;

                *self.shared.mqtt.lock().unwrap() = Some(mqtt);
                let shared = self.shared.clone();
                let worker = tokio::spawn(async move {
                    while shared.running.load(Ordering::Acquire) {
                        match eventloop.poll().await {
                            Ok(Event::Incoming(Incoming::SubAck(_))) => {
                                if !shared.connected.swap(true, Ordering::AcqRel) {
                                    let listeners =
                                        shared.connection_listeners.read().unwrap().clone();
                                    for listener in listeners {
                                        listener.on_connected();
                                    }
                                }
                            }
                            Ok(Event::Incoming(Incoming::Publish(_))) => {
                                // The contact-sync/RPC envelope is intentionally not guessed here.
                                // Consequently `ready` stays false until that protocol is ported.
                            }
                            Ok(_) => {}
                            Err(error) => {
                                if shared.connected.swap(false, Ordering::AcqRel) {
                                    shared.ready.store(false, Ordering::Release);
                                    let listeners =
                                        shared.connection_listeners.read().unwrap().clone();
                                    for listener in listeners {
                                        listener.on_disconnected();
                                    }
                                }
                                if shared.running.load(Ordering::Acquire) {
                                    log::warn!("Messaging MQTT connection error: {error}");
                                    tokio::time::sleep(Duration::from_secs(2)).await;
                                }
                            }
                        }
                    }
                    if shared.connected.swap(false, Ordering::AcqRel) {
                        let listeners = shared.connection_listeners.read().unwrap().clone();
                        for listener in listeners {
                            listener.on_disconnected();
                        }
                    }
                    shared.ready.store(false, Ordering::Release);
                });
                *self.shared.worker.lock().unwrap() = Some(worker);
                Ok(())
            }
            .await;

            if result.is_err() {
                self.shared.running.store(false, Ordering::Release);
            }
            result
        })
    }

    fn stop(&self) -> BoxFuture<'_, Result<()>> {
        Box::pin(async move {
            if !self.shared.running.swap(false, Ordering::AcqRel) {
                return Ok(());
            }

            let mqtt = self.shared.mqtt.lock().unwrap().take();
            if let Some(mqtt) = mqtt {
                let _ = mqtt.disconnect().await;
            }
            let worker = self.shared.worker.lock().unwrap().take();
            if let Some(mut worker) = worker {
                if tokio::time::timeout(Duration::from_secs(2), &mut worker)
                    .await
                    .is_err()
                {
                    worker.abort();
                    let _ = worker.await;
                }
            }
            self.shared.connected.store(false, Ordering::Release);
            self.shared.ready.store(false, Ordering::Release);
            Ok(())
        })
    }

    fn is_running(&self) -> bool {
        self.shared.running.load(Ordering::Acquire)
    }

    fn is_connected(&self) -> bool {
        self.shared.connected.load(Ordering::Acquire)
    }

    fn is_ready(&self) -> bool {
        self.shared.ready.load(Ordering::Acquire)
    }

    fn add_connection_listener(&self, listener: Arc<dyn ConnectionListener>) {
        self.shared
            .connection_listeners
            .write()
            .unwrap()
            .push(listener);
    }

    fn remove_connection_listener(&self, listener: &Arc<dyn ConnectionListener>) {
        Self::remove_listener(&self.shared.connection_listeners, listener);
    }

    fn add_message_listener(&self, listener: Arc<dyn MessageListener>) {
        self.shared
            .message_listeners
            .write()
            .unwrap()
            .push(listener);
    }

    fn remove_message_listener(&self, listener: &Arc<dyn MessageListener>) {
        Self::remove_listener(&self.shared.message_listeners, listener);
    }

    fn add_channel_listener(&self, listener: Arc<dyn ChannelListener>) {
        self.shared
            .channel_listeners
            .write()
            .unwrap()
            .push(listener);
    }

    fn remove_channel_listener(&self, listener: &Arc<dyn ChannelListener>) {
        Self::remove_listener(&self.shared.channel_listeners, listener);
    }

    fn add_contact_listener(&self, listener: Arc<dyn ContactListener>) {
        self.shared
            .contact_listeners
            .write()
            .unwrap()
            .push(listener);
    }

    fn remove_contact_listener(&self, listener: &Arc<dyn ContactListener>) {
        Self::remove_listener(&self.shared.contact_listeners, listener);
    }

    fn add_session_listener(&self, listener: Arc<dyn SessionListener>) {
        self.shared
            .session_listeners
            .write()
            .unwrap()
            .push(listener);
    }

    fn remove_session_listener(&self, listener: &Arc<dyn SessionListener>) {
        Self::remove_listener(&self.shared.session_listeners, listener);
    }

    fn add_friend_request_listener(&self, listener: Arc<dyn FriendRequestListener>) {
        self.shared
            .friend_request_listeners
            .write()
            .unwrap()
            .push(listener);
    }

    fn remove_friend_request_listener(&self, listener: &Arc<dyn FriendRequestListener>) {
        Self::remove_listener(&self.shared.friend_request_listeners, listener);
    }

    fn remove_all_listeners(&self) {
        self.shared.connection_listeners.write().unwrap().clear();
        self.shared.message_listeners.write().unwrap().clear();
        self.shared.channel_listeners.write().unwrap().clear();
        self.shared.contact_listeners.write().unwrap().clear();
        self.shared.session_listeners.write().unwrap().clear();
        self.shared
            .friend_request_listeners
            .write()
            .unwrap()
            .clear();
    }

    fn message(&self, recipient: Option<Id>) -> Box<dyn MessageBuilder> {
        Box::new(ComposedMessageBuilder::new(recipient))
    }

    fn get_conversation(&self, _id: &Id) -> BoxFuture<'_, Result<Option<Box<dyn Conversation>>>> {
        self.unsupported("get_conversation")
    }

    fn get_conversations(&self) -> BoxFuture<'_, Result<Vec<Box<dyn Conversation>>>> {
        self.unsupported("get_conversations")
    }

    fn remove_conversation(&self, _id: &Id) -> BoxFuture<'_, Result<()>> {
        self.unsupported("remove_conversation")
    }

    fn remove_conversations(&self, _ids: &[Id]) -> BoxFuture<'_, Result<()>> {
        self.unsupported("remove_conversations")
    }

    fn get_messages(
        &self,
        _conversation_id: &Id,
        _until: Option<i64>,
        _limit: usize,
        _offset: usize,
    ) -> BoxFuture<'_, Result<Vec<Box<dyn Message>>>> {
        self.unsupported("get_messages")
    }

    fn get_messages_in_range(
        &self,
        _conversation_id: &Id,
        _begin: i64,
        _end: i64,
    ) -> BoxFuture<'_, Result<Vec<Box<dyn Message>>>> {
        self.unsupported("get_messages_in_range")
    }

    fn remove_message(&self, _message_id: i64) -> BoxFuture<'_, Result<()>> {
        self.unsupported("remove_message")
    }

    fn remove_messages_by_ids(&self, _message_ids: &[i64]) -> BoxFuture<'_, Result<()>> {
        self.unsupported("remove_messages_by_ids")
    }

    fn remove_messages_in_conversation(&self, _conversation_id: &Id) -> BoxFuture<'_, Result<()>> {
        self.unsupported("remove_messages_in_conversation")
    }

    fn get_sessions(&self) -> BoxFuture<'_, Result<Vec<SessionInfo>>> {
        self.unsupported("get_sessions")
    }

    fn revoke_session(&self, _device_id: &Id) -> BoxFuture<'_, Result<()>> {
        self.unsupported("revoke_session")
    }

    fn friend_request(&self, _user_id: Id, _hello: Option<String>) -> BoxFuture<'_, Result<()>> {
        self.unsupported("friend_request")
    }

    fn accept_friend_request(&self, _user_id: &Id) -> BoxFuture<'_, Result<()>> {
        self.unsupported("accept_friend_request")
    }

    fn get_friend_request(
        &self,
        _user_id: &Id,
    ) -> BoxFuture<'_, Result<Option<Box<dyn FriendRequest>>>> {
        self.unsupported("get_friend_request")
    }

    fn get_friend_requests(&self) -> BoxFuture<'_, Result<Vec<Box<dyn FriendRequest>>>> {
        self.unsupported("get_friend_requests")
    }

    fn remove_friend_request(&self, _user_id: &Id) -> BoxFuture<'_, Result<()>> {
        self.unsupported("remove_friend_request")
    }

    fn remove_friend_requests(&self, _user_ids: &[Id]) -> BoxFuture<'_, Result<()>> {
        self.unsupported("remove_friend_requests")
    }

    fn clear_friend_requests(&self) -> BoxFuture<'_, Result<()>> {
        self.unsupported("clear_friend_requests")
    }

    fn add_friend(
        &self,
        _user_id: Id,
        _session_key: Vec<u8>,
        _remark: Option<String>,
    ) -> BoxFuture<'_, Result<()>> {
        self.unsupported("add_friend")
    }

    fn create_channel(
        &self,
        _permission: Permission,
        _name: String,
        _notice: Option<String>,
        _announcement: Option<String>,
    ) -> BoxFuture<'_, Result<Box<dyn Channel>>> {
        self.unsupported("create_channel")
    }

    fn remove_channel(&self, _channel_id: &Id) -> BoxFuture<'_, Result<()>> {
        self.unsupported("remove_channel")
    }

    fn join_channel(&self, _ticket: InviteTicket) -> BoxFuture<'_, Result<Box<dyn Channel>>> {
        self.unsupported("join_channel")
    }

    fn leave_channel(&self, _channel_id: &Id) -> BoxFuture<'_, Result<()>> {
        self.unsupported("leave_channel")
    }

    fn create_invite_ticket(
        &self,
        _channel_id: &Id,
        _invitee: Option<Id>,
    ) -> BoxFuture<'_, Result<InviteTicket>> {
        self.unsupported("create_invite_ticket")
    }

    fn transfer_channel_ownership(
        &self,
        _channel_id: &Id,
        _new_owner: Id,
    ) -> BoxFuture<'_, Result<()>> {
        self.unsupported("transfer_channel_ownership")
    }

    fn rotate_channel_session_key(&self, _channel_id: &Id) -> BoxFuture<'_, Result<()>> {
        self.unsupported("rotate_channel_session_key")
    }

    fn update_channel_info(&self, _channel: &dyn Channel) -> BoxFuture<'_, Result<()>> {
        self.unsupported("update_channel_info")
    }

    fn set_channel_members_role(
        &self,
        _channel_id: &Id,
        _members: &[Id],
        _role: Role,
    ) -> BoxFuture<'_, Result<()>> {
        self.unsupported("set_channel_members_role")
    }

    fn ban_channel_members(&self, _channel_id: &Id, _members: &[Id]) -> BoxFuture<'_, Result<()>> {
        self.unsupported("ban_channel_members")
    }

    fn unban_channel_members(
        &self,
        _channel_id: &Id,
        _members: &[Id],
    ) -> BoxFuture<'_, Result<()>> {
        self.unsupported("unban_channel_members")
    }

    fn remove_channel_members(
        &self,
        _channel_id: &Id,
        _members: &[Id],
    ) -> BoxFuture<'_, Result<()>> {
        self.unsupported("remove_channel_members")
    }

    fn get_contact(&self, _id: &Id) -> BoxFuture<'_, Result<Option<Box<dyn Contact>>>> {
        self.unsupported("get_contact")
    }

    fn get_contacts(&self) -> BoxFuture<'_, Result<Vec<Box<dyn Contact>>>> {
        self.unsupported("get_contacts")
    }

    fn update_contact(&self, _contact: &dyn Contact) -> BoxFuture<'_, Result<()>> {
        self.unsupported("update_contact")
    }

    fn remove_contact(&self, _id: &Id) -> BoxFuture<'_, Result<()>> {
        self.unsupported("remove_contact")
    }

    fn remove_contacts(&self, _ids: &[Id]) -> BoxFuture<'_, Result<()>> {
        self.unsupported("remove_contacts")
    }

    fn clear_contacts(&self) -> BoxFuture<'_, Result<()>> {
        self.unsupported("clear_contacts")
    }
}

#[allow(dead_code)]
struct ComposedMessageBuilder {
    recipient: Option<Id>,
    content_type: Option<String>,
    content_disposition: Option<ContentDisposition>,
    headers: Vec<(String, String)>,
    body: Option<Vec<u8>>,
}

impl ComposedMessageBuilder {
    fn new(recipient: Option<Id>) -> Self {
        Self {
            recipient,
            content_type: None,
            content_disposition: None,
            headers: Vec::new(),
            body: None,
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
        self
    }

    fn binary_body(mut self: Box<Self>, data: Vec<u8>) -> Box<dyn MessageBuilder> {
        self.body = Some(data);
        self
    }

    fn header(mut self: Box<Self>, key: &str, value: &str) -> Box<dyn MessageBuilder> {
        self.headers.push((key.to_string(), value.to_string()));
        self
    }
}

#[cfg(test)]
mod tests {
    use super::PhotonMessagingClient;
    use crate::{
        messaging::{
            client::MessagingClient, config::Configuration,
            photon_messaging_client::InitialListeners,
        },
        signature, Id,
    };

    #[tokio::test]
    async fn start_without_endpoint_fails_and_rolls_back_running_state() {
        let config = Configuration::new(
            Id::random(),
            None,
            signature::KeyPair::random(),
            signature::KeyPair::random(),
            None,
        );
        let client = PhotonMessagingClient::new(config, InitialListeners::default());

        let error = client.start().await.unwrap_err();
        assert!(error.to_string().contains("service.endpoint is required"));
        assert!(!client.is_running());
        assert!(!client.is_connected());
        assert!(!client.is_ready());
    }

    #[test]
    fn identities_are_derived_from_configuration_keys() {
        let user_key = signature::KeyPair::random();
        let device_key = signature::KeyPair::random();
        let expected_user = Id::from(user_key.public_key());
        let expected_device = Id::from(device_key.public_key());
        let config = Configuration::new(Id::random(), None, user_key, device_key, None);
        let client = PhotonMessagingClient::new(config, InitialListeners::default());

        assert_eq!(client.user_id(), &expected_user);
        assert_eq!(client.device_id(), &expected_device);
    }
}
