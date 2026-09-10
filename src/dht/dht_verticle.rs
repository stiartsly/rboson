use std::{
    rc::Rc,
    pin::Pin,
    cell::RefCell,
    path::PathBuf,
    result::Result as StdResult,
    sync::{mpsc as std_mpsc, Arc, Mutex},
    thread::JoinHandle,
    future::Future,
};
use futures::{
    stream::{FuturesUnordered, StreamExt },
    FutureExt,
};
use log::{error, info};
use tokio::{
    task,
    runtime,
    sync::{mpsc,oneshot},
};

use crate::{
    CryptoIdentity,
    Id, Network, NodeInfo,
    PeerInfo, Value,
    Result,
    errors::StateError,
    LocalBoxTimerClient as TimerClient,
    LocalBoxTimerCmd as TimerCmd,
    LocalBoxTimerManager as TimerManager,
    Promise
};
use crate::dht::{
    ConnectionStatusListener,
    dht::DHT,
    lookup_option::LookupOption,
    storage::data_storage::DataStorage,
    token_manager::TokenManager,
    rpc::rpc_server::RpcServer,
};

const CHANNEL_REQ_CLOSED: &str = "verticle request channel closed";
const CHANNEL_RSP_CLOSED: &str = "verticle response channel closed";

enum CallEvent {
    Bootstrap {
        nodes: Vec<NodeInfo>,
        complete: oneshot::Sender<CmdResult<()>>,
    },
    FindNode {
        target: Id,
        option: LookupOption,
        complete: oneshot::Sender<CmdResult<Option<NodeInfo>>>,
    },
    FindValue {
        target: Id,
        expected_seq: i32,
        option: LookupOption,
        complete: oneshot::Sender<CmdResult<Option<Value>>>,
    },
    StoreValue {
        value: Value,
        expected_seq: i32,
        complete: oneshot::Sender<CmdResult<()>>,
    },
    FindPeer {
        target: Id,
        expected_seq: i32,
        expected_count: usize,
        option: LookupOption,
        complete: oneshot::Sender<CmdResult<Vec<PeerInfo>>>,
    },
    AnnouncePeer {
        peer: PeerInfo,
        expected_seq: i32,
        complete: oneshot::Sender<CmdResult<()>>,
    },
    Start {
        complete: oneshot::Sender<CmdResult<()>>,
    },
    StopAll {
        complete: oneshot::Sender<CmdResult<()>>,
    },
}

pub(crate) struct VerticleClient {
    ni          : NodeInfo,
    event_tx    : mpsc::UnboundedSender<CallEvent>,
    handle      : Mutex<Option<JoinHandle<()>>>,
}
type CmdResult<T> = StdResult<T, String>;

impl VerticleClient {
    pub(crate) fn ni(&self) -> NodeInfo {
        self.ni.clone()
    }

    async fn rx_result<T>(
        &self,
        rx: oneshot::Receiver<CmdResult<T>>,
    ) -> Result<T> {
        match rx.await {
            Ok(Ok(v)) => Ok(v),
            Ok(Err(msg)) => Err(StateError::new(msg)),
            Err(_) => Err(StateError::new(CHANNEL_RSP_CLOSED)),
        }
    }

    pub(crate) async fn bootstrap(
        &self,
        nodes: Vec<NodeInfo>
    ) -> Result<()> {
        let (tx, rx) = oneshot::channel();
        if self.event_tx.send(
            CallEvent::Bootstrap { nodes, complete: tx }
        ).is_err() {
            return Err(StateError::new(CHANNEL_REQ_CLOSED));
        }
        self.rx_result(rx).await
    }

    pub(crate) async fn find_node(
        &self,
        target: Id,
        option: LookupOption
    ) -> Result<Option<NodeInfo>> {
        let (tx, rx) = oneshot::channel();
        if self.event_tx.send(CallEvent::FindNode {
            target,
            option,
            complete: tx
        }).is_err() {
            return Err(StateError::new(CHANNEL_REQ_CLOSED));
        }
        self.rx_result(rx).await
    }

