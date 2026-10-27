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

use futures::{
    stream::{FuturesUnordered, StreamExt},
    FutureExt,
};
use log::{error, info, warn};
use std::{
    future::Future,
    net::SocketAddr,
    path::PathBuf,
    pin::Pin,
    rc::Rc,
    result::Result as StdResult,
    sync::{Arc, Mutex},
    thread::JoinHandle,
};
use tokio::{
    runtime,
    sync::{
        mpsc::{self, error::TrySendError},
        oneshot,
    },
    task,
};

use crate::{
    dht::DHT,
    lookup_option::LookupOption,
    rpc::rpc_server::RpcServer,
    storage::data_storage::DataStorage,
    token_manager::TokenManager,
    ConnectionStatusListener,
};
use boson_api::{
    Id,
    errors::StateError,
    CryptoIdentity,
    LocalBoxTimerClient as TimerClient,
    LocalBoxTimerCmd as TimerCmd,
    LocalBoxTimerManager as TimerManager,
    Network,
    NodeInfo,
    PeerInfo,
    Promise,
    Result,
    Value,
};

/// UDP datagrams are copied out of the receive buffer before they are queued.
const RECEIVE_BUFFER_BYTES: usize = 2048;
/// Bound the packet hand-off queue so a flood cannot grow without limit.
const PACKET_QUEUE_CAPACITY: usize = 256;

// ---------------------------------------------------------------------------
// Node-facing command channel
// ---------------------------------------------------------------------------

type CmdResult<T> = StdResult<T, String>;
type LocalTask = Pin<Box<dyn Future<Output = ()>>>;
type PendingTasks = FuturesUnordered<LocalTask>;

const CHANNEL_REQ_CLOSED: &str = "verticle request channel closed";
const CHANNEL_RSP_CLOSED: &str = "verticle response channel closed";

/// A command sent by the owning `Node` to the DHT thread.
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

/// The thread-safe handle used by `Node`.
pub(crate) struct VerticleClient {
    ni: NodeInfo,
    event_tx: mpsc::UnboundedSender<CallEvent>,
    handle: Mutex<Option<JoinHandle<()>>>,
}

impl VerticleClient {
    pub(crate) fn ni(&self) -> NodeInfo {
        self.ni.clone()
    }

    /// Sends one command and waits for its result.
    async fn call<T>(
        &self,
        event: impl FnOnce(oneshot::Sender<CmdResult<T>>) -> CallEvent,
    ) -> Result<T> {
        let (complete, response) = oneshot::channel();
        self.event_tx
            .send(event(complete))
            .map_err(|_| StateError::new(CHANNEL_REQ_CLOSED))?;

        match response.await {
            Ok(Ok(value)) => Ok(value),
            Ok(Err(message)) => Err(StateError::new(message)),
            Err(_) => Err(StateError::new(CHANNEL_RSP_CLOSED)),
        }
    }

    pub(crate) async fn bootstrap(&self, nodes: Vec<NodeInfo>) -> Result<()> {
        self.call(|complete| CallEvent::Bootstrap { nodes, complete })
            .await
    }

    pub(crate) async fn find_node(
        &self,
        target: Id,
        option: LookupOption,
    ) -> Result<Option<NodeInfo>> {
        self.call(|complete| CallEvent::FindNode {
            target,
            option,
            complete,
        })
        .await
    }

    pub(crate) async fn find_value(
        &self,
        target: Id,
        expected_seq: i32,
        option: LookupOption,
    ) -> Result<Option<Value>> {
        self.call(|complete| CallEvent::FindValue {
            target,
            expected_seq,
            option,
            complete,
        })
        .await
    }

    pub(crate) async fn store_value(&self, value: Value, expected_seq: i32) -> Result<()> {
        self.call(|complete| CallEvent::StoreValue {
            value,
            expected_seq,
            complete,
        })
        .await
    }

