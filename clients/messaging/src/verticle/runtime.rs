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

use std::rc::Rc;

use futures::stream::StreamExt;
use log::error;
use tokio::{
    sync::mpsc,
    task,
    time::{self, Duration, MissedTickBehavior},
};

use boson_api::errors::Result;

use crate::rpc_server::RpcServer;

use super::{commands, event::Event, transport, VerticleOptions};

const MAINTENANCE_INTERVAL: Duration = Duration::from_secs(30);

/// The single-owner container of the messaging engine.
///
/// It lives inside one current-thread runtime on one physical thread. Commands,
/// timer/maintenance work and MQTT delivery are all tasks/futures on that
/// thread; no work is moved to a second worker thread.
pub(super) struct Verticle {
    rpc_server: Rc<RpcServer>,
    event_rx: mpsc::UnboundedReceiver<Event>,
    transport_rx: Option<mpsc::UnboundedReceiver<transport::Delivery>>,
    transport_task: Option<task::JoinHandle<()>>,
}

impl Verticle {
    pub(super) fn new(
        options: VerticleOptions,
        event_rx: mpsc::UnboundedReceiver<Event>,
    ) -> Result<Self> {
        let rpc_server = RpcServer::new(options)?;
        Ok(Self {
            rpc_server,
            event_rx,
            transport_rx: None,
            transport_task: None,
        })
    }

    /// Starts the MQTT pump once the session is running.
    ///
    /// The pump owns the only loop around `EventLoop::poll`, so stopping a
    /// command or timer never cancels a partially polled transport.
    fn ensure_transport_pump(&mut self) {
        if self.transport_task.is_some() || !self.rpc_server.is_running() {
            return;
        }

        let (tx, rx) = mpsc::unbounded_channel();
        self.transport_rx = Some(rx);
        let rpc = self.rpc_server.clone();
        self.transport_task = Some(task::spawn_local(transport::run(rpc, tx)));
    }

    pub(super) async fn run_loop(&mut self) {
        let mut pending = commands::Pending::new();
        let mut quit = false;
        // Periodic local reclamation. This is the verticle's timer source; it
        // runs on the same thread and never blocks the command/transport paths.
        let mut maintenance = time::interval(MAINTENANCE_INTERVAL);
        maintenance.set_missed_tick_behavior(MissedTickBehavior::Skip);

        loop {
            // Stop only after every command already accepted has settled.
            if quit && pending.is_empty() {
                break;
            }

            if !quit {
                self.rpc_server.schedule_contact_sync(&mut pending);
            }
            self.ensure_transport_pump();

            let rpc = self.rpc_server.clone();
            let event_rx = &mut self.event_rx;
            let transport_rx = &mut self.transport_rx;
            let transport_closed;

            tokio::select! {
                event = event_rx.recv(), if !quit => {
                    match event {
                        Some(event) => {
                            if commands::handle(event, &rpc, &mut pending) {
                                quit = true;
                            }
                        }
                        // Every client handle is gone: drain in-flight work,
                        // then stop the session exactly once below.
                        None => quit = true,
                    }
                    transport_closed = false;
                }
                delivery = async {
                    match transport_rx.as_mut() {
                        Some(rx) => rx.recv().await,
                        None => std::future::pending().await,
                    }
                } => {
                    match delivery {
                        Some((event, ack)) => {
                            rpc.handle_mqtt_event(event, &mut pending);
                            // The event loop lock is free again; let the pump
                            // poll the next packet. Credential refreshes happen
                            // in handle_mqtt_event before this point.
                            let _ = ack.send(());
                            transport_closed = false;
                        }
                        None => transport_closed = true,
                    }
                }
                Some(()) = pending.next(), if !pending.is_empty() => {
                    transport_closed = false;
                }
                _ = maintenance.tick() => {
                    if rpc.is_running() {
                        rpc.maintenance();
                    }
                    transport_closed = false;
                }
                else => break,
            }

            if transport_closed && !quit {
                error!("Messaging transport pump stopped unexpectedly");
                quit = true;
            }
        }

        if let Some(handle) = self.transport_task.take() {
            handle.abort();
        }
        if let Err(error) = self.rpc_server.stop().await {
            error!("Stopping messaging session failed: {error}");
        }
    }
}

#[cfg(test)]
#[path = "../unitests/test_runtime.rs"]
mod test_runtime;


