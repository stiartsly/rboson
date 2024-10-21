use std::rc::Rc;
use std::cell::RefCell;
use std::sync::{Arc, Mutex};
use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::fs::File;
use std::path::PathBuf;
use std::io::Write;
use std::ops::Deref;
use std::net::SocketAddr;
use std::time::SystemTime;

use futures::stream::{FuturesUnordered, StreamExt};
use tokio::io::AsyncReadExt;
use tokio::time::Duration;
use ciborium::value::Value as CVal;
use log::{info, debug, error};

use crate::{
    unwrap,
    Id,
    Node,
    PeerInfo,
    PeerBuilder,
    cryptobox, CryptoBox,
    signature,
    error::Result,
    core::cbor,
    Error,
};

use crate::activeproxy::{
    connection::ProxyConnection
};

const IDLE_CHECK_INTERVAL:      u128 = 60 * 1000;           // 60s
const MAX_IDLE_TIME:            u128 = 5 * 60 * 1000;       // 5 minutes;
const HEALTH_CHECK_INTERVAL:    u128 = 10 * 1000;           // 10s
const RE_ANNOUNCE_INTERVAL:     u128 = 60 * 60 * 1000;      // 1hour
const PERSISTENCE_INTERVAL:     u128 = 60 * 60 * 1000;      // 1hour

pub(crate)
const MAX_DATA_PACKET_SIZE:     usize = 0x7FFF;

pub struct ProxyWorker {
    node:               Arc<Mutex<Node>>,
    cached_dir:         PathBuf,

    session_keypair:    Option<Rc<cryptobox::KeyPair>>,
    server_pk:          Option<cryptobox::PublicKey>,
    crypto_box:         Option<Rc<RefCell<Option<CryptoBox>>>>,

    peer_nodeid:        Option<Rc<Id>>,
    peer_id:            Option<Id>,

    remote_addr:        Option<Rc<SocketAddr>>,
    remote_name:        Option<Rc<String>>,
    remote_peer:        Option<PeerInfo>,       // remote active proxy peer service.

    upstream_addr:      Option<Rc<SocketAddr>>,
    upstream_name:      Option<Rc<String>>,

    peer_keypair:       Option<signature::KeyPair>,
    peer_domain:        Option<Rc<String>>,
    peer:               Option<PeerInfo>,
    relay_port:         Option<u16>,

   // replaystream_failures: i32,
   // upstream_failures:  i32,

    rcvbuf:             Option<Rc<RefCell<Box<Vec<u8>>>>>,

    max_connections:    usize,
    inflights:          usize,
    connections:        HashMap<i32, Rc<RefCell<ProxyConnection>>>,


    last_announcepeer_timestamp:    SystemTime,

    last_idle_check_timestamp:      SystemTime,
    last_health_check_timestamp:    SystemTime,
    last_save_peer_timestamp:       SystemTime,

    server_failures:    i32,
    reconnect_delay:    u128,
    last_reconnect_timestamp:       SystemTime,

    cloned: Option<Rc<RefCell<ProxyWorker>>>,
}

impl ProxyWorker {
    pub fn new(node: Arc<Mutex<Node>>, cached_dir: PathBuf) -> Self {
        Self {
            node,
            cached_dir,

            session_keypair:    Some(Rc::new(cryptobox::KeyPair::random())),
            server_pk:          None,
            crypto_box:         Some(Rc::new(RefCell::new(None))),

            // The server node that provides active proxy service
            peer_nodeid:        None,
            peer_id:            None,

            remote_peer:        None,
            remote_addr:        None,
            remote_name:        None,


            upstream_name:      None,
            upstream_addr:      None,

            peer_domain:        None,
            peer_keypair:       None,
            peer:               None,         // upstream service as a public peer service.
            relay_port:         None,
            last_announcepeer_timestamp:SystemTime::UNIX_EPOCH,

           // replaystream_failures: 0,
           // upstream_failures:  0,

            rcvbuf:             Some(Rc::new(RefCell::new(Box::new(vec![0u8; MAX_DATA_PACKET_SIZE])))),

            max_connections:    16,
            inflights:          0,
            connections:        HashMap::new(),

            last_idle_check_timestamp:  SystemTime::UNIX_EPOCH,
            last_health_check_timestamp:SystemTime::UNIX_EPOCH,
            last_save_peer_timestamp:   SystemTime::UNIX_EPOCH,

            server_failures:    0,
            reconnect_delay:    0,
            last_reconnect_timestamp:   SystemTime::UNIX_EPOCH,

            cloned:             None,
        }
    }

