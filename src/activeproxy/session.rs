use std::{
    cell::RefCell,
    rc::{Rc, Weak},
    sync::Arc,
    time::{Duration, SystemTime},
    collections::HashMap,
    net::{SocketAddr, ToSocketAddrs}
};
use log::{debug, error, info, warn};
use futures::future::LocalBoxFuture;
use tokio::{
    net::{TcpSocket, TcpStream},
    task,
    time
};

use crate::{
    CryptoContext,
    Id,
    Identity,
    PeerBuilder,
    PeerInfo,
    Result,
    errors::{ArgumentError, NetworkError, StateError},
    cryptobox,
    dht::Node,
    elapsed_ms,
    identity::CryptoIdentity,
    signature
};
use super::{
    LocalBoxHandler,
    LocalBoxTimerClient as TimerClient,
    connection::ProxyConnection,
    connection_handler::ConnectionHandler,
    connection_registry::ConnectionRegistry,
    verticle::VerticleOptions,
};

const PERIODIC_CHECK_INTERVAL: u64  = 15 * 1000;        // 15 seconds
const IDLE_CHECK_INTERVAL:     u128 = 60 * 1000;         // 1 minute
const STOP_DELAY:              u64  = 5 * 1000;          // 5 seconds
const RE_ANNOUNCE_INTERVAL:    u128 = 60 * 60 * 1000;    // 60 minutes
const MAX_IDLE_TIME:           u128 = 5 * 60 * 1000;     // 5 minutes

pub trait ConnectionStatusListener {
    fn connected(&self) {}
    fn disconnected(&self) {}
}

struct SessionConfig {
    user_id                     : Id,
    peerid                      : Id,
    announce_peer_enabled       : bool,
    name_access_enabled         : bool,
}

pub(crate) struct ProxySession {
    cfg                         : SessionConfig,
    node                        : Option<Arc<Node>>,

   // service_peerinfo            : Option<PeerInfo>,
    service_addr                : SocketAddr,

    upstream_addr                : SocketAddr,

    device_identity             : CryptoIdentity,

    client_session_keypair      : RefCell<Option<cryptobox::KeyPair>>,
    peer_context                : Rc<RefCell<CryptoContext>>,
    session_context             : Rc<RefCell<Option<CryptoContext>>>,

    connected                   : RefCell<bool>,
    next_connection_id          : RefCell<i32>,
    pending_connects            : RefCell<i32>,
    max_connections             : RefCell<i32>,
    name_access                 : RefCell<bool>,
    connect_failures            : RefCell<i32>,

    peer_info                   : RefCell<Option<PeerInfo>>,

    connection_map              : RefCell<HashMap<i32, Rc<ProxyConnection>>>,
    connections                 : RefCell<ConnectionRegistry<ProxyConnection>>,

    running                     : RefCell<bool>,
    dangling_timestamp          : RefCell<SystemTime>,
    idle_timestamp              : RefCell<SystemTime>,
    last_announce_timestamp     : RefCell<SystemTime>,
    last_idle_check_timestamp   : RefCell<SystemTime>,

    connection_status_listener  : RefCell<Option<Rc<dyn ConnectionStatusListener>>>,

    timer_client      : TimerClient,
}

impl ProxySession {
    pub(crate) fn new(
        options: VerticleOptions,
        timer_client: TimerClient
    ) -> Result<Rc<Self>> {
        let rest = options.service_endpoint.strip_prefix("tcp://")
            .unwrap_or(&options.service_endpoint);

        let service_addr = rest.to_socket_addrs()
            .map_err(|e| {
                error!("Failed to resolve address '{rest}', network error: {e}");
                ArgumentError::new(format!("Bad remote service address: {rest}"))
            })?
            .next()
            .ok_or_else(|| {
                error!("No valid address found for '{rest}', network error!!!");
                NetworkError::new(format!("No valid address found for '{rest}'!"))
            })?;

        let device_kp = signature::KeyPair::from(options.device_key);
        let device_identity = CryptoIdentity::from(device_kp);
        let peer_context = device_identity.create_crypto_context(
            &options.service_peerid
        )?;

        let cfg = SessionConfig {
            peerid                      : options.service_peerid,
            announce_peer_enabled       : options.announce_peer_enabled,
            name_access_enabled         : options.name_access_enabled,
            user_id                     : options.user_id,
        };

        Ok(Rc::new(Self {
            cfg,
            node                        : options.node.clone(),
          //  service_peerinfo            : options.service_peer.clone(),
            service_addr,

            upstream_addr               : options.upstream_addr,
            device_identity,

            peer_info                   : RefCell::new(None),

            client_session_keypair      : RefCell::new(None),
            peer_context                : Rc::new(RefCell::new(peer_context)),
            session_context             : Rc::new(RefCell::new(None)),
            connected                   : RefCell::new(false),
            next_connection_id          : RefCell::new(0),
            max_connections             : RefCell::new(1),
            name_access                 : RefCell::new(false),
            connect_failures            : RefCell::new(0),
            pending_connects            : RefCell::new(0),
            connection_map              : RefCell::new(HashMap::new()),
            connections                 : RefCell::new(ConnectionRegistry::new()),
            running                     : RefCell::new(false),
            timer_client,

            dangling_timestamp          : RefCell::new(SystemTime::UNIX_EPOCH),
            idle_timestamp              : RefCell::new(SystemTime::UNIX_EPOCH),
            last_announce_timestamp     : RefCell::new(SystemTime::UNIX_EPOCH),
            last_idle_check_timestamp   : RefCell::new(SystemTime::UNIX_EPOCH),
            connection_status_listener  : RefCell::new(None),

        }))
    }

