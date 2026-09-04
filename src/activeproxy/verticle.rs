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
    event_tx  : mpsc::UnboundedSender<Event>,
    handle  : Option<JoinHandle<()>>,
}

enum Event {
    Start   { complete: oneshot::Sender<StdResult<(),String>> },
    Stop    { complete: oneshot::Sender<StdResult<(),String>> },
}

impl VerticleClient {
    fn new(
        event_tx: mpsc::UnboundedSender<Event>,
        handle: JoinHandle<()>
    ) -> Self {
        Self {
            event_tx,
            handle: Some(handle),
        }
    }

    pub(crate) async fn start(&self) -> Result<()> {
        info!("Starting ActiveProxy verticle");

        let (tx, rx) = oneshot::channel();
        if self.event_tx.send(Event::Start { complete: tx }).is_ok() {
            let _ = rx.await;
        }

        info!("ActiveProxy verticle started");
        Ok(())
    }

    pub(crate) async fn stop(&mut self) -> Result<()> {
        info!("Stopping ActiveProxy verticle");

        let (tx, rx) = oneshot::channel();
        if self.event_tx.send(Event::Stop { complete: tx }).is_ok() {
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
    pub(super) service_endpoint            : String,
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
        service_endpoint: String,
        upstream_addr: SocketAddr,
        user_id: Id,
        device_key: signature::PrivateKey,
    ) -> Self {
        Self {
            node,
            service_peerid,
            service_endpoint,
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
    event_rx          : mpsc::UnboundedReceiver<Event>,
    tmr_rx          : mpsc::UnboundedReceiver<TimerCmd>,
    quit            : bool,
}

impl Verticle {
    fn new(
        options: VerticleOptions,
        event_rx: mpsc::UnboundedReceiver<Event>
    ) -> Result<Self> {
        let (tmr_tx, tmr_rx) = mpsc::unbounded_channel::<TimerCmd>();
        let timer_client = TimerClient::new(tmr_tx);
        let timer_manager = TimerManager::new();
        Ok(Self {
            session: ProxySession::new(options, timer_client)?,
            timer_manager,
            event_rx,
            tmr_rx,
            quit: false,
        })
    }

    fn handle_event(&mut self, event: Event) {
        match event {
            Event::Start { complete } => {
                task::spawn_local({
                    let session = self.session.clone();
                    async move {
                        let result = session.start().await;
                        let _ = complete.send(
                            result.map_err(|e| e.to_string())
                        );
                    }
                });
            }
            Event::Stop { complete } => {
                self.quit = true;
                self.timer_manager.stop_all();
                let _ = complete.send(Ok(()));
            }
        }
    }

    fn handle_command(&mut self, cmd: TimerCmd) {
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
                Some(event) = self.event_rx.recv() => {
                    self.handle_event(event);
                }
                Some(cmd) = self.tmr_rx.recv() => {
                    self.handle_command(cmd);
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

pub(crate) fn deploy(options: VerticleOptions) -> Result<VerticleClient> {
    let (event_tx, event_rx) = mpsc::unbounded_channel::<Event>();
    let (reply_tx, reply_rx) = std_mpsc::sync_channel::<StdResult<(), String>>(1);

    let handle = std::thread::spawn(move || {
        let rt = runtime::Builder::new_current_thread()
            .enable_time()
            .enable_io()
            .build()
            .expect("ActiveProxy runtime verticle should be built");

        let local = task::LocalSet::new();
        rt.block_on(local.run_until(async move {
            match Verticle::new(options, event_rx) {
                Ok(mut v) => {
                    let _ = reply_tx.send(Ok(()));
                    v.run_loop().await;
                },
                Err(e) => {
                    let _ = reply_tx.send(Err(e.to_string()));
                }
            }
        }));
    });

    match reply_rx.recv() {
        Ok(Ok(())) => Ok(VerticleClient::new(event_tx, handle)),
        Ok(Err(msg)) => Err(StateError::new(msg)),
        Err(_) => Err(StateError::new("ActiveProxy verticle startup channel closed")),
    }
}
