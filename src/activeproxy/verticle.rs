use std::{
    net::SocketAddr,
    rc::Rc,
    result::Result as StdResult,
    sync::{mpsc as std_mpsc, Arc},
    thread::JoinHandle,
};
use log::info;
use tokio::{
    runtime,
    sync::{mpsc, oneshot},
    task,
};

use crate::{
    Id,
    PeerInfo,
    dht::Node,
    errors::{Result, StateError},
    signature,
};
use super::{
    LocalBoxTimerCmd as TimerCmd,
    LocalBoxTimerClient as TimerClient,
    LocalBoxTimerManager as TimerManager,
    session::ProxySession
};

pub(crate) struct VerticleClient {
    cmd_tx  : mpsc::UnboundedSender<Cmd>,
    handle  : Option<JoinHandle<()>>,
}

enum Cmd {
    Stop { complete: oneshot::Sender<()> },
}

impl VerticleClient {
    fn new(
        cmd_tx: mpsc::UnboundedSender<Cmd>,
        handle: JoinHandle<()>
    ) -> Self {
        Self {
            cmd_tx,
            handle: Some(handle),
        }
    }

    pub(crate) async fn stop(&mut self) -> Result<()> {
        info!("Stopping ActiveProxy verticle");

        let (tx, rx) = oneshot::channel();
        if self.cmd_tx.send(Cmd::Stop { complete: tx }).is_ok() {
            let _ = rx.await;
        }
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }

        info!("ActiveProxy verticle stopped");
        Ok(())
    }
}

pub(crate) struct VerticleOptions {
    pub(super) node                        : Option<Arc<Node>>,
    pub(super) service_peerid              : Id,
    pub(super) service_peer                : Option<PeerInfo>,
    pub(super) service_endpoint            : String,
    pub(super) upstream_endpoint           : String,
    pub(super) upstream_addr               : SocketAddr,
    pub(super) user_id                     : Id,
    pub(super) device_key                  : signature::PrivateKey,
    pub(super) name_access_enabled         : bool,
    pub(super) announce_peer_enabled       : bool,
}

impl VerticleOptions {
    pub(crate) fn new(
        node: Option<Arc<Node>>,
        service_peerid: Id,
        service_peer: Option<PeerInfo>,
        service_endpoint: String,
        upstream_endpoint: String,
        upstream_addr: SocketAddr,
        user_id: Id,
        device_key: signature::PrivateKey,
    ) -> Self {
        Self {
            node,
            service_peerid,
            service_peer,
            service_endpoint,
            upstream_endpoint,
            upstream_addr,
            user_id,
            device_key,
            name_access_enabled: false,
            announce_peer_enabled: false,
        }
    }
}

pub(crate) struct Verticle {
    session         : Rc<ProxySession>,
    timer_manager   : TimerManager,
    cmd_rx          : mpsc::UnboundedReceiver<Cmd>,
    tmr_rx          : mpsc::UnboundedReceiver<TimerCmd>,
    quit            : bool,
}

impl Verticle {
    fn new(
        options: VerticleOptions,
        cmd_rx: mpsc::UnboundedReceiver<Cmd>
    ) -> Result<Self> {
        let (tmr_tx, tmr_rx) = mpsc::unbounded_channel::<TimerCmd>();
        let timer_client = TimerClient::new(tmr_tx);
        let timer_manager = TimerManager::new();
        Ok(Self {
            session: ProxySession::new(options, timer_client)?,
            timer_manager,
            cmd_rx,
            tmr_rx,
            quit: false,
        })
    }

    async fn start0(&mut self) -> Result<()> {
        info!("ActiveProxy verticle starting");
        self.session.start().await
    }

    fn handle_cmd(&mut self, cmd: Cmd) {
        match cmd {
            Cmd::Stop { complete } => {
                self.quit = true;
                self.timer_manager.stop_all();
                let _ = complete.send(());
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

    async fn run_loop(&mut self) {
        loop {
            tokio::select! {
                Some(cmd) = self.cmd_rx.recv() => {
                    self.handle_cmd(cmd);
                }
                Some(cmd) = self.tmr_rx.recv() => {
                    self.handle_timer_cmd(cmd);
                }
                Some(timer_id) = self.timer_manager.next_expired(), if !self.timer_manager.is_idle() => {
                    self.timer_manager.fire_expired(timer_id).await;
                }
            }

            if self.quit {
                break;
            }
        }

        self.session.stop().await;
        self.session.close();
        info!("ActiveProxy verticle stopped");
    }
}

type StartupResult = StdResult<(), String>;

pub(crate) fn deploy(options: VerticleOptions) -> Result<VerticleClient> {
    let (cmd_tx, cmd_rx) = mpsc::unbounded_channel::<Cmd>();
    let (startup_tx, startup_rx) = std_mpsc::sync_channel::<StartupResult>(1);

    let handle = std::thread::spawn(move || {
        let rt = runtime::Builder::new_current_thread()
            .enable_time()
            .enable_io()
            .build()
            .expect("ActiveProxy runtime verticle should be built");

        let local = task::LocalSet::new();
        rt.block_on(local.run_until(async move {
            let mut vert = match Verticle::new(options, cmd_rx) {
                Ok(v) => v,
                Err(e) => {
                    let _ = startup_tx.send(Err(e.to_string()));
                    return;
                }
            };

            if let Err(e) = vert.start0().await {
                let _ = startup_tx.send(Err(e.to_string()));
                return;
            }
            let _ = startup_tx.send(Ok(()));

            vert.run_loop().await;
        }));
    });

    match startup_rx.recv() {
        Ok(Ok(())) => Ok(VerticleClient::new(cmd_tx, handle)),
        Ok(Err(msg)) => Err(StateError::new(msg)),
        Err(_) => Err(StateError::new("ActiveProxy verticle startup channel closed")),
    }
}