    fn user_id(&self) -> &Id {
        &self.cfg.user_id
    }
    fn peer_id(&self) -> &Id {
        &self.cfg.peerid
    }

    pub(crate) fn _set_connection_listener(&self, listener: Option<Rc<dyn ConnectionStatusListener>>) {
        *self.connection_status_listener.borrow_mut() = listener;
    }

    pub(crate) async fn start(self: &Rc<Self>) -> Result<()> {
        *self.last_idle_check_timestamp.borrow_mut() = SystemTime::now();

        *self.running.borrow_mut() = true;
        if let Err(e) = self.setup_periodic_tasks().await {
            *self.running.borrow_mut() = false;
            return Err(e);
        }

        match self.connect().await {
            Ok(()) => {
                debug!("Proxy session {} started", self.peer_id());
                Ok(())
            },
            Err(e) => {
                *self.running.borrow_mut() = false;
                self.clear_periodic_tasks().await;
                warn!("Proxy session {} failed to make connection: {e}, try again later", self.peer_id());

                let failures = *self.connect_failures.borrow();
                let delay = (failures.min(12) * 5 * 1000) as u64;
                let session = self.clone();
                let _ = self.timer_client.add_timer(
                    delay, None,
                    LocalBoxHandler::new(move |_| {
                        let session = session.clone();
                        Box::pin(async move {
                            if session.needs_new_connection() {
                                let _ = session.connect().await;

                            };
                        })
                    })
                )?;
                Err(e)
            }
        }
    }

    async fn setup_periodic_tasks(self: &Rc<Self>) -> Result<()> {
        let Some(ref node) = self.node else {
            return Ok(());
        };

        let node = node.clone();
        let peerid = self.peer_id().clone();
        let _ = self.timer_client.add_timer(30*1000, Some(30*1000),
            LocalBoxHandler::new(move |_| {
                let node = node.clone();
                Box::pin(async move {
                    let _ = super::utils::lookup_peer(node, &peerid).await;
                })
            })
        )?;

        let session = self.clone();
        let _ = self.timer_client.add_timer(
            PERIODIC_CHECK_INTERVAL,
            Some(PERIODIC_CHECK_INTERVAL),
            LocalBoxHandler::new(move |_| {
                let session = session.clone();
                Box::pin(async move {
                    session.periodic_check();
                })
            })
        )?;
        Ok(())
    }

    async fn clear_periodic_tasks(self: &Rc<Self>) {
        let _ = self.timer_client.stop_timers().await;
    }

    pub(crate) async fn stop(self: &Rc<Self>) {
        if !*self.running.borrow() {
            return;
        }

        debug!("Proxy session {} is stopping ...", self.peer_id());
        *self.running.borrow_mut() = false;

        let conn_ids = self.connections.borrow().connections();
        for id in conn_ids {
            let conn = {
                self.connection_map.borrow().get(&id).cloned()
            };
            if let Some(conn) = conn {
                let _ = conn.close().await;
            }
        }
        self.connection_map.borrow_mut().clear();
        self.connections.borrow_mut().clear();

        if *self.connected.borrow() {
            *self.connected.borrow_mut() = false;
            if let Some(listener) = self.connection_status_listener.borrow().as_ref() {
                listener.disconnected();
            }
        }

        info!("Proxy session {} stopped", self.peer_id());
    }

