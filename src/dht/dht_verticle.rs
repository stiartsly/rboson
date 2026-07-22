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
    runtime,
    sync::{mpsc,oneshot},
};

use crate::{
    CryptoIdentity,
    Id, Network, NodeInfo,
    PeerInfo, Value,
    Result,
    errors::StateError
};
use crate::dht::{
    ConnectionStatusListener,
    dht::DHT,
    lookup_option::LookupOption,
    promise::Promise,
    storage::data_storage::DataStorage,
    timer_client::{LocalTimerClient as TimerClient, LocalTimerCmd as TimerCmd},
    timer_manager::LocalTimerManager as TimerManager,
    token_manager::TokenManager,
    rpc::rpc_server::RpcServer,
};

const CHANNEL_REQ_CLOSED: &str = "verticle request channel closed";
const CHANNEL_RSP_CLOSED: &str = "verticle response channel closed";

enum Cmd {
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
    StopAll {
        complete: oneshot::Sender<CmdResult<()>>,
    },
}

pub(crate) struct VerticleClient {
    ni          : NodeInfo,
    command_tx  : mpsc::UnboundedSender<Cmd>,
    handle      : Option<JoinHandle<()>>,
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
        if self.command_tx.send(
            Cmd::Bootstrap { nodes, complete: tx }
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
        if self.command_tx.send(
            Cmd::FindNode { target, option, complete: tx }
        ).is_err() {
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
        if self.command_tx.send(Cmd::FindValue {
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
        if self.command_tx.send(
            Cmd::StoreValue { value, expected_seq, complete: tx }
        ).is_err() {
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
        if self.command_tx.send(Cmd::FindPeer {
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
        if self.command_tx.send(
            Cmd::AnnouncePeer { peer, expected_seq, complete: tx }
        ).is_err() {
            return Err(StateError::new(CHANNEL_REQ_CLOSED));
        }
        self.rx_result(rx).await
    }

    pub(crate) async fn stop(&mut self) {
        info!("Stopping DHT verticle");
        let (tx, rx) = oneshot::channel();
        if self.command_tx.send(Cmd::StopAll { complete: tx }).is_ok() {
            let _ = rx.await;
        }

        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
        info!("DHT verticle stopped");
    }
}

#[derive(Default, Clone)]
pub(crate) struct VerticleOptions {
    pub(crate) identity     : Option<Arc<CryptoIdentity>>,
    pub(crate) storage      : Option<Arc<Mutex<dyn DataStorage>>>,
    pub(crate) token_man    : Option<Arc<TokenManager>>,
    pub(crate) listener     : Option<Arc<dyn ConnectionStatusListener>>,
    pub(crate) data_dir     : Option<PathBuf>,
    pub(crate) bootstrap_nodes  : Option<Vec<NodeInfo>>,
}

impl VerticleOptions {
    pub(crate) fn with_identity(mut self, identity: Arc<CryptoIdentity>) -> Self {
        self.identity = Some(identity);
        self
    }

    pub(crate) fn with_bootstrap(mut self, bootstrap_nodes: Vec<NodeInfo>) -> Self {
        self.bootstrap_nodes = Some(bootstrap_nodes);
        self
    }

    pub(crate) fn with_storage(mut self, storage: Arc<Mutex<dyn DataStorage>>) -> Self {
        self.storage = Some(storage);
        self
    }

    pub(crate) fn with_tokenman(mut self, token_man: Arc<TokenManager>) -> Self {
        self.token_man = Some(token_man);
        self
    }

    pub(crate) fn with_datadir(mut self, datadir: PathBuf) -> Self {
        self.data_dir = Some(datadir);
        self
    }

    pub(crate) fn with_listener(mut self, listener: Arc<dyn ConnectionStatusListener>) -> Self {
        self.listener = Some(listener);
        self
    }
}

pub(crate) struct Verticle {
    dht             : Rc<RefCell<DHT>>,
    timer_manager   : TimerManager,

    cmd_rx          : mpsc::UnboundedReceiver<Cmd>,
    tmr_rx          : mpsc::UnboundedReceiver<TimerCmd>,

    quit            : bool,
}

impl Verticle {
    fn new(
        options: VerticleOptions,
        network: Network,
        host: String,
        port: u16,
        cmd_rx: mpsc::UnboundedReceiver<Cmd>
    ) -> Result<Verticle> {
        let persist_file = options.data_dir.as_ref().map(|dir| {
            let filename = match network {
                Network::IPv4 => "dht4.cache",
                Network::IPv6 => "dht6.cache",
            };
            dir.join(filename)
        });

        let (tmr_tx, tmr_rx) = mpsc::unbounded_channel::<TimerCmd>();
        let timer_client = Rc::new(TimerClient::new(tmr_tx));
        let timer_manager = TimerManager::new();

        let dht = DHT::new(options, network, host, port, persist_file, timer_client)?;
        let dht = Rc::new(RefCell::new(dht));
        dht.borrow_mut().weak = Rc::downgrade(&dht);

        Ok(Self {
            dht,
            timer_manager,
            cmd_rx,
            tmr_rx,
            quit: false,
        })
    }

    async fn start(&mut self) -> Result<()> {
        self.dht.borrow_mut().start().await
    }

    fn ni(&self) -> NodeInfo {
        self.dht.borrow().ni()
    }

    fn handle_dht_cmd(
        &mut self,
        cmd: Cmd,
        pending: &mut FuturesUnordered<Pin<Box<dyn Future<Output=()>>>>
    ) {
        match cmd {
            Cmd::Bootstrap {
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
            Cmd::FindNode {
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
            Cmd::FindValue {
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
            Cmd::StoreValue {
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
            Cmd::FindPeer {
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
            Cmd::AnnouncePeer {
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
            Cmd::StopAll { complete } => {
                self.quit = true;
                self.timer_manager.stop_all();
                let _ = complete.send(Ok(()));
            }
        }
    }

    fn handle_timer_cmd(&mut self, cmd: TimerCmd) {
        match cmd {
            TimerCmd::Add { timer_id, delay, interval, cb } =>
                self.timer_manager.add_timer(timer_id, delay, interval, cb),

            TimerCmd::Cancel { timer_id } =>
                self.timer_manager.cancel_timer(timer_id),

            TimerCmd::Stop { complete } => {
                self.timer_manager.stop_all();
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
                Some(cmd) = self.cmd_rx.recv() => {
                    self.handle_dht_cmd(cmd, &mut pendings);
                }
                Some(cmd) = self.tmr_rx.recv() => {
                    self.handle_timer_cmd(cmd);
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
                Some(timer_id) = self.timer_manager.next_expired(), if !self.timer_manager.is_idle() => {
                    self.timer_manager.fire_expired(timer_id).await;
                }
                Some(_) = pendings.next() => {},
            }

            if self.quit {
                break;
            }
        }

        self.timer_manager.stop_all();
        self.dht.borrow_mut().stop().await;
        info!("DHT verticle exited run_loop");
    }
}

type StartupResult = StdResult<NodeInfo, String>;
pub(crate) fn deploy(
    options: VerticleOptions,
    network: Network,
    host: String,
    port: u16,
) -> Result<VerticleClient> {
    let (command_tx, command_rx) = mpsc::unbounded_channel::<Cmd>();
    let (startup_tx, startup_rx) = std_mpsc::sync_channel::<StartupResult>(1);

    let handle = std::thread::spawn(move || {
        let rt = runtime::Builder::new_current_thread()
            .enable_time()
            .enable_io()
            .build()
            .expect("dht verticle runtime should build");

        let local = tokio::task::LocalSet::new();
        rt.block_on(local.run_until(async move {
                let result = Verticle::new(options, network, host, port, command_rx);
                let mut vert = match result {
                    Ok(v) => v,
                    Err(e) => {
                        let _ = startup_tx.send(Err(format!("{e}")));
                        return;
                    }
                };

                let result = vert.start().await;
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

    match startup_rx.recv() {
        Ok(Ok(ni)) => Ok(VerticleClient {ni, command_tx, handle: Some(handle)}),
        Ok(Err(msg)) => Err(StateError::new(msg)),
        Err(_) => Err(StateError::new("dht verticle startup channel closed")),
    }
}
