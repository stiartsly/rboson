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
    cell::{Cell, RefCell},
    collections::{HashMap, HashSet},
    rc::Rc,
    sync::{Arc, Mutex},
    sync::atomic::AtomicI64,
};
use log::{debug, info, warn};
use tokio::sync;

use boson_api::{
    CryptoContext,
    Result,
    CryptoIdentity,
    Id,
    Identity,
};
use crate::{
    contact::ContactType,
    options::Options,
    ChannelListener,
    ContactHandler,
    ConnectionListener,
    ContactListener,
    FriendRequestListener,
    FriendRequestHandler,
    MessageListener,
    SessionListener,
    ChannelMemberHandler
};
use crate::internal::{
    SessionContext,
    MessagingRepository,
    verticle::VerticleOptions,
};
use crate::rpc::call::Calls;
use super::mqtt::ClientHandle;


pub(crate) struct RpcServer {
    pub(super) options: Arc<Options>,
    pub(super) mqtt_client: ClientHandle,
    pub(super) mqtt_eventloop: tokio::sync::Mutex<Option<rumqttc::EventLoop>>,
    pub(super) running: Cell<bool>,
    pub(super) connected: Cell<bool>,
    pub(super) ready: Cell<bool>,
    pub(super) reconnect_at: Cell<Option<tokio::time::Instant>>,
    pub(super) sync_running: Cell<bool>,
    pub(super) stopping: Cell<bool>,

    pub(super) lifecycle: sync::Mutex<()>,

    pub(super) origin_timestamp: AtomicI64,

    pub(super) pending_calls: Calls,
    pub(super) contacts_revision: Cell<i32>,
    pub(super) contact_sync_needed: Cell<bool>,

    pub(super) user_identity:   CryptoIdentity,
    pub(super) device_identity: CryptoIdentity,
    pub(crate) service_context: Rc<RefCell<CryptoContext>>,
    pub(crate) self_context:    Rc<RefCell<CryptoContext>>,

    pub(super) session_contexts: Arc<Mutex<HashMap<Id, SessionContext>>>,
    pub(super) blocked_contacts: Arc<Mutex<HashSet<Id>>>,

    pub(super) repository: RefCell<MessagingRepository>,

    pub(super) connection_listener: Arc<dyn ConnectionListener>,
    pub(super) message_listener: Arc<dyn MessageListener>,
    pub(super) channel_listener: Arc<dyn ChannelListener>,
    pub(super) contact_listener: Arc<dyn ContactListener>,
    pub(super) session_listener: Arc<dyn SessionListener>,
    pub(super) friend_request_listener: Arc<dyn FriendRequestListener>,
}
impl RpcServer {
    pub(crate) fn new(options: VerticleOptions) -> Result<Rc<Self>> {
        let user_key = options.options.user_key().clone();
        let device_key = options.options.device_key().clone();

        let user_identity = CryptoIdentity::from(user_key);
        let device_identity = CryptoIdentity::from(device_key);

        let peer_id = options.options.peer_id();
        let self_context = Rc::new(RefCell::new(
            user_identity.create_crypto_context(user_identity.id())?
        ));
        let service_context = Rc::new(RefCell::new(
            device_identity.create_crypto_context(peer_id)?
        ));

        let repository = RefCell::new(MessagingRepository::new(Arc::new(user_identity.clone())));

        Ok(Rc::new(Self {
            options: options.options,
            mqtt_client: Arc::new(Mutex::new(None)),
            mqtt_eventloop: tokio::sync::Mutex::new(None),
            running: Cell::new(false),
            connected: Cell::new(false),
            ready: Cell::new(false),
            reconnect_at: Cell::new(None),
            sync_running: Cell::new(false),
            stopping: Cell::new(false),
            lifecycle: tokio::sync::Mutex::new(()),
            origin_timestamp: AtomicI64::new(0),

            pending_calls: Calls::default(),

            contacts_revision: Cell::new(0),
            contact_sync_needed: Cell::new(false),

            user_identity,
            device_identity,
            self_context,
            service_context,

            session_contexts: Arc::new(Mutex::new(HashMap::new())),
            blocked_contacts: Arc::new(Mutex::new(HashSet::new())),

            repository,

            connection_listener: options.connection_listener.clone(),
            message_listener: options.message_listener.clone(),
            channel_listener: options.channel_listener.clone(),
            contact_listener: options.contact_listener.clone(),
            session_listener: options.session_listener.clone(),
            friend_request_listener: options.friend_request_listener.clone(),
        }))
    }