    fn periodic_check(self: Rc<Self>) {
        self.try_close_idle_connections();
        self.health_check();
        self.try_announce_peer();
    }

    fn try_close_idle_connections(self: &Rc<Self>) {
        let now = SystemTime::now();
        if elapsed_ms!(*self.last_idle_check_timestamp.borrow()) < IDLE_CHECK_INTERVAL {
            return;
        }
        *self.last_idle_check_timestamp.borrow_mut() = now;

        let (size, in_flight) = {
            let registry = self.connections.borrow();
            (registry.size(), registry.in_flight())
        };
        info!("STATUS: session={}, connections={size}, inFlight={in_flight}", self.peer_id());

        let idle_timestamp = *self.idle_timestamp.borrow();
        if in_flight != 0 || idle_timestamp == SystemTime::UNIX_EPOCH || size <= 1
            || elapsed_ms!(idle_timestamp) < MAX_IDLE_TIME {
            return;
        }

        info!("Session {} closing the idle connections...", self.peer_id());
        // All connections are idle here (inFlight == 0); keep one and close the rest.
        let idle_ids = self.connections.borrow().connections();
        for id in idle_ids.into_iter().skip(1) {
            self.connections.borrow_mut().remove(id);
            let conn = self.connection_map.borrow_mut().remove(&id);
            if let Some(conn) = conn {
                task::spawn_local(async move {
                    let _ = conn.close().await;
                });
            }
        }
    }

    fn health_check(self: &Rc<Self>) {
        let conn_ids = self.connections.borrow().connections();
        for id in conn_ids {
            let conn = {
                self.connection_map.borrow().get(&id).cloned()
            };
            if let Some(conn) = conn {
                task::spawn_local(async move {
                    let _ = conn.check_keepalive().await;
                });
            }
        }
    }

    fn try_announce_peer(self: &Rc<Self>) {
        let Some(node) = self.node.clone() else {
            return
        };
        let Some(peer) = self.peer_info.borrow().clone() else {
            return
        };
        if elapsed_ms!(*self.last_announce_timestamp.borrow()) < RE_ANNOUNCE_INTERVAL {
            return;
        }

        info!("Session {} announcing peer info {peer} ...", self.peer_id());
        *self.last_announce_timestamp.borrow_mut() = SystemTime::now();

        let session = self.clone();
        task::spawn_local(async move {
            match node.announce_peer(&peer, -1, false).await {
                Ok(_) => info!("Session {} peer info announced", session.peer_id()),
                Err(e) => {
                    error!("Session {} failed to announce peer info: {e}", session.peer_id());
                    // retry after 1 minute
                    *session.last_announce_timestamp.borrow_mut() =
                        SystemTime::now() - Duration::from_millis((RE_ANNOUNCE_INTERVAL - 60_000) as u64);
                }
            }
        });
    }

    fn reset(&self) {
        *self.connected.borrow_mut() = false;
        *self.dangling_timestamp.borrow_mut() = SystemTime::UNIX_EPOCH;
    }

    fn needs_new_connection(&self) -> bool {
        if !*self.running.borrow() {
            return false;
        }

        let registry = self.connections.borrow();
        let pending = *self.pending_connects.borrow() as usize;

        // Count in-flight dials against the ceiling so concurrent busy/close handlers cannot
        // over-provision past max_connections while a CONNECT is still pending.
        if registry.size() + pending >= *self.max_connections.borrow() as usize {
            return false;
        }

        // A new connection is needed only when there is no idle (or pending) spare ready to serve
        // the next CONNECT.
        let spares = (registry.size() - registry.in_flight()) + pending;
        spares == 0
    }

    async fn connect(self: &Rc<Self>) -> Result<()> {
        *self.pending_connects.borrow_mut() += 1;

        let addr = self.service_addr;
        debug!("Creating new proxy connection to service {}@{addr} ...", self.peer_id());

        let socket = TcpSocket::new_v4()?;
        let result = socket.connect(addr).await;
        *self.pending_connects.borrow_mut() -= 1;

        let stream = match result {
            Ok(stream) => stream,
            Err(e) => {
                *self.connect_failures.borrow_mut() += 1;
                let failures = *self.connect_failures.borrow();
                warn!("Create new proxy connection to service {}@{addr} failed({failures}): {e}", self.peer_id());
                return Err(e.into());
            }
        };

        let cid = *self.next_connection_id.borrow();
        *self.next_connection_id.borrow_mut() = cid + 1;
        info!("Created new proxy connection {cid} to service {}@{addr}", self.peer_id());

        let handler: Rc<dyn ConnectionHandler> = Rc::new(SessionConnectionHandler {
            session: Rc::downgrade(self),
        });
        let connection = ProxyConnection::new(
            cid,
            stream,
            self.peer_context.clone(),
            self.session_context.clone(),
            handler,
        );

        info!("Connection {} is created.", connection.cid());

        self.connection_map.borrow_mut().insert(cid, connection.clone());
        self.connections.borrow_mut().add(cid);

        task::spawn_local(connection.run());

        Ok(())
    }

