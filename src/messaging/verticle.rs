use log::{debug, error};
use std::{
    rc::Rc,
    result::Result as StdResult,
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc as std_mpsc, Arc, Mutex,
    },
    thread::JoinHandle,
};
use tokio::{
    runtime,
    sync::{mpsc, oneshot},
    task,
};

use crate::Id;

use crate::errors::{Result, StateError};
use crate::messaging::{
    channel_listener::ChannelListener,
    connection_listener::ConnectionListener,
    contact::Contact,
    contact_listener::ContactListener,
    message::Message,
    message_listener::MessageListener,
    options::Options,
    session::{Session, SessionAgent},
    session_listener::SessionListener,
    MessagingClient,
    FriendRequest,
};
use super::internal::{
    PhotonContact,
};

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
    event_tx: mpsc::UnboundedSender<VerticleEvent>,
    handle: Arc<Mutex<Option<JoinHandle<()>>>>,
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
        hello: Option<String>,
        complete: oneshot::Sender<StdResult<(), String>>,
    },
    FriendAccept {
        user_id: Id,
        complete: oneshot::Sender<StdResult<(), String>>,
    },
    GetFriendRequest {
        user_id: Id,
        complete: oneshot::Sender<StdResult<Option<FriendRequest>, String>>,
    },
    GetFriendRequests {
        complete: oneshot::Sender<StdResult<Vec<FriendRequest>, String>>,
    },
    RemoveFriendRequest {
        user_id: Id,
        complete: oneshot::Sender<StdResult<(), String>>,
    },
    RemoveFriendRequests {
        user_ids: Vec<Id>,
        complete: oneshot::Sender<StdResult<(), String>>,
    },
    ClearFriendRequests {
        complete: oneshot::Sender<StdResult<(), String>>,
    },
    ContentMessage {
        recipient: Id,
        headers: std::collections::HashMap<String, serde_json::Value>,
        body: Vec<u8>,
        text: bool,
        complete: oneshot::Sender<StdResult<Box<dyn Message>, String>>,
    },
    RegisterFriendSession {
        user_id: Id,
        session_key: Vec<u8>,
        remark: Option<String>,
        complete: oneshot::Sender<StdResult<(), String>>,
    },
    GetContact {
        id: Id,
        complete: oneshot::Sender<StdResult<Option<PhotonContact>, String>>,
    },
    GetContacts {
        complete: oneshot::Sender<StdResult<Vec<PhotonContact>, String>>,
    },
    UpdateContact {
        contact: PhotonContact,
        complete: oneshot::Sender<StdResult<(), String>>,
    },
    RemoveContact {
        id: Id,
        complete: oneshot::Sender<StdResult<(), String>>,
    },
    RemoveContacts {
        ids: Vec<Id>,
        complete: oneshot::Sender<StdResult<(), String>>,
    },
    ClearContacts {
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
            handle: Arc::new(Mutex::new(Some(handle))),
            running,
        }
    }

    pub(crate) async fn start(&self) -> Result<()> {
        debug!("VerticleClient: sending Start event to verticle");
        let (tx, rx) = oneshot::channel();
        self.event_tx
            .send(VerticleEvent::Start { complete: tx })
            .map_err(|_| StateError::new("Messaging verticle event channel closed"))?;
        rx.await
            .map_err(|_| StateError::new("Messaging verticle startup channel closed"))?
            .map_err(StateError::new)?;
        debug!("VerticleClient: verticle started");
        Ok(())
    }

    pub(crate) async fn stop(&self) -> Result<()> {
        debug!("VerticleClient: sending Stop event to verticle");
        let (tx, rx) = oneshot::channel();
        let result = if self
            .event_tx
            .send(VerticleEvent::Stop { complete: tx })
            .is_ok()
        {
            rx.await
                .map_err(|_| -> crate::errors::Error {
                    StateError::new("Messaging verticle shutdown channel closed")
                })
                .and_then(|result| result.map_err(|error| -> crate::errors::Error {
                    StateError::new(error)
                }))
        } else {
            Err(StateError::new("Messaging verticle event channel closed").into())
        };
        let handle = self.handle.lock().unwrap().take();
        if let Some(handle) = handle {
            handle.join()
                .map_err(|_| StateError::new("Messaging verticle thread panicked"))?;
        }
        debug!("VerticleClient: verticle stopped");
        result
    }

    pub(crate) fn is_running(&self) -> bool {
        self.running.load(Ordering::Acquire)
    }

    pub(crate) fn sender(&self) -> mpsc::UnboundedSender<VerticleEvent> {
        self.event_tx.clone()
    }

    pub(crate) async fn friend_request(
        &self,
        user_id: Id,
        hello: Option<String>
    ) -> Result<()> {
        let (tx, rx) = oneshot::channel();
        self.event_tx
            .send(VerticleEvent::FriendRequest {
                user_id,
                hello,
                complete: tx,
            })
            .map_err(|_| StateError::new("Messaging verticle event channel closed"))?;
        Ok(rx.await
            .map_err(|_| StateError::new("Messaging verticle response channel closed"))?
            .map_err(StateError::new)?)
    }

    pub(crate) async fn friend_accept(
        &self,
        user_id: Id
    ) -> Result<()> {
        let (tx, rx) = oneshot::channel();
        self.event_tx
            .send(VerticleEvent::FriendAccept {
                user_id,
                complete: tx,
            })
            .map_err(|_| StateError::new("Messaging verticle event channel closed"))?;
        Ok(rx.await
            .map_err(|_| StateError::new("Messaging verticle response channel closed"))?
            .map_err(StateError::new)?)
    }

    pub(crate) async fn get_friend_request(
        &self,
        user_id: Id
    ) -> Result<Option<FriendRequest>> {
        let (tx, rx) = oneshot::channel();
        self.event_tx
            .send(VerticleEvent::GetFriendRequest {
                user_id,
                complete: tx,
            })
            .map_err(|_| StateError::new("Messaging verticle event channel closed"))?;
        Ok(rx.await
            .map_err(|_| StateError::new("Messaging verticle response channel closed"))?
            .map_err(StateError::new)?)
    }

    pub(crate) async fn get_friend_requests(&self) -> Result<Vec<FriendRequest>> {
        let (tx, rx) = oneshot::channel();
        self.event_tx
            .send(VerticleEvent::GetFriendRequests { complete: tx })
            .map_err(|_| StateError::new("Messaging verticle event channel closed"))?;
        Ok(rx.await
            .map_err(|_| StateError::new("Messaging verticle response channel closed"))?
            .map_err(StateError::new)?)
    }

    pub(crate) async fn remove_friend_request(
        &self,
        user_id: Id
    ) -> Result<()> {
        let (tx, rx) = oneshot::channel();
        self.event_tx
            .send(VerticleEvent::RemoveFriendRequest {
                user_id,
                complete: tx,
            })
            .map_err(|_| StateError::new("Messaging verticle event channel closed"))?;
        Ok(rx.await
            .map_err(|_| StateError::new("Messaging verticle response channel closed"))?
            .map_err(StateError::new)?)
    }

    pub(crate) async fn remove_friend_requests(
        &self,
        user_ids: Vec<Id>
    ) -> Result<()> {
        let (tx, rx) = oneshot::channel();
        self.event_tx
            .send(VerticleEvent::RemoveFriendRequests {
                user_ids,
                complete: tx,
            })
            .map_err(|_| StateError::new("Messaging verticle event channel closed"))?;
        Ok(rx.await
            .map_err(|_| StateError::new("Messaging verticle response channel closed"))?
            .map_err(StateError::new)?)
    }

    pub(crate) async fn clear_friend_requests(&self) -> Result<()> {
        let (tx, rx) = oneshot::channel();
        self.event_tx
            .send(VerticleEvent::ClearFriendRequests { complete: tx })
            .map_err(|_| StateError::new("Messaging verticle event channel closed"))?;
        Ok(rx.await
            .map_err(|_| StateError::new("Messaging verticle response channel closed"))?
            .map_err(StateError::new)?)
    }

    pub(crate) async fn add_friend(
        &self,
        user_id: Id,
        session_key: Vec<u8>,
        remark: Option<String>,
    ) -> Result<()> {
        let (tx, rx) = oneshot::channel();
        self.event_tx
            .send(VerticleEvent::RegisterFriendSession {
                user_id,
                session_key,
                remark,
                complete: tx,
            })
            .map_err(|_| StateError::new("Messaging verticle event channel closed"))?;
        Ok(rx.await
            .map_err(|_| StateError::new("Messaging verticle response channel closed"))?
            .map_err(StateError::new)?)
    }

    pub(crate) async fn get_contact(&self, id: Id) -> Result<Option<PhotonContact>> {
        let (tx, rx) = oneshot::channel();
        self.event_tx
            .send(VerticleEvent::GetContact { id, complete: tx })
            .map_err(|_| StateError::new("Messaging verticle event channel closed"))?;
        Ok(rx.await
            .map_err(|_| StateError::new("Messaging verticle response channel closed"))?
            .map_err(StateError::new)?)
    }

    pub(crate) async fn get_contacts(&self) -> Result<Vec<PhotonContact>> {
        let (tx, rx) = oneshot::channel();
        self.event_tx
            .send(VerticleEvent::GetContacts { complete: tx })
            .map_err(|_| StateError::new("Messaging verticle event channel closed"))?;
        Ok(rx.await
            .map_err(|_| StateError::new("Messaging verticle response channel closed"))?
            .map_err(StateError::new)?)
    }

    pub(crate) async fn update_contact(&self, contact: PhotonContact) -> Result<()> {
        let (tx, rx) = oneshot::channel();
        self.event_tx
            .send(VerticleEvent::UpdateContact {
                contact,
                complete: tx,
            })
            .map_err(|_| StateError::new("Messaging verticle event channel closed"))?;
        Ok(rx.await
            .map_err(|_| StateError::new("Messaging verticle response channel closed"))?
            .map_err(StateError::new)?)
    }

    pub(crate) async fn remove_contact(&self, id: Id) -> Result<()> {
        let (tx, rx) = oneshot::channel();
        self.event_tx
            .send(VerticleEvent::RemoveContact { id, complete: tx })
            .map_err(|_| StateError::new("Messaging verticle event channel closed"))?;
        Ok(rx.await
            .map_err(|_| StateError::new("Messaging verticle response channel closed"))?
            .map_err(StateError::new)?)
    }

    pub(crate) async fn remove_contacts(&self, ids: Vec<Id>) -> Result<()> {
        let (tx, rx) = oneshot::channel();
        self.event_tx
            .send(VerticleEvent::RemoveContacts { ids, complete: tx })
            .map_err(|_| StateError::new("Messaging verticle event channel closed"))?;
        Ok(rx.await
            .map_err(|_| StateError::new("Messaging verticle response channel closed"))?
            .map_err(StateError::new)?)
    }

    pub(crate) async fn clear_contacts(&self) -> Result<()> {
        let (tx, rx) = oneshot::channel();
        self.event_tx
            .send(VerticleEvent::ClearContacts { complete: tx })
            .map_err(|_| StateError::new("Messaging verticle event channel closed"))?;
        Ok(rx.await
            .map_err(|_| StateError::new("Messaging verticle response channel closed"))?
            .map_err(StateError::new)?)
    }

    #[allow(dead_code)]
    pub(crate) async fn friend_reject(&self, user_id: Id) -> Result<()> {
        let (tx, rx) = oneshot::channel();
        self.event_tx
            .send(VerticleEvent::FriendReject {
                user_id,
                complete: tx,
            })
            .map_err(|_| StateError::new("Messaging verticle event channel closed"))?;
        rx.await
            .map_err(|_| StateError::new("Messaging verticle response channel closed"))?
            .map_err(StateError::new)?;
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
            .map_err(|_| StateError::new("Messaging verticle event channel closed"))?;
        rx.await
            .map_err(|_| StateError::new("Messaging verticle response channel closed"))?
            .map_err(StateError::new)?;
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
            .map_err(|_| StateError::new("Messaging verticle event channel closed"))?;
        rx.await
            .map_err(|_| StateError::new("Messaging verticle response channel closed"))?
            .map_err(StateError::new)?;
        Ok(())
    }
}