    pub(crate) async fn find_value(
        &self,
        target: Id,
        expected_seq: i32,
        option: LookupOption
    ) -> Result<Option<Value>> {
        let (tx, rx) = oneshot::channel();
        if self.event_tx.send(CallEvent::FindValue {
            target,
            expected_seq,
            option,
            complete: tx,
        }).is_err() {
            return Err(StateError::new(CHANNEL_REQ_CLOSED));
        }
        self.rx_result(rx).await
    }

    pub(crate) async fn store_value(
        &self,
        value: Value,
        expected_seq: i32
    ) -> Result<()> {
        let (tx, rx) = oneshot::channel();
        if self.event_tx.send(CallEvent::StoreValue {
            value,
            expected_seq,
            complete: tx
        }).is_err() {
            return Err(StateError::new(CHANNEL_REQ_CLOSED));
        }
        self.rx_result(rx).await
    }

    pub(crate) async fn find_peer(
        &self,
        target: Id,
        expected_seq: i32,
        expected_count: usize,
        option: LookupOption
    ) -> Result<Vec<PeerInfo>> {
        let (tx, rx) = oneshot::channel();
        if self.event_tx.send(CallEvent::FindPeer {
            target,
            expected_seq,
            expected_count,
            option,
            complete: tx,
        }).is_err() {
            return Err(StateError::new(CHANNEL_REQ_CLOSED));
        }
        self.rx_result(rx).await
    }

    pub(crate) async fn announce_peer(
        &self,
        peer: PeerInfo,
        expected_seq: i32,
    ) -> Result<()> {
        let (tx, rx) = oneshot::channel();
        if self.event_tx.send(CallEvent::AnnouncePeer {
            peer,
            expected_seq,
            complete: tx
        }).is_err() {
            return Err(StateError::new(CHANNEL_REQ_CLOSED));
        }
        self.rx_result(rx).await
    }

    async fn start(&mut self) -> Result<()> {
        let (tx, rx) = oneshot::channel();
        if self.event_tx.send(CallEvent::Start {
            complete: tx
        }).is_err() {
            return Err(StateError::new(CHANNEL_REQ_CLOSED));
        }
        self.rx_result(rx).await
    }

    pub(crate) async fn stop(&self) {
        info!("Stopping DHT verticle");
        let (tx, rx) = oneshot::channel();
        if self.event_tx.send(CallEvent::StopAll {
            complete: tx
        }).is_ok() {
            let _ = rx.await;
        }

        if let Some(handle) = self.handle.lock().unwrap().take() {
            let _ = handle.join();
        }
        info!("DHT verticle stopped");
    }
}

#[derive(Clone)]
pub(crate) struct VerticleOptions {
    pub(crate) identity     : Arc<CryptoIdentity>,
    pub(crate) storage      : Arc<Mutex<dyn DataStorage>>,
    pub(crate) token_man    : Arc<TokenManager>,
    pub(crate) listener     : Arc<dyn ConnectionStatusListener>,
    pub(crate) bootstrap_nodes  : Vec<NodeInfo>,
}

pub(crate) struct Verticle {
    dht         : Rc<RefCell<DHT>>,
    timerman    : TimerManager,

    event_rx    : mpsc::UnboundedReceiver<CallEvent>,
    cmd_rx      : mpsc::UnboundedReceiver<TimerCmd>,
    quit        : bool,
}

