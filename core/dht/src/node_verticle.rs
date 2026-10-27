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


use std::{sync::Mutex, thread};
use tokio::{
    runtime,
    sync::mpsc::{self, UnboundedSender},
    task,
};
use boson_api::{
    errors::Result,
    BoxHandler,
    BoxTimerClient as TimerClient,
    BoxTimerCmd as TimerCmd,
    BoxTimerManager as TimerManager,
};

pub(crate) type TimerId = u64;

pub(crate) struct VerticleClient {
    timerc: TimerClient,
    handle: Mutex<Option<thread::JoinHandle<()>>>,
}

impl VerticleClient {
    pub(crate) fn new(
        sender: UnboundedSender<TimerCmd>,
        handle: thread::JoinHandle<()>
    ) -> Self {
        Self {
            timerc: TimerClient::new(sender),
            handle: Mutex::new(Some(handle)),
        }
    }

    pub(crate) fn add_timer(
        &self,
        delay: u64,
        interval: Option<u64>,
        cb: BoxHandler<()>,
    ) -> Result<TimerId> {
        self.timerc.add_timer(delay, interval, cb)
    }

    pub(crate) async fn stop(&self) -> Result<()> {
        self.timerc.stop_timers().await?;

        let handle = self.handle.lock().unwrap().take();
        if let Some(h) = handle {
            let _ = h.join();
        }
        Ok(())
    }
}

pub(crate) struct Verticle {
    timerman: TimerManager,
    receiver: mpsc::UnboundedReceiver<TimerCmd>,
}

impl Verticle {
    pub(crate) fn new(receiver: mpsc::UnboundedReceiver<TimerCmd>) -> Self {
        Self {
            timerman: TimerManager::new(),
            receiver,
        }
    }

    fn handle_commands(&mut self, cmd: TimerCmd) -> bool {
        match cmd {
            TimerCmd::Add {timer_id, delay, interval, cb }=> {
                self.timerman.add_timer(timer_id, delay, interval, cb);
            }
            TimerCmd::Cancel { timer_id } => {
                self.timerman.cancel_timer(timer_id);
            }
            TimerCmd::Stop { complete } => {
                self.timerman.stop_all();
                let _ = complete.send(());
                return true;
            }
        }
        false
    }

    async fn run_loop(&mut self) {
        loop {
            tokio::select! {
                cmd = self.receiver.recv() => {
                    match cmd {
                        Some(cmd) => {
                            if self.handle_commands(cmd) {
                                break;
                            }
                        }
                        None => break,
                    }
                }

                Some(timer_id) = self.timerman.next_expired(), if !self.timerman.is_idle() => {
                    self.timerman.fire_expired(timer_id).await;
                }
            }
        }
    }
}

#[derive(Default)]
pub(crate) struct VerticleOptions {}

fn run(receiver: mpsc::UnboundedReceiver<TimerCmd>) {
    let rt = runtime::Builder::new_current_thread()
        .enable_time()
        .enable_io()
        .build()
        .expect("dht verticle runtime should build");

    let local = task::LocalSet::new();
    rt.block_on(local.run_until(async move {
        let mut vert = Verticle::new(receiver);
        vert.run_loop().await;
    }));
}

pub(crate) fn deploy(_option: VerticleOptions) -> Result<VerticleClient> {
    let (sender, receiver) = mpsc::unbounded_channel::<TimerCmd>();
    let handle = thread::spawn(move || run(receiver));
    Ok(VerticleClient::new(sender, handle))
}