    pub(crate) async fn find_peer(
        &self,
        target: Id,
        expected_seq: i32,
        expected_count: usize,
        option: LookupOption,
    ) -> Result<Vec<PeerInfo>> {
        self.call(|complete| CallEvent::FindPeer {
            target,
            expected_seq,
            expected_count,
            option,
            complete,
        })
        .await
    }

    pub(crate) async fn announce_peer(&self, peer: PeerInfo, expected_seq: i32) -> Result<()> {
        self.call(|complete| CallEvent::AnnouncePeer {
            peer,
            expected_seq,
            complete,
        })
        .await
    }

    async fn start(&mut self) -> Result<()> {
        self.call(|complete| CallEvent::Start { complete }).await
    }

    /// Asks the thread to stop and joins it without blocking an async worker.
    pub(crate) async fn stop(&self) {
        info!("Stopping DHT verticle");
        let _ = self
            .call(|complete| CallEvent::StopAll { complete })
            .await;

        let handle = self.handle.lock().unwrap().take();
        if let Some(handle) = handle {
            if let Ok(joined) = task::spawn_blocking(move || handle.join()).await {
                let _ = joined;
            }
        }
        info!("DHT verticle stopped");
    }
}

// ---------------------------------------------------------------------------
// Shared options and startup
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub(crate) struct VerticleOptions {
    pub(crate) identity: Arc<CryptoIdentity>,
    pub(crate) storage: Arc<Mutex<dyn DataStorage>>,
    pub(crate) token_man: Arc<TokenManager>,
    pub(crate) listener: Arc<dyn ConnectionStatusListener>,
    pub(crate) bootstrap_nodes: Vec<NodeInfo>,
    pub(crate) developer_mode: bool,
}

struct Verticle {
    dht: Rc<DHT>,
    event_rx: mpsc::UnboundedReceiver<CallEvent>,
    timer_rx: Option<mpsc::UnboundedReceiver<TimerCmd>>,
    quit: bool,
}

impl Verticle {
    fn new(
        options: VerticleOptions,
        data_dir: String,
        network: Network,
        host: String,
        port: u16,
        event_rx: mpsc::UnboundedReceiver<CallEvent>,
    ) -> Result<Self> {
        let data_dir = PathBuf::from(data_dir);
        let persist_file = Some(data_dir.join(match network {
            Network::IPv4 => "dht4.cache",
            Network::IPv6 => "dht6.cache",
        }));

        let (timer_tx, timer_rx) = mpsc::unbounded_channel::<TimerCmd>();
        let timer_client = Rc::new(TimerClient::new(timer_tx));
        let dht = DHT::new(options, network, host, port, persist_file, timer_client);

        Ok(Self {
            dht,
            event_rx,
            timer_rx: Some(timer_rx),
            quit: false,
        })
    }

    async fn start0(&mut self) -> Result<()> {
        self.dht.start0().await
    }

    fn ni(&self) -> NodeInfo {
        self.dht.ni().clone()
    }

