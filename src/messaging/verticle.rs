use log::{info, debug, error};
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
    event_tx: mpsc::UnboundedSender<VerticleEvent>,
    handle: Arc<Mutex<Option<JoinHandle<()>>>>,
}

type CmdResult<T> = StdResult<T, String>;

pub(crate) enum VerticleEvent {
    Start {
        complete: oneshot::Sender<CmdResult<()>>,
    },
    Stop {
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
        event_tx: mpsc::UnboundedSender<VerticleEvent>,
        handle: JoinHandle<()>
    ) -> Self {
        Self {
            event_tx,
            handle: Arc::new(Mutex::new(Some(handle))),
        }
    }

    pub(crate) async fn start(&self) -> Result<()> {
        debug!("VerticleClient: sending Start event to verticle");
        let (tx, rx) = oneshot::channel();
        self.event_tx
            .send(VerticleEvent::Start { complete: tx })
            .map_err(|_| StateError::new(CHANNEL_REQ_CLOSED))?;
        rx.await
            .map_err(|_| StateError::new(CHANNEL_RSP_CLOSED))?
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
                    StateError::new(CHANNEL_RSP_CLOSED)
                })
                .and_then(|result| result.map_err(|error| -> crate::errors::Error {
                    StateError::new(error)
                }))
        } else {
            Err(StateError::new(CHANNEL_REQ_CLOSED).into())
        };

        let handle = self.handle.lock().unwrap().take();
        if let Some(handle) = handle {
            handle.join()
                .map_err(|_| StateError::new("Messaging verticle thread panicked"))?;
        }
        debug!("VerticleClient: verticle stopped");
        result
    }

    async fn rx_result<T>(&self, rx: oneshot::Receiver<CmdResult<T>>) -> Result<T> {
        match rx.await {
            Ok(Ok(v)) => Ok(v),
            Ok(Err(msg)) => Err(StateError::new(msg)),
            Err(_) => Err(StateError::new(CHANNEL_RSP_CLOSED)),
        }
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
            .map_err(|_| StateError::new(CHANNEL_REQ_CLOSED))?;
        self.rx_result(rx).await
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
            .map_err(|_| StateError::new(CHANNEL_REQ_CLOSED))?;
        self.rx_result(rx).await
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
            .map_err(|_| StateError::new(CHANNEL_REQ_CLOSED))?;
        self.rx_result(rx).await
    }

    pub(crate) async fn get_friend_requests(&self) -> Result<Vec<FriendRequest>> {
        let (tx, rx) = oneshot::channel();
        self.event_tx
            .send(VerticleEvent::GetFriendRequests { complete: tx })
            .map_err(|_| StateError::new(CHANNEL_REQ_CLOSED))?;
        self.rx_result(rx).await
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
            .map_err(|_| StateError::new(CHANNEL_REQ_CLOSED))?;
        self.rx_result(rx).await
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
            .map_err(|_| StateError::new(CHANNEL_REQ_CLOSED))?;
        self.rx_result(rx).await
    }

    pub(crate) async fn clear_friend_requests(&self) -> Result<()> {
        let (tx, rx) = oneshot::channel();
        self.event_tx
            .send(VerticleEvent::ClearFriendRequests { complete: tx })
            .map_err(|_| StateError::new(CHANNEL_REQ_CLOSED))?;
        self.rx_result(rx).await
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
            .map_err(|_| StateError::new(CHANNEL_REQ_CLOSED))?;
        self.rx_result(rx).await
    }

    pub(crate) async fn get_contact(&self, id: Id) -> Result<Option<PhotonContact>> {
        let (tx, rx) = oneshot::channel();
        self.event_tx
            .send(VerticleEvent::GetContact { id, complete: tx })
            .map_err(|_| StateError::new(CHANNEL_REQ_CLOSED))?;
        self.rx_result(rx).await
    }

    pub(crate) async fn get_contacts(&self) -> Result<Vec<PhotonContact>> {
        let (tx, rx) = oneshot::channel();
        self.event_tx
            .send(VerticleEvent::GetContacts { complete: tx })
            .map_err(|_| StateError::new(CHANNEL_REQ_CLOSED))?;
        self.rx_result(rx).await
    }

    pub(crate) async fn update_contact(&self, contact: PhotonContact) -> Result<()> {
        let (tx, rx) = oneshot::channel();
        self.event_tx
            .send(VerticleEvent::UpdateContact {
                contact,
                complete: tx,
            })
            .map_err(|_| StateError::new(CHANNEL_REQ_CLOSED))?;
        self.rx_result(rx).await
    }

    pub(crate) async fn remove_contact(&self, id: Id) -> Result<()> {
        let (tx, rx) = oneshot::channel();
        self.event_tx
            .send(VerticleEvent::RemoveContact { id, complete: tx })
            .map_err(|_| StateError::new(CHANNEL_REQ_CLOSED))?;
        self.rx_result(rx).await
    }

    pub(crate) async fn remove_contacts(&self, ids: Vec<Id>) -> Result<()> {
        let (tx, rx) = oneshot::channel();
        self.event_tx
            .send(VerticleEvent::RemoveContacts { ids, complete: tx })
            .map_err(|_| StateError::new(CHANNEL_REQ_CLOSED))?;
        self.rx_result(rx).await
    }

    pub(crate) async fn clear_contacts(&self) -> Result<()> {
        let (tx, rx) = oneshot::channel();
        self.event_tx
            .send(VerticleEvent::ClearContacts { complete: tx })
            .map_err(|_| StateError::new(CHANNEL_REQ_CLOSED))?;
        self.rx_result(rx).await
    }

    #[allow(dead_code)]
    pub(crate) async fn friend_reject(&self, user_id: Id) -> Result<()> {
        let (tx, rx) = oneshot::channel();
        self.event_tx
            .send(VerticleEvent::FriendReject {
                user_id,
                complete: tx,
            })
            .map_err(|_| StateError::new(CHANNEL_REQ_CLOSED))?;
        self.rx_result(rx).await
    }

    #[allow(dead_code)]
    pub(crate) async fn friend_remove(&self, user_id: Id) -> Result<()> {
        let (tx, rx) = oneshot::channel();
        self.event_tx
            .send(VerticleEvent::FriendRemove {
                user_id,
                complete: tx,
            })
            .map_err(|_| StateError::new(CHANNEL_REQ_CLOSED))?;
        self.rx_result(rx).await
    }

    #[allow(dead_code)]
    pub(crate) async fn friend_info(&self, user_id: Id) -> Result<()> {
        let (tx, rx) = oneshot::channel();
        self.event_tx
            .send(VerticleEvent::FriendInfo {
                user_id,
                complete: tx,
            })
            .map_err(|_| StateError::new(CHANNEL_REQ_CLOSED))?;
        self.rx_result(rx).await
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
    session: Rc<SessionAgent>,
    event_rx: mpsc::UnboundedReceiver<VerticleEvent>,
    quit: bool,
}

