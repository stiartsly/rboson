use log::{debug, error};
use std::{
    rc::Rc,
    result::Result as StdResult,
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc as std_mpsc, Arc,
    },
    thread::JoinHandle,
};
use tokio::{
    runtime,
    sync::{mpsc, oneshot},
    task,
};

use crate::messaging::{
    channel_listener::ChannelListener,
    client::SharedListeners,
    connection_listener::ConnectionListener,
    contact_listener::ContactListener,
    errors::{Error, Result},
    friend_request_listener::FriendRequestListener,
    message_listener::MessageListener,
    options::Options,
    session::Session,
    session_listener::SessionListener,
};
use crate::Id;

pub(crate) struct VerticleClient {
    event_tx: mpsc::UnboundedSender<VerticleEvent>,
    handle: Option<JoinHandle<()>>,
    running: Arc<AtomicBool>,
}

pub(crate) enum VerticleEvent {
    Start {
        complete: oneshot::Sender<StdResult<(), String>>,
    },
    Stop {
        complete: oneshot::Sender<StdResult<(), String>>,
    },
    FriendRequest {
        user_id: Id,
        hello: String,
        complete: oneshot::Sender<StdResult<(), String>>,
    },
    FriendAccept {
        user_id: Id,
        complete: oneshot::Sender<StdResult<(), String>>,
    },
    FriendReject {
        user_id: Id,
        complete: oneshot::Sender<StdResult<(), String>>,
    },
    FriendRemove {
        user_id: Id,
        complete: oneshot::Sender<StdResult<(), String>>,
    },
    FriendInfo {
        user_id: Id,
        complete: oneshot::Sender<StdResult<(), String>>,
    },
}

impl VerticleClient {
    fn new(
        event_tx: mpsc::UnboundedSender<VerticleEvent>,
        handle: JoinHandle<()>,
        running: Arc<AtomicBool>,
    ) -> Self {
        Self {
            event_tx,
            handle: Some(handle),
            running,
        }
    }

    pub(crate) async fn start(&self) -> Result<()> {
        debug!("VerticleClient: sending Start event to verticle");
        let (tx, rx) = oneshot::channel();
        self.event_tx
            .send(VerticleEvent::Start { complete: tx })
            .map_err(|_| Error::State("Messaging verticle event channel closed".into()))?;
        rx.await
            .map_err(|_| Error::State("Messaging verticle startup channel closed".into()))?
            .map_err(Error::State)?;
        debug!("VerticleClient: verticle started");
        Ok(())
    }

    pub(crate) async fn stop(&mut self) -> Result<()> {
        debug!("VerticleClient: sending Stop event to verticle");
        let (tx, rx) = oneshot::channel();
        if self
            .event_tx
            .send(VerticleEvent::Stop { complete: tx })
            .is_ok()
        {
            let _ = rx.await;
        }
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
        debug!("VerticleClient: verticle stopped");
        Ok(())
    }

    pub(crate) fn is_running(&self) -> bool {
        self.running.load(Ordering::Acquire)
    }

    pub(crate) fn sender(&self) -> mpsc::UnboundedSender<VerticleEvent> {
        self.event_tx.clone()
    }

    #[allow(dead_code)]
    pub(crate) async fn friend_request(&self, user_id: Id, hello: String) -> Result<()> {
        let (tx, rx) = oneshot::channel();
        self.event_tx
            .send(VerticleEvent::FriendRequest {
                user_id,
                hello,
                complete: tx,
            })
            .map_err(|_| Error::State("Messaging verticle event channel closed".into()))?;
        rx.await
            .map_err(|_| Error::State("Messaging verticle response channel closed".into()))?
            .map_err(Error::State)?;
        Ok(())
    }

    #[allow(dead_code)]
    pub(crate) async fn friend_accept(&self, user_id: Id) -> Result<()> {
        let (tx, rx) = oneshot::channel();
        self.event_tx
            .send(VerticleEvent::FriendAccept {
                user_id,
                complete: tx,
            })
            .map_err(|_| Error::State("Messaging verticle event channel closed".into()))?;
        rx.await
            .map_err(|_| Error::State("Messaging verticle response channel closed".into()))?
            .map_err(Error::State)?;
        Ok(())
    }