pub(crate) struct VerticleOptions {
    pub(crate) options: Arc<Options>,

    pub(crate) connection_listener: Arc<dyn ConnectionListener>,
    pub(crate) message_listener: Arc<dyn MessageListener>,
    pub(crate) channel_listener: Arc<dyn ChannelListener>,
    pub(crate) contact_listener: Arc<dyn ContactListener>,
    pub(crate) session_listener: Arc<dyn SessionListener>,
    pub(crate) friend_request_listener: Arc<dyn crate::messaging::FriendRequestListener>,
}

impl VerticleOptions {
    pub(crate) fn user_id(&self) -> &Id {
        self.options.user_id()
    }

    pub(crate) fn device_id(&self) -> &Id {
        self.options.device_id()
    }

    pub(crate) fn into_options(self) -> Arc<Options> {
        self.options
    }
}

pub(crate) struct Verticle {
    session: Rc<SessionAgent>,
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
        let session = Session::new(options)?;
        Ok(Self {
            session,
            event_rx,
            running_flag,
            quit: false,
        })
    }

    /*
    fn handle_events1(
        &mut self,
        event: VerticleEvent,
        pending: &mut FuturesUnordered<Pin<Box<dyn Future<Output = ()>>>>,
    ) {
    }
    */

    async fn handle_event(&mut self, event: VerticleEvent) {
        match event {
            VerticleEvent::Start { complete } => {
                debug!("Verticle handling Start event");
                let session = self.session.clone();
                let running_flag = self.running_flag.clone();
                {
                    let result = MessagingClient::start(session.as_ref()).await;
                    if result.is_ok() {
                        running_flag.store(true, Ordering::Release);
                    }
                    let _ = complete.send(result.map_err(|e| e.to_string()));
                }
            }
            VerticleEvent::Stop { complete } => {
                debug!("Verticle handling Stop event");
                self.quit = true;
                let session = self.session.clone();
                let running_flag = self.running_flag.clone();
                {
                    let result = MessagingClient::stop(session.as_ref()).await;
                    running_flag.store(false, Ordering::Release);
                    let _ = complete.send(result.map_err(|e| e.to_string()));
                }
            }
            VerticleEvent::FriendRequest {
                user_id,
                hello,
                complete,
            } => {
                debug!("Verticle handling FriendRequest event for {user_id}");
                let result = self.session.friend_request(
                    &user_id,
                    hello,
                ).await;
                let _ = complete.send(result.map_err(|e| e.to_string()));
            }
            VerticleEvent::FriendAccept { user_id, complete } => {
                debug!("Verticle handling FriendAccept event for {user_id}");
                let result = self.session.accept_friend_request(
                    &user_id,
                ).await;
                let _ = complete.send(result.map_err(|e| e.to_string()));

            }
            VerticleEvent::GetFriendRequest { user_id, complete } => {
                let result = self.session.get_friend_request(&user_id).await;
                let _ = complete.send(result.map_err(|e| e.to_string()));
            }
            VerticleEvent::GetFriendRequests { complete } => {
                let result = self.session.get_friend_requests()
                    .await;
                let _ = complete.send(result.map_err(|e| e.to_string()));
            }
            VerticleEvent::RemoveFriendRequest { user_id, complete } => {
                let result = self.session.remove_friend_request(&user_id).await;
                let _ = complete.send(result.map_err(|e| e.to_string()));
            }
            VerticleEvent::RemoveFriendRequests { user_ids, complete } => {
                let result = self.session.remove_friend_requests(&user_ids).await;
                let _ = complete.send(result.map_err(|e| e.to_string()));
            }
            VerticleEvent::ClearFriendRequests { complete } => {
                let result = self.session.clear_friend_requests().await;
                let _ = complete.send(result.map_err(|e| e.to_string()));
            }
            VerticleEvent::ContentMessage {
                recipient,
                headers,
                body,
                text,
                complete,
            } => {
                debug!("Verticle handling ContentMessage event for {recipient}");
                let session = self.session.clone();
                {
                    let mut builder = MessagingClient::message(session.as_ref(), Some(recipient));
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
            }
            VerticleEvent::RegisterFriendSession {
                user_id,
                session_key,
                remark,
                complete,
            } => {
                let session = self.session.clone();
                {
                    let result = MessagingClient::add_friend(
                        session.as_ref(),
                        &user_id,
                        session_key,
                        remark,
                    ).await;
                    let _ = complete.send(result.map_err(|error| error.to_string()));
                }
            }
            VerticleEvent::GetContact { id, complete } => {
                let session = self.session.clone();
                {
                    let result = MessagingClient::get_contact(session.as_ref(), &id)
                        .await
                        .map(|contact| contact.map(|contact| contact_snapshot(contact.as_ref())));
                    let _ = complete.send(result.map_err(|error| error.to_string()));
                }
            }
            VerticleEvent::GetContacts { complete } => {
                let session = self.session.clone();
                {
                    let result = MessagingClient::get_contacts(session.as_ref())
                        .await
                        .map(|contacts| contacts.into_iter()
                            .map(|contact| contact_snapshot(contact.as_ref()))
                            .collect());
                    let _ = complete.send(result.map_err(|error| error.to_string()));
                }
            }
            VerticleEvent::UpdateContact { contact, complete } => {
                let session = self.session.clone();
                {
                    let result = MessagingClient::update_contact(session.as_ref(), &contact).await;
                    let _ = complete.send(result.map_err(|error| error.to_string()));
                }
            }
            VerticleEvent::RemoveContact { id, complete } => {
                let session = self.session.clone();
                {
                    let result = MessagingClient::remove_contact(session.as_ref(), &id).await;
                    let _ = complete.send(result.map_err(|error| error.to_string()));
                }
            }
            VerticleEvent::RemoveContacts { ids, complete } => {
                let session = self.session.clone();
                {
                    let result = MessagingClient::remove_contacts(session.as_ref(), &ids).await;
                    let _ = complete.send(result.map_err(|error| error.to_string()));
                }
            }
            VerticleEvent::ClearContacts { complete } => {
                let session = self.session.clone();
                {
                    let result = MessagingClient::clear_contacts(session.as_ref()).await;
                    let _ = complete.send(result.map_err(|error| error.to_string()));
                }
            }
            VerticleEvent::FriendReject { user_id, complete } => {
                debug!("Verticle handling FriendReject event for {user_id}");
                let session = self.session.clone();
                {
                    let result = session.friend_reject(user_id).await;
                    let _ = complete.send(result.map_err(|e| e.to_string()));
                }
            }
            VerticleEvent::FriendRemove { user_id, complete } => {
                debug!("Verticle handling FriendRemove event for {user_id}");
                let session = self.session.clone();
                {
                    let result = session.friend_remove(user_id).await;
                    let _ = complete.send(result.map_err(|e| e.to_string()));
                }
            }
            VerticleEvent::FriendInfo { user_id, complete } => {
                debug!("Verticle handling FriendInfo event for {user_id}");
                let session = self.session.clone();
                {
                    let result = session.friend_info(user_id).await;
                    let _ = complete.send(result.map_err(|e| e.to_string()));
                }
            }
        }
    }

    async fn run_loop(&mut self) {
        debug!("Verticle run loop entering event wait loop");
        while let Some(event) = self.event_rx.recv().await {
            self.handle_event(event).await;
            if self.quit {
                break;
            }
        }
        if !self.quit {
            if let Err(error) = MessagingClient::stop(self.session.as_ref()).await {
                error!("Failed to stop messaging session: {error}");
            }
        }
        self.running_flag.store(false, Ordering::Release);
        debug!("Verticle run loop finished");
    }
}