impl Verticle {
    fn new(
        options: VerticleOptions,
        data_dir: String,
        network: Network,
        host: String,
        port: u16,
        event_rx: mpsc::UnboundedReceiver<CallEvent>
    ) -> Result<Verticle> {
        let data_dir = PathBuf::from(data_dir);
        let persist_file = Some(data_dir.join(match network {
            Network::IPv4 => "dht4.cache",
            Network::IPv6 => "dht6.cache",
        }));

        let (cmd_tx, cmd_rx) = mpsc::unbounded_channel::<TimerCmd>();
        let timer_client = Rc::new(TimerClient::new(cmd_tx));
        let dht = Rc::new(RefCell::new(DHT::new(
            options,
            network, host, port,
            persist_file,
            timer_client
        )));
        dht.borrow_mut().weak = Rc::downgrade(&dht);

        let timerman = TimerManager::new();
        Ok(Self {
            dht,
            timerman,
            event_rx,
            cmd_rx,
            quit: false,
        })
    }

    async fn start0(&mut self) -> Result<()> {
        self.dht.borrow_mut().start0().await
    }

    fn ni(&self) -> NodeInfo {
        self.dht.borrow().ni()
    }

    fn handle_events(
        &mut self,
        event: CallEvent,
        pending: &mut FuturesUnordered<Pin<Box<dyn Future<Output=()>>>>
    ) {
        match event {
            CallEvent::Bootstrap {
                nodes,
                complete
            } => {
                let dht = self.dht.clone();
                pending.push(async move {
                    let (promise, future) = Promise::<()>::pair();
                    dht.borrow_mut().bootstrap(nodes, promise).await;
                    let _ = complete.send(
                        future.await.map_err(|e| format!("{e}"))
                    );
                }.boxed_local());
            }
            CallEvent::FindNode {
                target,
                option,
                complete,
            } => {
                let dht = self.dht.clone();
                pending.push(async move {
                    let (promise, future) = Promise::<Option<NodeInfo>>::pair();
                    dht.borrow().find_node(target, option, promise);
                    let _ = complete.send(
                        future.await.map_err(|e| format!("{e}"))
                    );
                }.boxed_local());
            }
            CallEvent::FindValue {
                target,
                expected_seq,
                option,
                complete,
            } => {
                let dht = self.dht.clone();
                pending.push(async move {
                    let (promise, future) = Promise::<Option<Value>>::pair();
                    dht.borrow().find_value(target, expected_seq, option, promise);
                    let _ = complete.send(
                        future.await.map_err(|e| format!("{e}"))
                    );
                }.boxed_local());
            }
            CallEvent::StoreValue {
                value,
                expected_seq,
                complete,
            } => {
                let dht = self.dht.clone();
                pending.push(async move {
                    let (promise, future) = Promise::<()>::pair();
                    dht.borrow().store_value(value, expected_seq, promise);
                    let _ = complete.send(
                        future.await.map_err(|e| format!("{e}"))
                    );
                }.boxed_local());
            }
            CallEvent::FindPeer {
                target,
                expected_seq,
                expected_count,
                option,
                complete,
            } => {
                let dht = self.dht.clone();
                pending.push(async move {
                    let (promise, future) = Promise::<Vec<PeerInfo>>::pair();
                    dht.borrow().find_peer(target, expected_seq, expected_count, option, promise);
                    let _ = complete.send(
                        future.await.map_err(|e| format!("{e}"))
                    );
                }.boxed_local());
            }
            CallEvent::AnnouncePeer {
                peer,
                expected_seq,
                complete,
            } => {
                let dht = self.dht.clone();
                pending.push(async move {
                    let (promise, future) = Promise::<()>::pair();
                    dht.borrow().announce_peer(peer, expected_seq, promise);
                    let _ = complete.send(
                        future.await.map_err(|e| format!("{e}"))
                    );
                }.boxed_local());
            }
            CallEvent::Start { complete } => {
                let dht = self.dht.clone();
                pending.push(async move {
                    let (promise, future) = Promise::<()>::pair();
                    let _ = dht.borrow_mut().start(promise).await;
                    let _ = complete.send(
                        future.await.map_err(|e| format!("{e}"))
                    );
                }.boxed_local());
            }
            CallEvent::StopAll { complete } => {
                self.quit = true;
                self.timerman.stop_all();
                let _ = complete.send(Ok(()));
            }
        }
    }