    /// Builds the cached session context for a contact or channel.
    /// Connects the transport. Idempotent while already running.
    pub(crate) async fn start(self: &Rc<Self>) -> Result<()> {
        let _lifecycle = self.lifecycle.lock().await;
        self.stopping.set(false);

        if self.is_running() {
            debug!("Messaging session is already running");
            return Ok(());
        }

        if !self.repository.borrow().is_open() {
            self.repository
                .borrow_mut()
                .open(self.options.database_path())?;
        }
        self.restore_state()?;

        let session = self.clone();
        let revision = self.contacts_revision.get();
        self
            .connect(revision, move || {
                session.notify_connection(|listener| listener.on_connecting());
            })
            .await?;
        self.reconnect_at.set(None);
        self.running.set(true);
        Ok(())
    }

    /// Disconnects the transport and releases the event loop.
    pub(crate) async fn stop(&self) -> Result<()> {
        let _lifecycle = self.lifecycle.lock().await;
        self.stopping.set(true);
        let result = self.stop_transport().await;
        self.close_transport();
        self.clear_eventloop();
        self.repository.borrow_mut().close();
        result
    }

    pub(crate) fn is_running(&self) -> bool {
        self.running.get()
    }

    pub(crate) fn is_ready(&self) -> bool {
        self.ready.get()
    }

    async fn stop_transport(&self) -> Result<()> {
        info!("Stopping messaging session...");
        if !self.is_open() {
            return Ok(());
        }
        debug!("Disconnecting MQTT client...");
        let result = self.disconnect().await;

        if self.connected.replace(false) {
            self.notify_connection(|l| l.on_disconnected());
        }
        info!("Messaging session stopped");
        result
    }

    fn restore_state(&self) -> Result<()> {
        let (revision, stored_contacts) = {
            let mut repository = self.repository.borrow_mut();
            (
                repository.contacts_revision()?,
                repository.load_contacts()?,
            )
        };

        let mut sessions = HashMap::new();
        let mut blocked = HashSet::new();
        for stored in stored_contacts {
            if let Some(key) = stored.session_key() {
                let key = self.user_identity.decrypt_into(self.user_id(), key)?;
                let identity = CryptoIdentity::try_from(key.as_slice())?;
                sessions.insert(
                    stored.id,
                    self.session_context(
                        stored.id,
                        identity,
                        stored.contact_type == ContactType::Channel as i32,
                    ),
                );
            }
            if stored.is_blocked() {
                blocked.insert(stored.id);
            }
        }
        *self.session_contexts.lock().unwrap() = sessions;
        *self.blocked_contacts.lock().unwrap() = blocked;
        self.contacts_revision.set(revision);
        Ok(())
    }

    /// Reclaims expired local records; run periodically by the verticle timer.
    pub(crate) fn maintenance(&self) {
        let requests = self
            .repository
            .borrow_mut()
            .load_friend_requests()
            .unwrap_or_default();
        let mut removed = 0;
        for request in requests {
            if request.is_expired() {
                if let Err(error) = self
                    .repository
                    .borrow_mut()
                    .remove_friend_request(request.user_id())
                {
                    warn!("Failed to reclaim expired friend request: {error}");
                } else {
                    removed += 1;
                }
            }
        }
        if removed > 0 {
            debug!("Reclaimed {removed} expired friend request(s)");
        }
    }
}