    /// Dispatches one Node command without awaiting the DHT operation itself.
    fn handle_event(&mut self, event: CallEvent, pending: &mut PendingTasks) {
        match event {
            CallEvent::Bootstrap { nodes, complete } => {
                let dht = self.dht.clone();
                let task = async move {
                    let (promise, future) = Promise::<()>::pair();
                    dht.bootstrap(nodes, promise).await;
                    future.await
                };
                forward(task, complete, pending);
            }
            CallEvent::FindNode {
                target,
                option,
                complete,
            } => {
                let (promise, future) = Promise::<Option<NodeInfo>>::pair();
                self.dht.find_node(target, option, promise);
                forward(future, complete, pending);
            }
            CallEvent::FindValue {
                target,
                expected_seq,
                option,
                complete,
            } => {
                let (promise, future) = Promise::<Option<Value>>::pair();
                self.dht
                    .find_value(target, expected_seq, option, promise);
                forward(future, complete, pending);
            }
            CallEvent::StoreValue {
                value,
                expected_seq,
                complete,
            } => {
                let (promise, future) = Promise::<()>::pair();
                self.dht.store_value(value, expected_seq, promise);
                forward(future, complete, pending);
            }
            CallEvent::FindPeer {
                target,
                expected_seq,
                expected_count,
                option,
                complete,
            } => {
                let (promise, future) = Promise::<Vec<PeerInfo>>::pair();
                self.dht.find_peer(
                    target,
                    expected_seq,
                    expected_count,
                    option,
                    promise,
                );
                forward(future, complete, pending);
            }
            CallEvent::AnnouncePeer {
                peer,
                expected_seq,
                complete,
            } => {
                let (promise, future) = Promise::<()>::pair();
                self.dht.announce_peer(peer, expected_seq, promise);
                forward(future, complete, pending);
            }
            CallEvent::Start { complete } => {
                let dht = self.dht.clone();
                let task = async move {
                    let (promise, future) = Promise::<()>::pair();
                    let _ = dht.start(promise).await;
                    future.await
                };
                forward(task, complete, pending);
            }
            CallEvent::StopAll { complete } => {
                self.quit = true;
                let _ = complete.send(Ok(()));
            }
        }
    }

    async fn run_loop(mut self) {
        let server = self.dht.rs();
        let socket = match server.rx_tokio_socket() {
            Ok(socket) => socket,
            Err(error) => {
                error!("Failed to get rx socket: {error}");
                return;
            }
        };

        if !server.prepare() {
            error!("Failed to prepare RPC server");
            self.dht.stop().await;
            return;
        }

        let mut pending = PendingTasks::new();

        // Timers are driven by their own local task so a slow timer callback
        // cannot stall the socket or Node command loop.
        let timer_rx = self
            .timer_rx
            .take()
            .expect("timer receiver is consumed exactly once");
        pending.push(run_timer_worker(TimerManager::new(), timer_rx).boxed_local());

        // Packets are processed by one worker, preserving request ordering while
        // keeping `recv_from` free to continue draining the socket.
        let (packet_tx, packet_rx) = mpsc::channel::<(Vec<u8>, SocketAddr)>(PACKET_QUEUE_CAPACITY);
        pending.push(run_packet_worker(self.dht.rs(), packet_rx).boxed_local());

        let mut buf = vec![0u8; RECEIVE_BUFFER_BYTES];

        while !self.quit {
            tokio::select! {
                event = self.event_rx.recv() => {
                    match event {
                        Some(event) => self.handle_event(event, &mut pending),
                        None => self.quit = true,
                    }
                }
                packet = socket.recv_from(&mut buf) => {
                    self.dispatch_packet(packet, &buf, &packet_tx);
                }
                Some(()) = pending.next() => {}
            }
        }

        // Dropping the workers cancels queued timer/packet work before the DHT
        // state is torn down.
        drop(packet_tx);
        pending.clear();
        self.dht.stop().await;
        info!("DHT verticle exited run_loop");
    }