    fn handle_commands(&mut self, cmd: TimerCmd) {
        match cmd {
            TimerCmd::Add { timer_id, delay, interval, cb } =>
                self.timerman.add_timer(timer_id, delay, interval, cb),

            TimerCmd::Cancel { timer_id } =>
                self.timerman.cancel_timer(timer_id),

            TimerCmd::Stop { complete } => {
                self.timerman.stop_all();
                let _ = complete.send(());
            }
        }
    }

    async fn run_loop(mut self) {
        let mut buf = vec![0u8; 2048];
        let mut pendings = FuturesUnordered::<Pin<Box<dyn Future<Output=()>>>>::new();

        let cloned_server = self.dht.borrow().rs();
        let socket = match cloned_server.borrow().rx_tokio_socket() {
            Ok(socket) => socket,
            Err(e) => {
                error!("Failed to get rx socket: {e}");
                return;
            }
        };

        if !cloned_server.borrow_mut().prepare() {
            error!("Failed to prepare RPC server");
            self.dht.borrow_mut().stop().await;
            return;
        }

        loop {
            tokio::select! {
                Some(event) = self.event_rx.recv() => {
                    self.handle_events(event, &mut pendings);
                }
                Some(cmd) = self.cmd_rx.recv() => {
                    self.handle_commands(cmd);
                }
                packet = socket.recv_from(&mut buf) => {
                    match packet {
                        Ok((len, from)) => {
                            let rs = self.dht.borrow().rs();
                            RpcServer::handle_packet(rs, &buf[..len], from).await;
                        }
                        Err(e) => {
                            error!("Receiving data error: {e}");
                            continue;
                        }
                    }

                },
                Some(timer_id) = self.timerman.next_expired(), if !self.timerman.is_idle() => {
                    self.timerman.fire_expired(timer_id).await;
                }
                Some(_) = pendings.next() => {},
            }

            if self.quit {
                break;
            }
        }

        self.timerman.stop_all();
        self.dht.borrow_mut().stop().await;
        info!("DHT verticle exited run_loop");
    }
}

pub(crate) async fn deploy(
    options: VerticleOptions,
    data_dir: &str,
    network: Network,
    host: String,
    port: u16,
) -> Result<VerticleClient> {
    let (event_tx, event_rx) = mpsc::unbounded_channel::<CallEvent>();
    let (startup_tx, startup_rx) = std_mpsc::sync_channel::<StdResult<NodeInfo, String>>(1);
    let data_dir = data_dir.into();
    let handle = std::thread::spawn(move || {
        let rt = runtime::Builder::new_current_thread()
            .enable_time()
            .enable_io()
            .build()
            .expect("dht verticle runtime should build");

        let local = task::LocalSet::new();
        rt.block_on(local.run_until(async move {
            let result = Verticle::new(options, data_dir, network, host, port, event_rx);
            let mut vert = match result {
                Ok(v) => v,
                Err(e) => {
                    let _ = startup_tx.send(Err(format!("{e}")));
                    return;
                }
            };

            let result = vert.start0().await;
            match result {
                Ok(()) => {
                    let _ = startup_tx.send(Ok(vert.ni()));
                }
                Err(e) => {
                    let _ = startup_tx.send(Err(format!("{e}")));
                    return;
                }
            }
            vert.run_loop().await;
        }));
    });

    let mut vert = match startup_rx.recv() {
        Ok(Ok(ni)) => VerticleClient {
            ni,
            event_tx,
            handle: Mutex::new(Some(handle)),
        },
        Ok(Err(msg)) => return Err(StateError::new(msg)),
        Err(_) => return Err(StateError::new("dht verticle startup channel closed")),
    };

    vert.start().await.map(|_| vert)
}
