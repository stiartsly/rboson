use super::prelude::*;

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
    pub(super) requests: sync::Mutex<()>,

    pub(super) origin_clock: Arc<std::sync::Mutex<i64>>,

    pub(super) pending_calls: Calls,
    pub(super) contacts_revision: Cell<i32>,
    pub(super) contact_sync_needed: Cell<bool>,

    pub(super) user_identity: CryptoIdentity,
    pub(super) device_identity: CryptoIdentity,

    pub(super) session_contexts: Arc<Mutex<HashMap<Id, SessionContext>>>,
    pub(super) blocked_contacts: Arc<Mutex<HashSet<Id>>>,

    pub(super) repository: Rc<RefCell<MessagingRepository>>,

    pub(super) friend_requests_cache: RefCell<HashMap<Id, FriendRequest>>,
    pub(super) channels_cache: RefCell<HashMap<Id, Channel>>,
    pub(super) contacts_cache: RefCell<HashMap<Id, Contact>>,
    pub(super) conversations_cache: Arc<Mutex<HashMap<Id, PhotonConversation>>>,

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
        let user_identity = CryptoIdentity::from(user_key);

        let device_key = options.options.device_key().clone();
        let device_identity = CryptoIdentity::from(device_key);

        let repository = MessagingRepository::new();

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
            requests: tokio::sync::Mutex::new(()),
            origin_clock: Arc::new(std::sync::Mutex::new(0)),

            pending_calls: Calls::default(),

            contacts_revision: Cell::new(0),
            contact_sync_needed: Cell::new(false),
            user_identity,
            device_identity,

            session_contexts: Arc::new(Mutex::new(HashMap::new())),
            blocked_contacts: Arc::new(Mutex::new(HashSet::new())),

            repository: Rc::new(RefCell::new(repository)),

            friend_requests_cache: RefCell::new(HashMap::new()),
            channels_cache: RefCell::new(HashMap::new()),
            contacts_cache: RefCell::new(HashMap::new()),
            conversations_cache: Arc::new(Mutex::new(HashMap::new())),

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
        self.close_transport("Messaging session stopped");
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
        let (revision, stored_contacts, friend_requests, conversations) = {
            let mut repository = self.repository.borrow_mut();
            (
                repository.contacts_revision()?,
                repository.load_contacts()?,
                repository.load_friend_requests()?,
                repository.load_conversations()?,
            )
        };

        let mut contacts: HashMap<Id, Contact> = HashMap::new();
        let mut sessions = HashMap::new();
        let mut channels = HashMap::new();
        for stored in stored_contacts {
            let session = match stored.session_key.as_deref() {
                Some(key) => {
                    let key = self.user_identity.decrypt_into(self.user_id(), key)?;
                    let identity = CryptoIdentity::try_from(key.as_slice())?;
                    Some(self.session_context(
                        stored.id,
                        identity,
                        stored.contact_type == ContactType::Channel as i32,
                    ))
                }
                None => None,
            };

            if stored.contact_type == ContactType::Channel as i32 {
                let Some(session) = session else {
                    warn!("Skipping persisted channel {} without a session key", stored.id);
                    continue;
                };
                let Some(metadata) = stored.channel.as_ref() else {
                    warn!("Skipping persisted channel {} without metadata", stored.id);
                    continue;
                };
                let contact = {
                    let repo = self.repository.borrow();
                    match repo.photon_contact_from_stored(&stored) {
                        Ok(contact) => contact,
                        Err(error) => {
                            warn!("Skipping invalid persisted contact {}: {error}", stored.id);
                            continue;
                        }
                    }
                };
                let members = self
                    .repository
                    .borrow_mut()
                    .photon_channel_members(&stored.id)?;
                let channel: Channel = Box::new(PhotonChannel::new(
                    contact.clone(),
                    metadata.owner,
                    *session.session_id(),
                    Permission::try_from(metadata.permission)?,
                    metadata.notice.clone(),
                    metadata.announce,
                    members,
                ));
                sessions.insert(stored.id, session);
                contacts.insert(stored.id, Box::new(contact) as Contact);
                channels.insert(stored.id, channel);
            } else {
                let contact = {
                    let repo = self.repository.borrow();
                    match repo.contact_from_stored(&stored) {
                        Ok(contact) => contact,
                        Err(error) => {
                            warn!("Skipping invalid persisted contact {}: {error}", stored.id);
                            continue;
                        }
                    }
                };
                if let Some(session) = session {
                    sessions.insert(stored.id, session);
                }
                contacts.insert(stored.id, contact);
            }
        }

        let blocked = contacts
            .values()
            .filter(|contact| contact.is_blocked())
            .map(|contact| *contact.id())
            .collect();
        *self.contacts_cache.borrow_mut() = contacts;
        *self.session_contexts.lock().unwrap() = sessions;
        *self.channels_cache.borrow_mut() = channels;
        *self.blocked_contacts.lock().unwrap() = blocked;
        *self.friend_requests_cache.borrow_mut() = friend_requests
            .into_iter()
            .map(|request| (*request.user_id(), request))
            .collect();
        *self.conversations_cache.lock().unwrap() = conversations
            .into_iter()
            .map(|conversation| (*conversation.id(), conversation))
            .collect();
        self.contacts_revision.set(revision);
        Ok(())
    }
}