    #[allow(dead_code)]
    pub(crate) async fn friend_reject(&self, user_id: Id) -> Result<()> {
        let (tx, rx) = oneshot::channel();
        self.event_tx
            .send(VerticleEvent::FriendReject {
                user_id,
                complete: tx,
            })
            .map_err(|_| Error::State("Messaging verticle event channel closed".into()))?;
        rx.await
            .map_err(|_| Error::State("Messaging verticle response channel closed".into()))?
            .map_err(Error::State)?;
        Ok(())
    }

    #[allow(dead_code)]
    pub(crate) async fn friend_remove(&self, user_id: Id) -> Result<()> {
        let (tx, rx) = oneshot::channel();
        self.event_tx
            .send(VerticleEvent::FriendRemove {
                user_id,
                complete: tx,
            })
            .map_err(|_| Error::State("Messaging verticle event channel closed".into()))?;
        rx.await
            .map_err(|_| Error::State("Messaging verticle response channel closed".into()))?
            .map_err(Error::State)?;
        Ok(())
    }

    #[allow(dead_code)]
    pub(crate) async fn friend_info(&self, user_id: Id) -> Result<()> {
        let (tx, rx) = oneshot::channel();
        self.event_tx
            .send(VerticleEvent::FriendInfo {
                user_id,
                complete: tx,
            })
            .map_err(|_| Error::State("Messaging verticle event channel closed".into()))?;
        rx.await
            .map_err(|_| Error::State("Messaging verticle response channel closed".into()))?
            .map_err(Error::State)?;
        Ok(())
    }
}

pub(crate) struct VerticleOptions {
    options: Options,

    #[allow(dead_code)]
    connected: AtomicBool,
    #[allow(dead_code)]
    ready: AtomicBool,

    pub(crate) connection_listener: Arc<dyn ConnectionListener>,
    pub(crate) message_listener: Arc<dyn MessageListener>,
    pub(crate) channel_listener: Arc<dyn ChannelListener>,
    pub(crate) contact_listener: Arc<dyn ContactListener>,
    pub(crate) session_listener: Arc<dyn SessionListener>,
    pub(crate) friend_request_listener: Arc<dyn FriendRequestListener>,
}

impl VerticleOptions {
    pub(crate) fn user_id(&self) -> &Id {
        self.options.user_id()
    }

    pub(crate) fn device_id(&self) -> &Id {
        self.options.device_id()
    }

    pub(crate) fn into_options(self) -> Options {
        self.options
    }
}

pub(crate) struct Verticle {
    session: Rc<Session>,
    event_rx: mpsc::UnboundedReceiver<VerticleEvent>,
    running_flag: Arc<AtomicBool>,
    quit: bool,
}

impl Verticle {
    fn new(
        options: VerticleOptions,
        event_rx: mpsc::UnboundedReceiver<VerticleEvent>,
        running_flag: Arc<AtomicBool>,
    ) -> Result<Self> {
        let session = Rc::new(Session::new(options)?);
        Ok(Self {
            session,
            event_rx,
            running_flag,
            quit: false,
        })
    }

    fn handle_event(&mut self, event: VerticleEvent) {
        match event {
            VerticleEvent::Start { complete } => {
                debug!("Verticle handling Start event");
                let session = self.session.clone();
                let running_flag = self.running_flag.clone();
                task::spawn_local(async move {
                    let result = session.start().await;
                    if result.is_ok() {
                        running_flag.store(true, Ordering::Release);
                    }
                    let _ = complete.send(result.map_err(|e| e.to_string()));
                });
            }
            VerticleEvent::Stop { complete } => {
                debug!("Verticle handling Stop event");
                self.quit = true;
                let session = self.session.clone();
                let running_flag = self.running_flag.clone();
                task::spawn_local(async move {
                    session.stop().await;
                    running_flag.store(false, Ordering::Release);
                    let _ = complete.send(Ok(()));
                });
            }
            VerticleEvent::FriendRequest {
                user_id,
                hello,
                complete,
            } => {
                debug!("Verticle handling FriendRequest event for {user_id}");
                let session = self.session.clone();
                task::spawn_local(async move {
                    let result = session.friend_request(user_id, hello).await;
                    let _ = complete.send(result.map_err(|e| e.to_string()));
                });
            }
            VerticleEvent::FriendAccept { user_id, complete } => {
                debug!("Verticle handling FriendAccept event for {user_id}");
                let session = self.session.clone();
                task::spawn_local(async move {
                    let result = session.friend_accept(user_id).await;
                    let _ = complete.send(result.map_err(|e| e.to_string()));
                });
            }
            VerticleEvent::FriendReject { user_id, complete } => {
                debug!("Verticle handling FriendReject event for {user_id}");
                let session = self.session.clone();
                task::spawn_local(async move {
                    let result = session.friend_reject(user_id).await;
                    let _ = complete.send(result.map_err(|e| e.to_string()));
                });
            }
            VerticleEvent::FriendRemove { user_id, complete } => {
                debug!("Verticle handling FriendRemove event for {user_id}");
                let session = self.session.clone();
                task::spawn_local(async move {
                    let result = session.friend_remove(user_id).await;
                    let _ = complete.send(result.map_err(|e| e.to_string()));
                });
            }
            VerticleEvent::FriendInfo { user_id, complete } => {
                debug!("Verticle handling FriendInfo event for {user_id}");
                let session = self.session.clone();
                task::spawn_local(async move {
                    let result = session.friend_info(user_id).await;
                    let _ = complete.send(result.map_err(|e| e.to_string()));
                });
            }
        }
    }