    pub(crate) fn set_field<T: 'static>(&mut self, field: T, seq: Option<i32>) -> &mut Self {
        let typid = TypeId::of::<T>();
        let field = Box::new(field) as Box<dyn Any>;

        if typid == TypeId::of::<SocketAddr>() {
            let rc = field.downcast::<SocketAddr>().unwrap();
            match seq {
                Some(0) => {
                    self.remote_addr = Some(Rc::new(rc.deref().clone()));
                    self.remote_name = Some(Rc::new(unwrap!(self.remote_addr).to_string()));
                },
                Some(1) => {
                    self.upstream_addr = Some(Rc::new(rc.deref().clone()));
                    self.upstream_name = Some(Rc::new(unwrap!(self.upstream_addr).to_string()));
                },
                _ => {}
            }
        }
        else if typid == TypeId::of::<Rc<RefCell<ProxyWorker>>>() {
            let rc = field.downcast::<Rc<RefCell<ProxyWorker>>>().unwrap();
            self.cloned = Some(rc.deref().clone());
        } else if typid == TypeId::of::<PeerInfo>() {
            let rc = field.downcast::<PeerInfo>().unwrap();
            self.remote_peer = Some(rc.deref().clone());
            self.peer_nodeid = Some(Rc::new(unwrap!(self.remote_peer).nodeid().clone()));
            self.peer_id = Some(unwrap!(self.remote_peer).id().clone());
        } else if typid == TypeId::of::<Option<String>>() {
            let rc = field.downcast::<Option<String>>().unwrap();
            self.peer_domain = rc.deref().clone().map(|v| Rc::new(v));
        } else if typid == TypeId::of::<Option<signature::KeyPair>>() {
            let rc = field.downcast::<Option<signature::KeyPair>>().unwrap();
            self.peer_keypair = rc.deref().clone();
        }
        self
    }

    fn cloned(&self) -> Rc<RefCell<Self>> {
        unwrap!(self.cloned).clone()
    }

    pub(crate) fn set_max_connections(&mut self, connections: usize) {
        self.max_connections = connections;
    }

    fn reset(&self) {
        unimplemented!()
    }

    fn persist_peer(&self) {
        let val = CVal::Map(vec![
            (
                CVal::Text(String::from("peerId")),
                CVal::Bytes(unwrap!(self.remote_peer).id().as_bytes().into())
            ),
            (
                CVal::Text(String::from("serverHost")),
                CVal::Text(unwrap!(self.remote_addr).ip().to_string())
            ),
            (
                CVal::Text(String::from("serverPort")),
                CVal::Integer(unwrap!(self.remote_addr).port().into())
            ),
            (
                CVal::Text(String::from("serverId")),
                CVal::Bytes(unwrap!(self.remote_peer).nodeid().as_bytes().into())
            ),
            (
                CVal::Text(String::from("signature")),
                CVal::Bytes(unwrap!(self.remote_peer).signature().into())
            )
        ]);

        let mut buf = vec![];
        let writer = cbor::Writer::new(&mut buf);
        let _ = ciborium::ser::into_writer(&val, writer);

        if let Ok(mut fp) = File::create(&self.cached_dir) {
            _ = fp.write_all(&buf);
            _ = fp.sync_data();
        }
    }

    async fn lookup_peer(&self) {
        // TODO:
    }

    async fn announce_peer(&self) {
        let Some(peer) = self.peer.as_ref() else {
            return;
        };

        info!("Announce peer {} : {}", peer.id(), peer);

        if let Some(url) = peer.alternative_url() {
            info!("-**- ProxyWorker: peer server: {}:{}, domain: {} -**-", unwrap!(self.remote_addr).ip(), peer.port(), url);
        } else {
            info!("-**- ProxyWorker: peer server: {}:{} -**-", unwrap!(self.remote_addr).ip(), peer.port());
        }

        _ = self.node.lock()
            .unwrap()
            .announce_peer(peer, None).await;
    }

    async fn idle_check(&mut self) {
        // Dump the current status: should change the log level to debug later
        debug!("ProxyWorker STATUS dump: Connections = {}, inFlights = {}, idle = {}",
            self.connections.len(), self.inflights,
            unwrap!(self.last_idle_check_timestamp.elapsed()).as_secs());

        for (_, item) in self.connections.iter() {
            debug!("ProxyWorker status dump: \n{}", item.borrow());
        }

        if unwrap!(self.last_idle_check_timestamp.elapsed()).as_millis() < MAX_IDLE_TIME  ||
            self.inflights > 0 || self.connections.len() <= 1 {
            return;
        }

        info!("ProxyWorker is recycling redundant connections due to long time idle...");

        let keys: Vec<_> = self.connections.keys().cloned().collect();
        for key in keys {
            let item = self.connections.remove(&key).unwrap();
            item.borrow_mut().on_closed().await;
            item.borrow_mut().close().await.ok();
        }
    }

    async fn health_check(&mut self) {
        for (_, item) in self.connections.iter() {
            item.borrow_mut().periodic_check();
        }
    }

    pub(crate) async fn on_iteration(&mut self) {
        if self.needs_new_connection() {
            _ = self.try_connect().await;
        }

        if unwrap!(self.last_idle_check_timestamp.elapsed()).as_millis() >= IDLE_CHECK_INTERVAL {
            self.last_idle_check_timestamp = SystemTime::now();
            self.idle_check().await;
        }

        if unwrap!(self.last_health_check_timestamp.elapsed()).as_millis() >= HEALTH_CHECK_INTERVAL {
            self.last_health_check_timestamp = SystemTime::now();
            self.health_check().await;
        }

        if self.peer.is_some() &&
            unwrap!(self.last_announcepeer_timestamp.elapsed()).as_millis() >= RE_ANNOUNCE_INTERVAL {
            self.last_announcepeer_timestamp = SystemTime::now();
            self.announce_peer().await;
        }

        if unwrap!(self.last_save_peer_timestamp.elapsed()).as_millis() >= PERSISTENCE_INTERVAL {
            self.last_save_peer_timestamp = SystemTime::now();
            self.lookup_peer().await;
            self.persist_peer();
        }
    }

    async fn try_connect(&mut self) -> Result<()> {
        debug!("ProxyWorker started to create a new connectoin ...");

        let mut connection = ProxyConnection::new(
            self.cloned.as_ref().unwrap().clone(),
            self.node.clone()
        );
        connection
            .set_field(self.peer_domain.as_ref().map(|v|v.clone()), None)
            .set_field(unwrap!(self.remote_addr).clone(), Some(0))
            .set_field(unwrap!(self.upstream_addr).clone(), Some(1))
            .set_field(unwrap!(self.remote_name).clone(), Some(0))
            .set_field(unwrap!(self.upstream_name).clone(), Some(1))
            .set_field(unwrap!(self.peer_nodeid).clone(), None)
            .set_field(unwrap!(self.session_keypair).clone(), None)
            .set_field(unwrap!(self.crypto_box).clone(), None)
            .set_field(unwrap!(self.rcvbuf).clone(), None);

        let cloned = self.cloned();
        connection.with_on_authorized_cb(Box::new(move |_: &ProxyConnection, server_pk: &cryptobox::PublicKey, port: u16, domain_enabled: bool| {
            cloned.borrow_mut().server_pk = Some(server_pk.clone());
            cloned.borrow_mut().relay_port = Some(port);
            *unwrap!(cloned.borrow_mut().crypto_box).borrow_mut() = Some(
                CryptoBox::try_from((server_pk, unwrap!(cloned.borrow().session_keypair).private_key())).unwrap()
            );

            let borrowed = cloned.borrow();
            let Some(kp) = borrowed.peer_keypair.as_ref() else {
                return;
            };

            let nodeid = borrowed.node.lock().unwrap().id().clone();
            let mut builder = PeerBuilder::new(&nodeid);
            let has_domain = borrowed.peer_domain.is_some();
            if domain_enabled && has_domain {
                builder.with_alternative_url(borrowed.peer_domain.as_ref().map(|v|v.as_str()));
            }

            let peer = builder.with_keypair(Some(kp))
                .with_origin(Some(borrowed.node.lock().unwrap().id()))
                .with_alternative_url(borrowed.peer_domain.as_ref().map(|v|v.as_str()))
                .with_port(port)
                .build();

            if let Some(url) = peer.alternative_url() {
                info!("-**- ActiveProxy: peer server: {}:{}, domain: {} -**-", unwrap!(borrowed.remote_addr).ip(), peer.port(), url);
            } else {
                info!("-**- ActiveProxy: peer server: {}:{} -**-", unwrap!(borrowed.remote_addr).ip(), peer.port());
            }

            cloned.borrow_mut().peer = Some(peer);
            // Will announce this peer in the next iteration if it's effective.
        }));

        let cloned = self.cloned();
        connection.with_on_opened_cb(Box::new(move |_: &ProxyConnection| {
            cloned.borrow_mut().server_failures = 0;
            cloned.borrow_mut().reconnect_delay = 0;
        }));

        let cloned = self.cloned();
        connection.with_on_open_failed_cb(Box::new(move |_: &ProxyConnection| {
            cloned.borrow_mut().server_failures += 1;
            if cloned.borrow().reconnect_delay < 64 {
                cloned.borrow_mut().reconnect_delay = (1 << cloned.borrow_mut().server_failures) * 1000;
            }
        }));

        let cloned = self.cloned();
        connection.with_on_closed_cb(Box::new(move |conn: &ProxyConnection| {
           cloned.borrow_mut().connections.remove(&conn.id());
        }));

        let cloned = self.cloned();
        connection.with_on_busy_cb(Box::new(move |_| {
            cloned.borrow_mut().inflights += 1;
            cloned.borrow_mut().last_idle_check_timestamp = SystemTime::UNIX_EPOCH;
        }));

        let cloned = self.cloned();
        connection.with_on_idle_cb(Box::new(move |_| {
            cloned.borrow_mut().inflights -= 1;
            if cloned.borrow().inflights == 0 {
                cloned.borrow_mut().last_idle_check_timestamp = SystemTime::now();
            }
        }));

        let conn_rc = Rc::new(RefCell::new(connection));
        self.connections.insert(conn_rc.borrow().id(), conn_rc.clone());

        self.last_reconnect_timestamp = SystemTime::now();
        conn_rc.clone()
            .borrow_mut()
            .try_connect_server().await
    }

    fn needs_new_connection(&self) -> bool {
        if self.connections.len() >= self.max_connections {
            return false;
        }
        if unwrap!(self.last_reconnect_timestamp.elapsed()).as_millis() < self.reconnect_delay {
            return false;
        }

        if self.connections.is_empty() {
            if self.server_pk.is_some() {
                self.reset();
            }
            return true;
        }
        if self.inflights == self.connections.len() {
            return true;
        }

        false   // Maybe refine other conditions later.
    }
}

