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

use futures::{stream::FuturesUnordered, FutureExt, StreamExt};
use log::debug;
use std::{
    future::Future,
    net::SocketAddr,
    pin::Pin,
    rc::Rc,
    result::Result as StdResult,
    thread::JoinHandle,
};
use tokio::{
    runtime,
    sync::{mpsc, oneshot},
    task,
};

type Pending = FuturesUnordered<Pin<Box<dyn Future<Output = ()>>>>;

use super::{
    options::AnnouncePeerHandler,
    session::ProxySession,
    LocalBoxTimerClient as TimerClient,
    LocalBoxTimerCmd as TimerCmd,
    LocalBoxTimerManager as TimerManager,
};
use boson_api::{
    errors::{Result, StateError},
    signature, Id,
};

pub(crate) struct VerticleClient {
    event_tx: mpsc::UnboundedSender<Event>,
    handle: Option<JoinHandle<()>>,
}

enum Event {
    Start {
        complete: oneshot::Sender<StdResult<(), String>>,
    },
    Stop {
        complete: oneshot::Sender<StdResult<(), String>>,
    },
}

impl VerticleClient {
    fn new(event_tx: mpsc::UnboundedSender<Event>, handle: JoinHandle<()>) -> Self {
        Self {
            event_tx,
            handle: Some(handle),
        }
    }

    pub(crate) async fn start(&self) -> Result<()> {
        debug!("Starting ActiveProxy verticle");

        let (tx, rx) = oneshot::channel();
        self.event_tx
            .send(Event::Start { complete: tx })
            .map_err(|_| StateError::new("ActiveProxy verticle event channel closed"))?;
        rx.await
            .map_err(|_| StateError::new("ActiveProxy verticle startup channel closed"))?
            .map_err(StateError::new)?;

        debug!("ActiveProxy verticle started");
        Ok(())
    }

    pub(crate) async fn stop(&mut self) -> Result<()> {
        debug!("Stopping ActiveProxy verticle");

        // The ack arrives only after the session is torn down, so the join below is immediate.
        let (tx, rx) = oneshot::channel();
        if self.event_tx.send(Event::Stop { complete: tx }).is_ok() {
            let _ = rx.await;
        }
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }

        debug!("ActiveProxy verticle stopped");
        Ok(())
    }
}

pub(crate) struct VerticleOptions {
    pub(super) service_peerid: Id,
    pub(super) service_endpoint: String,
    pub(super) upstream_addr: SocketAddr,
    pub(super) upstream_endpoint: String,
    pub(super) user_id: Id,
    pub(super) device_key: signature::PrivateKey,
    pub(super) name_access_enabled: bool,
    pub(super) announce_peer_enabled: bool,
    pub(super) announce_peer_handler: Option<AnnouncePeerHandler>,
}

impl VerticleOptions {
    pub(crate) fn new(
        service_peerid: Id,
        service_endpoint: String,
        upstream_addr: SocketAddr,
        upstream_endpoint: String,
        user_id: Id,
        device_key: signature::PrivateKey,
        name_access_enabled: bool,
        announce_peer_enabled: bool,
        announce_peer_handler: Option<AnnouncePeerHandler>,
    ) -> Self {
        Self {
            service_peerid,
            service_endpoint,
            upstream_addr,
            upstream_endpoint,
            user_id,
            device_key,
            name_access_enabled,
            announce_peer_enabled,
            announce_peer_handler,
        }
    }
}

pub(crate) struct Verticle {
    session: Rc<ProxySession>,
    timer_manager: TimerManager,
    event_rx: mpsc::UnboundedReceiver<Event>,
    tmr_rx: mpsc::UnboundedReceiver<TimerCmd>,
    stop_ack: Option<oneshot::Sender<StdResult<(), String>>>,
    quit: bool,
}