    async fn run_loop(&mut self) {
        debug!("Verticle run loop entering event wait loop");
        while let Some(event) = self.event_rx.recv().await {
            self.handle_event(event);
            if self.quit {
                break;
            }
        }
        debug!("Verticle run loop finished");
    }
}

struct CompositeConnectionListener(Arc<SharedListeners>);
impl ConnectionListener for CompositeConnectionListener {
    fn on_connecting(&self) {
        let listeners = self.0.connection_listeners.read().unwrap().clone();
        for l in listeners {
            l.on_connecting();
        }
    }
    fn on_connected(&self) {
        self.0.connected.store(true, Ordering::Release);
        let listeners = self.0.connection_listeners.read().unwrap().clone();
        for l in listeners {
            l.on_connected();
        }
    }
    fn on_ready(&self) {
        self.0.ready.store(true, Ordering::Release);
        let listeners = self.0.connection_listeners.read().unwrap().clone();
        for l in listeners {
            l.on_ready();
        }
    }
    fn on_disconnected(&self) {
        self.0.connected.store(false, Ordering::Release);
        self.0.ready.store(false, Ordering::Release);
        let listeners = self.0.connection_listeners.read().unwrap().clone();
        for l in listeners {
            l.on_disconnected();
        }
    }
}

struct CompositeMessageListener(Arc<SharedListeners>);
impl MessageListener for CompositeMessageListener {
    fn on_message(&self, message: &dyn crate::messaging::message::Message) {
        let listeners = self.0.message_listeners.read().unwrap().clone();
        for l in listeners {
            l.on_message(message);
        }
    }
    fn on_sent(&self, message: &dyn crate::messaging::message::Message) {
        let listeners = self.0.message_listeners.read().unwrap().clone();
        for l in listeners {
            l.on_sent(message);
        }
    }
}

struct CompositeChannelListener(Arc<SharedListeners>);
impl ChannelListener for CompositeChannelListener {
    fn on_channel_created(&self, channel: &dyn crate::messaging::channel::Channel) {
        let listeners = self.0.channel_listeners.read().unwrap().clone();
        for l in listeners {
            l.on_channel_created(channel);
        }
    }
    fn on_channel_deleted(&self, channel: &dyn crate::messaging::channel::Channel) {
        let listeners = self.0.channel_listeners.read().unwrap().clone();
        for l in listeners {
            l.on_channel_deleted(channel);
        }
    }
    fn on_joined_channel(&self, channel: &dyn crate::messaging::channel::Channel) {
        let listeners = self.0.channel_listeners.read().unwrap().clone();
        for l in listeners {
            l.on_joined_channel(channel);
        }
    }
    fn on_left_channel(&self, channel: &dyn crate::messaging::channel::Channel) {
        let listeners = self.0.channel_listeners.read().unwrap().clone();
        for l in listeners {
            l.on_left_channel(channel);
        }
    }
    fn on_channel_updated(&self, channel: &dyn crate::messaging::channel::Channel) {
        let listeners = self.0.channel_listeners.read().unwrap().clone();
        for l in listeners {
            l.on_channel_updated(channel);
        }
    }
}