    pub(crate) async fn connect_upstream(&self) -> Result<TcpStream> {
        debug!("Connecting to the upstream {} ...", self.upstream_addr);
        let socket = TcpSocket::new_v4()?;
        socket.connect(self.upstream_addr).await.map_err(Into::into)
    }

    pub(crate) fn on_challenge(
        self: &Rc<Self>,
        connection: &Rc<ProxyConnection>,
        challenge: &[u8]
    ) -> Option<Rc<RefCell<CryptoContext>>> {
        let device_sig = match self.device_identity.sign_into(challenge) {
            Ok(sig) => sig,
            Err(e) => {
                error!("Session failed to sign the challenge: {e}");
                return None;
            }
        };

        let device_id = self.device_identity.id().clone();
        let connection = connection.clone();
        let connected  = *self.connected.borrow();
        if !connected {
            let kp = cryptobox::KeyPair::random();
            let pk = kp.to_public_key();
            *self.client_session_keypair.borrow_mut() = Some(kp);

            let user_id = self.user_id().clone();
            let name_access = self.cfg.name_access_enabled;

            task::spawn_local(async move {
                let _ = connection.send_auth(
                    user_id, device_id, pk, name_access, device_sig
                ).await;
            });
        } else {
            let borrowed_kp = self.client_session_keypair.borrow();
            let Some(kp) = borrowed_kp.as_ref() else {
                error!("Session INTERNAL ERROR: inconsistent state - client session keypair not initialized");
                return None;
            };

            let session_pk = kp.to_public_key();
            task::spawn_local(async move {
                let _ = connection.send_attach(
                    device_id, session_pk, device_sig
                ).await;
            });
        }
        None
    }

    pub(crate) fn on_authenticated(
        self: &Rc<Self>,
        server_session_pk: &cryptobox::PublicKey,
        max_connections: i32,
        name_access: bool,
        endpoint: &str,
        named_endpoint: Option<&str>,
    ) -> Result<()> {
        let borrowed_kp = self.client_session_keypair.borrow();
        let Some(kp) = borrowed_kp.as_ref() else {
            error!("Session INTERNAL ERROR: inconsistent state - client session keypair not initialized");
            return Err(ArgumentError::new("Session not initialized"));
        };

        *self.max_connections.borrow_mut() = max_connections;
        *self.name_access.borrow_mut() = name_access;
        //*self.endpoint.borrow_mut() = endpoint.to_string();
        //*self.named_endpoint.borrow_mut() = named_endpoint.map(|s| s.to_string());

        let crypto_box = cryptobox::CryptoBox::try_from(
            (server_session_pk, kp.private_key())
        )?;
        *self.session_context.borrow_mut() = Some(CryptoContext::new(
            self.peer_id().clone(),
            crypto_box
        ));
        *self.connected.borrow_mut() = true;

        info!("Proxy session {} authenticated, max connections: {max_connections}, endpoint: {}, named endpoint: {}",
            self.peer_id(),
            endpoint,
            named_endpoint.as_deref().unwrap_or("N/A")
        );

        if self.cfg.announce_peer_enabled {
            let advertised = named_endpoint.as_deref().unwrap_or(endpoint);
            let mut builder = PeerBuilder::new(advertised)
                .with_key(self.device_identity.signature_keypair().clone());
            if named_endpoint.is_some() {
                builder = builder.with_extra(format!("altEndpoint={endpoint}").as_bytes());
            }

            if let Ok(peer) = builder.build() {
                *self.peer_info.borrow_mut() = Some(peer);
                self.try_announce_peer();
            }
        }

        if let Some(listener) = self.connection_status_listener.borrow().as_ref() {
            listener.connected();
        }
        Ok(())
    }

    pub(crate) fn on_connection_opened(
        &self,
        _connection: &Rc<ProxyConnection>
    ) {
        *self.connect_failures.borrow_mut() = 0;
        *self.dangling_timestamp.borrow_mut() = SystemTime::UNIX_EPOCH;
    }