struct ClientConnectionListener {
    listener: Arc<dyn ConnectionListener>,
    connected: Arc<AtomicBool>,
    ready: Arc<AtomicBool>,
}

impl ConnectionListener for ClientConnectionListener {
    fn on_connecting(&self) {
        self.listener.on_connecting();
    }
    fn on_connected(&self) {
        self.connected.store(true, Ordering::Release);
        self.listener.on_connected();
    }
    fn on_ready(&self) {
        self.ready.store(true, Ordering::Release);
        self.listener.on_ready();
    }
    fn on_disconnected(&self) {
        self.connected.store(false, Ordering::Release);
        self.ready.store(false, Ordering::Release);
        self.listener.on_disconnected();
    }
}

pub(crate) fn deploy(
    options: Arc<Options>,
    connected: Arc<AtomicBool>,
    ready: Arc<AtomicBool>,
    connection_listener: Arc<dyn ConnectionListener>,
    message_listener: Arc<dyn MessageListener>,
    channel_listener: Arc<dyn ChannelListener>,
    contact_listener: Arc<dyn ContactListener>,
    session_listener: Arc<dyn SessionListener>,
    friend_request_listener: Arc<dyn crate::messaging::FriendRequestListener>,
) -> Result<VerticleClient> {
    debug!("Deploying messaging verticle...");
    let (event_tx, event_rx) = mpsc::unbounded_channel::<VerticleEvent>();
    let (reply_tx, reply_rx) = std_mpsc::sync_channel::<StdResult<(), String>>(1);
    let running_flag = Arc::new(AtomicBool::new(false));
    let running_clone = running_flag.clone();

    let verticle_options = VerticleOptions {
        options,
        connection_listener: Arc::new(ClientConnectionListener {
            listener: connection_listener,
            connected,
            ready,
        }),
        message_listener: message_listener.clone(),
        channel_listener,
        contact_listener,
        session_listener,
        friend_request_listener,
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