struct CompositeContactListener(Arc<SharedListeners>);
impl ContactListener for CompositeContactListener {
    fn on_contact_added(&self, contact: &dyn crate::messaging::contact::Contact) {
        let listeners = self.0.contact_listeners.read().unwrap().clone();
        for l in listeners {
            l.on_contact_added(contact);
        }
    }
    fn on_contacts_updated(&self, contacts: &[Box<dyn crate::messaging::contact::Contact>]) {
        let listeners = self.0.contact_listeners.read().unwrap().clone();
        for l in listeners {
            l.on_contacts_updated(contacts);
        }
    }
    fn on_contacts_removed(&self, contact_ids: &[Id]) {
        let listeners = self.0.contact_listeners.read().unwrap().clone();
        for l in listeners {
            l.on_contacts_removed(contact_ids);
        }
    }
    fn on_contacts_cleared(&self) {
        let listeners = self.0.contact_listeners.read().unwrap().clone();
        for l in listeners {
            l.on_contacts_cleared();
        }
    }
}

struct CompositeSessionListener(Arc<SharedListeners>);
impl SessionListener for CompositeSessionListener {
    fn on_new_session(&self, session_info: &crate::messaging::session_info::SessionInfo) {
        let listeners = self.0.session_listeners.read().unwrap().clone();
        for l in listeners {
            l.on_new_session(session_info);
        }
    }
}

struct CompositeFriendRequestListener(Arc<SharedListeners>);
impl FriendRequestListener for CompositeFriendRequestListener {
    fn on_friend_request(&self, user_id: &Id, hello: Option<&str>) {
        let listeners = self.0.friend_request_listeners.read().unwrap().clone();
        for l in listeners {
            l.on_friend_request(user_id, hello);
        }
    }
    fn on_friend_request_accepted(&self, user_id: &Id) {
        let listeners = self.0.friend_request_listeners.read().unwrap().clone();
        for l in listeners {
            l.on_friend_request_accepted(user_id);
        }
    }
}

pub(crate) fn deploy(options: Options, listeners: Arc<SharedListeners>) -> Result<VerticleClient> {
    debug!("Deploying messaging verticle...");
    let (event_tx, event_rx) = mpsc::unbounded_channel::<VerticleEvent>();
    let (reply_tx, reply_rx) = std_mpsc::sync_channel::<StdResult<(), String>>(1);
    let running_flag = Arc::new(AtomicBool::new(false));
    let running_clone = running_flag.clone();

    let verticle_options = VerticleOptions {
        options,
        connected: AtomicBool::new(false),
        ready: AtomicBool::new(false),
        connection_listener: Arc::new(CompositeConnectionListener(listeners.clone())),
        message_listener: Arc::new(CompositeMessageListener(listeners.clone())),
        channel_listener: Arc::new(CompositeChannelListener(listeners.clone())),
        contact_listener: Arc::new(CompositeContactListener(listeners.clone())),
        session_listener: Arc::new(CompositeSessionListener(listeners.clone())),
        friend_request_listener: Arc::new(CompositeFriendRequestListener(listeners.clone())),
    };

    let handle = std::thread::spawn(move || {
        let rt = runtime::Builder::new_current_thread()
            .enable_time()
            .enable_io()
            .build()
            .expect("Messaging runtime verticle should be built");

        let local = task::LocalSet::new();
        rt.block_on(local.run_until(async move {
            match Verticle::new(verticle_options, event_rx, running_clone) {
                Ok(mut v) => {
                    let _ = reply_tx.send(Ok(()));
                    v.run_loop().await;
                }
                Err(e) => {
                    let _ = reply_tx.send(Err(e.to_string()));
                }
            }
        }));
    });

    match reply_rx.recv() {
        Ok(Ok(())) => {
            debug!("Messaging verticle deployed successfully");
            Ok(VerticleClient::new(event_tx, handle, running_flag))
        }
        Ok(Err(msg)) => {
            error!("Messaging verticle failed to deploy: {msg}");
            Err(Error::State(msg))
        }
        Err(_) => {
            error!("Messaging verticle startup channel closed unexpectedly");
            Err(Error::State(
                "Messaging verticle startup channel closed".into(),
            ))
        }
    }
}