    pub(crate) fn on_connection_closed(
        self: &Rc<Self>,
        connection: &Rc<ProxyConnection>) {
        let id = connection.cid();
        self.connections.borrow_mut().remove(id);
        self.connection_map.borrow_mut().remove(&id);

        if self.connections.borrow().is_empty() {
            warn!("Proxy session {} is dangling ...", self.peer_id());
            *self.dangling_timestamp.borrow_mut() = SystemTime::now();

            let session = self.clone();
            task::spawn_local(async move {
                time::sleep(Duration::from_millis(STOP_DELAY)).await;
                let dangling = *session.dangling_timestamp.borrow();
                if dangling != SystemTime::UNIX_EPOCH && elapsed_ms!(dangling) >= STOP_DELAY as u128 {
                    info!("Proxy session {} disconnected, reset session to reconnect", session.peer_id());
                    session.reset();
                    if let Some(listener) = session.connection_status_listener.borrow().as_ref() {
                        listener.disconnected();
                    }
                }
            });
        }

        if self.needs_new_connection() {
            let session = self.clone();
            task::spawn_local(async move {
                let _ = session.connect().await;
            });
        }
    }

    pub(crate) fn on_connection_idle(
        &self,
        connection: &Rc<ProxyConnection>
    ) {
        let id = connection.cid();
        let mut registry = self.connections.borrow_mut();
        if registry.mark_idle(id) && registry.in_flight() == 0 {
            *self.idle_timestamp.borrow_mut() = SystemTime::now();
        }
    }

    pub(crate) fn on_connection_busy(
        self: &Rc<Self>,
        connection: &Rc<ProxyConnection>
    ) {
        let id = connection.cid();
        self.connections.borrow_mut().mark_busy(id);
        *self.idle_timestamp.borrow_mut() = SystemTime::UNIX_EPOCH;

        if self.needs_new_connection() {
            let session = self.clone();
            task::spawn_local(async move {
                let _ = session.connect().await;
            });
        }
    }

    pub(crate) fn allow(&self, _client_addr: SocketAddr) -> bool {
        true
    }

    // Closes the session: releases the crypto material. Must be called only after `stop()`.
    pub(crate) fn close(&self) {
        assert!(!*self.running.borrow(), "Proxy session is still running");

        *self.connection_status_listener.borrow_mut() = None;
        *self.session_context.borrow_mut() = None;
        *self.client_session_keypair.borrow_mut() = None;

        debug!("Proxy session {} closed", self.peer_id());
    }
}

struct SessionConnectionHandler {
    session: Weak<ProxySession>,
}

impl ConnectionHandler for SessionConnectionHandler {
    fn challenge(
        &self,
        connection: &Rc<ProxyConnection>,
        challenge: &[u8],
    ) {
        if let Some(session) = self.session.upgrade() {
            session.on_challenge(connection, challenge);
        }
    }

    fn authenticated(
        &self,
        _connection: &Rc<ProxyConnection>,
        server_session_pk: &cryptobox::PublicKey,
        max_connections: i32,
        name_access: bool,
        endpoint: &str,
        named_endpoint: Option<&str>,
    ) -> Option<CryptoContext> {
        if let Some(session) = self.session.upgrade() {
            let _ = session.on_authenticated(
                server_session_pk,
                max_connections,
                name_access,
                endpoint,
                named_endpoint,
            );
        }
        None
    }

    fn open(&self, connection: &Rc<ProxyConnection>) {
        if let Some(session) = self.session.upgrade() {
            session.on_connection_opened(connection);
        }
    }

    fn close(&self, connection: &Rc<ProxyConnection>){
        if let Some(session) = self.session.upgrade() {
            session.on_connection_closed(connection);
        }
    }

    fn idle(&self, connection: &Rc<ProxyConnection>) {
        if let Some(session) = self.session.upgrade() {
            session.on_connection_idle(connection);
        }
    }

    fn busy(&self, connection: &Rc<ProxyConnection>) {
        if let Some(session) = self.session.upgrade() {
            session.on_connection_busy(connection);
        }
    }

    fn allow(&self, client_addr: SocketAddr) -> bool {
        self.session.upgrade()
            .map(|session| session.allow(client_addr))
            .unwrap_or(false)
    }

    fn connect_upstream(&self) -> LocalBoxFuture<'_, Result<TcpStream>> {
        let session = self.session.upgrade();
        Box::pin(async move {
            let session = session.ok_or_else(|| StateError::new("proxy session is gone"))?;
            session.connect_upstream().await
        })
    }
}
