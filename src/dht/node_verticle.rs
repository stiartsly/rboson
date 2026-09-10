use std::{
    sync::Mutex,
    thread::JoinHandle
};
use tokio::{
    task,
    runtime,
    sync::mpsc::{self, UnboundedSender}
};
use crate::{
    errors::Result,
    BoxHandler,
    BoxTimerCmd as TimerCmd,
    BoxTimerClient as TimerClient,
    BoxTimerManager as TimerManager
};

pub(crate) type TimerId = u64;

pub(crate) struct VerticleClient {
    timerc: TimerClient,
    handle: Mutex<Option<JoinHandle<()>>>,
}

impl VerticleClient {
    pub(crate) fn new(
        sender: UnboundedSender<TimerCmd>,
        handle: JoinHandle<()>
    ) -> Self {
        Self {
            timerc: TimerClient::new(sender),
            handle: Mutex::new(Some(handle)),
        }
    }

    pub(crate) fn add_timer(&self,
        delay: u64,
        interval: Option<u64>,
        cb: BoxHandler<()>
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
    pub(crate) fn new(
        receiver: mpsc::UnboundedReceiver<TimerCmd>
    ) -> Self {
        Self {
            timerman: TimerManager::new(),
            receiver,
        }
    }

    fn handle_commands(&mut self, cmd: TimerCmd) -> bool {
        match cmd {
            TimerCmd::Add { timer_id, delay, interval, cb } => {
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

pub(crate) fn deploy(_option: VerticleOptions) -> Result<VerticleClient> {
    let (sender, receiver) = mpsc::unbounded_channel::<TimerCmd>();
    let handle = std::thread::spawn(move || {
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
    });
    Ok(VerticleClient::new(sender, handle))
}
