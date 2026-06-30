use tokio::{
    runtime,
    task::{self, JoinHandle},
    sync::mpsc::{self, UnboundedSender}
};
use crate::errors::Result;
use crate::dht::{
    timer_manager::TimerManager,
    timer_client::{TimerClient, TimerCmd},
    handler::AsyncHandler,
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
        cb: AsyncHandler<()>
    ) -> Result<TimerId> {
        self.timer_client.add_timer(delay, interval, cb)
    }

    pub(crate) async fn stop(&mut self) -> Result<()> {
        self.timer_client.stop().await;

        if let Some(handle) = self.handle.take() {
            let _ = handle.await;
        }
        Ok(())
    }
}

pub(crate) struct Verticle {
    timer_manager: TimerManager,
    receiver: mpsc::UnboundedReceiver<TimerCmd>,

    quit        : bool,
}

impl Verticle {
    pub(crate) fn new(
        _options: VerticleOptions,
        receiver: mpsc::UnboundedReceiver<TimerCmd>
    ) -> Self {
        Self {
            timer_manager: TimerManager::new(),
            receiver,
            quit: false,
        }
    }

    fn handle_timer_cmd(&mut self, cmd: TimerCmd) {
        match cmd {
            TimerCmd::Add { timer_id, delay, interval, cb } =>
                self.timer_manager.add_timer(timer_id, delay, interval, cb),
            TimerCmd::Cancel { id: timer_id } =>
                self.timer_manager.cancel_timer(timer_id),
            TimerCmd::Stop { complete } => {
                self.quit = true;
                self.timer_manager.stop_all();
                let _ = complete.send(());
            }
        }
    }

    async fn run_loop(&mut self) {
        loop {
            tokio::select! {
                Some(timer_cmd) = self.receiver.recv() => {
                    self.handle_timer_cmd(timer_cmd);
                }

                Some(timer_id) = self.timer_manager.next_expired(), if !self.timer_manager.is_idle() => {
                    self.timer_manager.fire_expired(timer_id).await;
                }
            }

            if self.quit {
                break;
            }
        }
    }
}

pub(crate) struct VerticleOptions {}
pub(crate) fn deploy(option: VerticleOptions) -> Result<VerticleClient> {
    let (sender, receiver) = mpsc::unbounded_channel::<TimerCmd>();
    let handle = task::spawn_blocking(move || {
        runtime::Builder::new_current_thread()
            .enable_time()
            .build().expect("timer runtime should build")
            .block_on(async move {
                let mut verticle = Verticle::new(option, receiver);
                verticle.run_loop().await;
            });
    });
    Ok(VerticleClient::new(sender, handle))
}