pub(crate) fn run_loop(
    worker: Rc<RefCell<ProxyWorker>>,
    _quit: Arc<Mutex<bool>>
) -> Result<()> {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();

    rt.block_on(async move {
        let mut interval = tokio::time::interval(
            Duration::from_millis(HEALTH_CHECK_INTERVAL as u64
        ));

        _ = worker.borrow_mut().on_iteration().await;

        loop {
            let mut read_tasks = FuturesUnordered::new();
            for (_, item) in worker.borrow().connections.iter() {
                let item = item.clone();
                let worker = worker.clone();
                let rcvbuf = unwrap!(worker.borrow().rcvbuf).clone();
                read_tasks.push(async move {
                    let mut borrowed = item.borrow_mut();
                    match borrowed.relay_mut().read(&mut rcvbuf.borrow_mut()).await {
                        Ok(n) if n == 0 => {
                            info!("Connection {} was closed by the server.", borrowed.id());
                            Err(Error::State(format!("Connection {} was closed by the server.", borrowed.id())))
                        },
                        Ok(len) => {
                            println!(">>>> received {} bytes", len);
                            // borrowed.on_relay_data(&borrowed[..len]).await,
                            Ok(())
                        },
                        Err(e) => {
                            error!("Connection {} failed to read server with error: {}", borrowed.id(), e);
                            _ = borrowed.close().await;
                            Err(Error::from(e))
                        }
                    }.ok();
                });
            }

            tokio::select! {
                result = read_tasks.next() => {
                    match result {
                        Some(_) => {panic!(">>>>>line:{}", line!())},
                        None => println!("failed"),
                    }
                },

                _ = interval.tick() => {
                    _ = worker.borrow_mut().on_iteration().await;
                }
            }
        }
    })
}
