use std::thread::JoinHandle;
use tokio::{
    runtime,
    sync::mpsc::{self, UnboundedSender}
};
use crate::errors::Result;
use crate::{
    BoxHandler,
    BoxTimerManager as TimerManager,
    BoxTimerClient as TimerClient,
    BoxTimerCmd as TimerCmd,
};

pub(crate) type TimerId = u64;

pub(crate) struct VerticleClient {
    timer_client: TimerClient,
    handle      : Option<JoinHandle<()>>,
}

impl VerticleClient {
    pub(crate) fn new(
        sender: UnboundedSender<TimerCmd>,
        handle: JoinHandle<()>
    ) -> Self {
        Self {
            timer_client: TimerClient::new(sender),
            handle: Some(handle),
        }
    }

    pub(crate) fn add_timer(&self,
        delay: u64,
        interval: Option<u64>,
        cb: BoxHandler<()>
    ) -> Result<TimerId> {
        self.timer_client.add_timer(delay, interval, cb)
    }

    pub(crate) async fn stop(&mut self) -> Result<()> {
        self.timer_client.stop_timers().await?;

        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
        Ok(())
    }
}

pub(crate) struct Verticle {
    timer_manager: TimerManager,
    receiver: mpsc::UnboundedReceiver<TimerCmd>,

}

impl Verticle {
    pub(crate) fn new(
        receiver: mpsc::UnboundedReceiver<TimerCmd>
    ) -> Self {
        Self {
            timer_manager: TimerManager::new(),
            receiver,
        }
    }

    fn handle_timer_cmd(&mut self, cmd: TimerCmd) -> bool {
        match cmd {
            TimerCmd::Add { timer_id, delay, interval, cb } => {
                self.timer_manager.add_timer(timer_id, delay, interval, cb);
            }
            TimerCmd::Cancel { timer_id } => {
                self.timer_manager.cancel_timer(timer_id);
            }
            TimerCmd::Stop { complete } => {
                self.timer_manager.stop_all();
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
                            if self.handle_timer_cmd(cmd) {
                                break;
                            }
                        }
                        None => break,
                    }
                }

                Some(timer_id) = self.timer_manager.next_expired(), if !self.timer_manager.is_idle() => {
                    self.timer_manager.fire_expired(timer_id).await;
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

        let local = tokio::task::LocalSet::new();
        rt.block_on(local.run_until(async move {
            let mut vert = Verticle::new(receiver);
            vert.run_loop().await;
        }));
    });
    Ok(VerticleClient::new(sender, handle))
}