impl Verticle {
    fn new(options: VerticleOptions, event_rx: mpsc::UnboundedReceiver<Event>) -> Result<Self> {
        let (tmr_tx, tmr_rx) = mpsc::unbounded_channel::<TimerCmd>();
        let timer_client = TimerClient::new(tmr_tx);

        Ok(Self {
            session: ProxySession::new(options, timer_client)?,
            timer_manager: TimerManager::new(),
            event_rx,
            tmr_rx,
            stop_ack: None,
            quit: false,
        })
    }

    fn handle_events(&mut self, event: Event, pending: &mut Pending) {
        match event {
            Event::Start { complete } => {
                let session = self.session.clone();
                pending.push(
                    async move {
                        let result = session.start().await;
                        let _ = complete.send(result.map_err(|e| e.to_string()));
                    }
                    .boxed_local(),
                );
            }
            Event::Stop { complete } => {
                self.quit = true;
                self.timer_manager.stop_all();
                self.stop_ack = Some(complete);
            }
        }
    }

    fn handle_command(&mut self, cmd: TimerCmd) {
        match cmd {
            TimerCmd::Add {
                timer_id,
                delay,
                interval,
                cb,
            } => self.timer_manager.add_timer(timer_id, delay, interval, cb),

            TimerCmd::Cancel { timer_id } => self.timer_manager.cancel_timer(timer_id),

            TimerCmd::Stop { complete } => {
                self.timer_manager.stop_all();
                let _ = complete.send(());
            }
        }
    }

    async fn run_loop(&mut self) {
        let mut pendings = Pending::new();
        loop {
            // Stop only once every event already in flight has settled.
            if self.quit && pendings.is_empty() {
                break;
            }

            tokio::select! {
                event = self.event_rx.recv(), if !self.quit => {
                    match event {
                        Some(event) => self.handle_events(event, &mut pendings),
                        // Every sender is gone: drain in-flight work, then shut down.
                        None => self.quit = true,
                    }
                }
                Some(()) = pendings.next(), if !pendings.is_empty() => {}
                Some(cmd) = self.tmr_rx.recv() => {
                    self.handle_command(cmd);
                }
                Some(timer_id) = self.timer_manager.next_expired(), if !self.timer_manager.is_idle() => {
                    self.timer_manager.fire_expired(timer_id).await;
                }
            }
        }

        self.session.stop().await;
        self.session.close();
        if let Some(ack) = self.stop_ack.take() {
            let _ = ack.send(Ok(()));
        }
    }
}

// Runs the verticle on its own single-threaded runtime so its non-`Send` state stays on one
// thread and the caller's executor is never blocked by session I/O.
pub(crate) async fn deploy(options: VerticleOptions) -> Result<VerticleClient> {
    let (event_tx, event_rx) = mpsc::unbounded_channel::<Event>();
    let (ready_tx, ready_rx) = oneshot::channel::<StdResult<(), String>>();

    let handle = std::thread::spawn(move || {
        let rt = match runtime::Builder::new_current_thread()
            .enable_time()
            .enable_io()
            .build()
        {
            Ok(rt) => rt,
            Err(e) => {
                let _ = ready_tx.send(Err(format!("Failed to build ActiveProxy runtime: {e}")));
                return;
            }
        };

        let local = task::LocalSet::new();
        rt.block_on(local.run_until(async move {
            match Verticle::new(options, event_rx).and_then(|v| v.session.init().map(|_| v)) {
                Ok(mut v) => {
                    let _ = ready_tx.send(Ok(()));
                    v.run_loop().await;
                }
                Err(e) => {
                    let _ = ready_tx.send(Err(e.to_string()));
                }
            }
        }));
    });

    match ready_rx.await {
        Ok(Ok(())) => Ok(VerticleClient::new(event_tx, handle)),
        Ok(Err(msg)) => {
            let _ = handle.join();
            Err(StateError::new(msg))
        }
        Err(_) => Err(StateError::new(
            "ActiveProxy verticle startup channel closed",
        )),
    }
}
