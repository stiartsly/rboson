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
    time::{
        self,
        Duration,
        MissedTickBehavior,
    },
};

use boson_api::errors::Result;

use crate::internal::transport::RpcServer;

use super::{
    commands,
    event::Event,
    VerticleOptions,
};

const MAINTENANCE_INTERVAL: Duration = Duration::from_secs(30);

pub(super) struct Verticle {
    rpc_server: Rc<RpcServer>,
    event_rx: mpsc::UnboundedReceiver<Event>,
}

impl Verticle {
    pub(super) fn new(
        options: VerticleOptions,
        event_rx: mpsc::UnboundedReceiver<Event>,
    ) -> Result<Self> {
        Ok(Self {
            rpc_server      : RpcServer::new(options)?,
            event_rx,
        })
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

            let rpc = self.rpc_server.clone();
            let event_rx = &mut self.event_rx;

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
                }
                event = rpc.poll_transport(), if !quit && rpc.is_running() => {
                    rpc.handle_transport_event(event, &mut pending);
                }
                Some(()) = pending.next(), if !pending.is_empty() => {
                }
                _ = maintenance.tick() => {
                    if rpc.is_running() {
                        rpc.maintenance();
                    }
                }
                else => break,
            }
        }

        if let Err(error) = self.rpc_server.stop().await {
            error!("Stopping messaging session failed: {error}");
        }
    }
}