impl Verticle {
    fn new(
        options: VerticleOptions,
        event_rx: mpsc::UnboundedReceiver<VerticleEvent>,
    ) -> Result<Self> {
        let session = Session::new(options)?;
        Ok(Self {
            session,
            event_rx,
            quit: false,
        })
    }

    fn handle_events(
        &mut self,
        event: VerticleEvent,
        pending: &mut FuturesUnordered<Pin<Box<dyn Future<Output = ()>>>>,
    ) {
        match event {
            VerticleEvent::Start { complete } => {
                let session = self.session.clone();
                pending.push(
                    async move {
                        let result = session.start().await;
                        let _ = complete.send(result.map_err(|e| e.to_string()));
                    }
                    .boxed_local(),
                );
            }
            VerticleEvent::Stop { complete } => {
                self.quit = true;
                let session = self.session.clone();
                pending.push(
                    async move {
                        let result = session.stop().await;
                        let _ = complete.send(result.map_err(|e| e.to_string()));
                    }
                    .boxed_local(),
                );
            }
            VerticleEvent::FriendRequest {
                user_id,
                hello,
                complete,
            } => {
                let session = self.session.clone();
                pending.push(
                    async move {
                        let result = session.friend_request(&user_id, hello).await;
                        let _ = complete.send(result.map_err(|e| e.to_string()));
                    }
                    .boxed_local(),
                );
            }
            VerticleEvent::FriendAccept { user_id, complete } => {
                let session = self.session.clone();
                pending.push(
                    async move {
                        let result = session.accept_friend_request(&user_id).await;
                        let _ = complete.send(result.map_err(|e| e.to_string()));
                    }
                    .boxed_local(),
                );

            }
            VerticleEvent::GetFriendRequest { user_id, complete } => {
                let session = self.session.clone();
                pending.push(
                    async move {
                        let result = session.get_friend_request(&user_id).await;
                        let _ = complete.send(result.map_err(|e| e.to_string()));
                    }
                    .boxed_local(),
                );
            }
            VerticleEvent::GetFriendRequests { complete } => {
                let session = self.session.clone();
                pending.push(
                    async move {
                        let result = session.get_friend_requests().await;
                        let _ = complete.send(result.map_err(|e| e.to_string()));
                    }
                    .boxed_local(),
                );
            }
            VerticleEvent::RemoveFriendRequest { user_id, complete } => {
                let session = self.session.clone();
                pending.push(
                    async move {
                        let result = session.remove_friend_request(&user_id).await;
                        let _ = complete.send(result.map_err(|e| e.to_string()));
                    }
                    .boxed_local(),
                );
            }
            VerticleEvent::RemoveFriendRequests { user_ids, complete } => {
                let session = self.session.clone();
                pending.push(
                    async move {
                        let result = session.remove_friend_requests(&user_ids).await;
                        let _ = complete.send(result.map_err(|e| e.to_string()));
                    }
                    .boxed_local(),
                );
            }
            VerticleEvent::ClearFriendRequests { complete } => {
                let session = self.session.clone();
                pending.push(
                    async move {
                        let result = session.clear_friend_requests().await;
                        let _ = complete.send(result.map_err(|e| e.to_string()));
                    }
                    .boxed_local(),
                );
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
                pending.push(
                    async move {
                        let mut builder = session.message(Some(recipient));
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
            VerticleEvent::RegisterFriendSession {
                user_id,
                session_key,
                remark,
                complete,
            } => {
                let session = self.session.clone();
                pending.push(
                    async move {
                        let result = session.add_friend(&user_id, session_key, remark).await;
                        let _ = complete.send(result.map_err(|error| error.to_string()));
                    }
                    .boxed_local(),
                );
            }
            VerticleEvent::GetContact { id, complete } => {
                let session = self.session.clone();
                pending.push(
                    async move {
                        let result = session.get_contact(&id).await
                            .map(|contact| contact.map(|contact| contact_snapshot(contact.as_ref())));
                        let _ = complete.send(result.map_err(|error| error.to_string()));
                    }
                    .boxed_local(),
                );
            }
            VerticleEvent::GetContacts { complete } => {
                let session = self.session.clone();
                pending.push(
                    async move {
                        let result = session.get_contacts().await
                            .map(|contacts| contacts.into_iter()
                                .map(|contact| contact_snapshot(contact.as_ref()))
                                .collect());
                        let _ = complete.send(result.map_err(|error| error.to_string()));
                    }
                    .boxed_local(),
                );
            }
            VerticleEvent::UpdateContact { contact, complete } => {
                let session = self.session.clone();
                pending.push(
                    async move {
                        let result = session.update_contact(&contact).await;
                        let _ = complete.send(result.map_err(|error| error.to_string()));
                    }
                    .boxed_local(),
                );
            }
            VerticleEvent::RemoveContact { id, complete } => {
                let session = self.session.clone();
                pending.push(
                    async move {
                        let result = session.remove_contact(&id).await;
                        let _ = complete.send(result.map_err(|error| error.to_string()));
                    }
                    .boxed_local(),
                );
            }
            VerticleEvent::RemoveContacts { ids, complete } => {
                let session = self.session.clone();
                pending.push(
                    async move {
                        let result = session.remove_contacts(&ids).await;
                        let _ = complete.send(result.map_err(|error| error.to_string()));
                    }
                    .boxed_local(),
                );
            }
            VerticleEvent::ClearContacts { complete } => {
                let session = self.session.clone();
                pending.push(
                    async move {
                        let result = session.clear_contacts().await;
                        let _ = complete.send(result.map_err(|error| error.to_string()));
                    }
                    .boxed_local(),
                );
            }
            VerticleEvent::FriendReject { user_id, complete } => {
                debug!("Verticle handling FriendReject event for {user_id}");
                let session = self.session.clone();
                pending.push(
                    async move {
                        let result = session.friend_reject(user_id).await;
                        let _ = complete.send(result.map_err(|e| e.to_string()));
                    }
                    .boxed_local(),
                );
            }
            VerticleEvent::FriendRemove { user_id, complete } => {
                debug!("Verticle handling FriendRemove event for {user_id}");
                let session = self.session.clone();
                pending.push(
                    async move {
                        let result = session.friend_remove(user_id).await;
                        let _ = complete.send(result.map_err(|e| e.to_string()));
                    }
                    .boxed_local(),
                );
            }
            VerticleEvent::FriendInfo { user_id, complete } => {
                debug!("Verticle handling FriendInfo event for {user_id}");
                let session = self.session.clone();
                pending.push(
                    async move {
                        let result = session.friend_info(user_id).await;
                        let _ = complete.send(result.map_err(|e| e.to_string()));
                    }
                    .boxed_local(),
                );
            }
        }
    }

    async fn run_loop(&mut self) {
        debug!("Verticle run loop entering event wait loop");
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

        if let Err(error) = self.session.stop().await {
            error!("Failed to stop messaging session: {error}");
        }
        info!("Messaging verticle exited run_loop");
    }
}

pub(crate) struct ClientConnectionListener {
    listener: Arc<dyn ConnectionListener>,
    connected: Arc<AtomicBool>,
    ready: Arc<AtomicBool>,
}

impl ClientConnectionListener {
    pub(crate) fn new(
        listener: Arc<dyn ConnectionListener>,
        connected: Arc<AtomicBool>,
        ready: Arc<AtomicBool>,
    ) -> Self {
        Self {
            listener,
            connected,
            ready,
        }
    }
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

pub(crate) fn deploy(options: VerticleOptions) -> Result<VerticleClient> {
    let (event_tx, event_rx) = mpsc::unbounded_channel::<VerticleEvent>();
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

#[cfg(test)]
include!("unitests/test_verticle.rs");