    /// Copies one datagram into the packet queue without blocking the loop.
    fn dispatch_packet(
        &self,
        packet: StdResult<(usize, SocketAddr), std::io::Error>,
        buf: &[u8],
        packet_tx: &mpsc::Sender<(Vec<u8>, SocketAddr)>,
    ) {
        let (len, from) = match packet {
            Ok(packet) => packet,
            Err(error) => {
                error!("Receiving data error: {error}");
                return;
            }
        };

        let data = buf[..len].to_vec();
        match packet_tx.try_send((data, from)) {
            Ok(()) => {}
            Err(TrySendError::Full(_)) => {
                warn!("DHT packet queue is full; dropping packet from {from}");
            }
            Err(TrySendError::Closed(_)) => {
                error!("DHT packet worker stopped; dropping packet from {from}");
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Local workers
// ---------------------------------------------------------------------------

/// Completes a Node command from a DHT future.
fn forward<T: 'static>(
    future: impl Future<Output = Result<T>> + 'static,
    complete: oneshot::Sender<CmdResult<T>>,
    pending: &mut PendingTasks,
) {
    pending.push(
        async move {
            let _ = complete.send(future.await.map_err(|error| error.to_string()));
        }
        .boxed_local(),
    );
}

/// Owns the timer queue; callbacks run here, not on the main event loop.
async fn run_timer_worker(
    mut timers: TimerManager,
    mut timer_rx: mpsc::UnboundedReceiver<TimerCmd>,
) {
    loop {
        tokio::select! {
            command = timer_rx.recv() => {
                match command {
                    Some(TimerCmd::Add {
                        timer_id,
                        delay,
                        interval,
                        cb,
                    }) => timers.add_timer(timer_id, delay, interval, cb),
                    Some(TimerCmd::Cancel { timer_id }) => timers.cancel_timer(timer_id),
                    Some(TimerCmd::Stop { complete }) => {
                        timers.stop_all();
                        let _ = complete.send(());
                        break;
                    }
                    None => break,
                }
            }
            Some(timer_id) = timers.next_expired(), if !timers.is_idle() => {
                timers.fire_expired(timer_id).await;
            }
        }
    }
}

/// Serializes packet decoding/handling away from the socket receive loop.
async fn run_packet_worker(
    server: Rc<RpcServer>,
    mut packet_rx: mpsc::Receiver<(Vec<u8>, SocketAddr)>,
) {
    while let Some((data, from)) = packet_rx.recv().await {
        RpcServer::handle_packet(server.clone(), &data, from).await;
    }
}

// ---------------------------------------------------------------------------
// Deployment
// ---------------------------------------------------------------------------

pub(crate) async fn deploy(
    options: VerticleOptions,
    data_dir: &str,
    network: Network,
    host: String,
    port: u16,
) -> Result<VerticleClient> {
    let (event_tx, event_rx) = mpsc::unbounded_channel::<CallEvent>();
    let (startup_tx, startup_rx) =
        oneshot::channel::<StdResult<NodeInfo, String>>();
    let data_dir = data_dir.to_owned();
    let thread_name = format!("dht-{network}-verticle");

    let handle = std::thread::Builder::new()
        .name(thread_name)
        .spawn(move || {
            let rt = match runtime::Builder::new_current_thread()
                .enable_time()
                .enable_io()
                .build()
            {
                Ok(rt) => rt,
                Err(error) => {
                    let _ = startup_tx.send(Err(format!(
                        "Building DHT verticle runtime failed: {error}"
                    )));
                    return;
                }
            };

            let local = task::LocalSet::new();
            rt.block_on(local.run_until(async move {
                let mut vert = match Verticle::new(
                    options,
                    data_dir,
                    network,
                    host,
                    port,
                    event_rx,
                ) {
                    Ok(vert) => vert,
                    Err(error) => {
                        let _ = startup_tx.send(Err(error.to_string()));
                        return;
                    }
                };

                if let Err(error) = vert.start0().await {
                    let _ = startup_tx.send(Err(error.to_string()));
                    return;
                }

                if startup_tx.send(Ok(vert.ni())).is_err() {
                    return;
                }
                vert.run_loop().await;
            }));
        })
        .map_err(|error| {
            StateError::new(format!("Spawning DHT verticle thread failed: {error}"))
        })?;

    let ni = match startup_rx.await {
        Ok(Ok(ni)) => ni,
        Ok(Err(message)) => return Err(StateError::new(message)),
        Err(_) => {
            return Err(StateError::new(
                "DHT verticle startup channel closed",
            ))
        }
    };

    let mut client = VerticleClient {
        ni,
        event_tx,
        handle: Mutex::new(Some(handle)),
    };

    if let Err(error) = client.start().await {
        client.stop().await;
        return Err(error.into());
    }
    Ok(client)
}
