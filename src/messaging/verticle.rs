use rumqttc::{
    AsyncClient, Event, Incoming, MqttOptions, Publish, QoS, SubscribeFilter, Transport,
};
use std::{
    cell::RefCell,
    rc::Rc,
    result::Result as StdResult,
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc as std_mpsc, Arc,
    },
    thread::JoinHandle,
    time::Duration,
};
use tokio::{
    runtime,
    sync::{mpsc, oneshot},
    task,
};

use crate::messaging::{
    client::SharedListeners,
    errors::{Error, Result},
    options::Options,
    ConnectionListener,
};
use crate::Id;

const USER_INBOX: &str = "u/i";
const USER_OUTBOX: &str = "u/o";
const DEVICE_INBOX: &str = "d/i";
const MAX_MESSAGE_SIZE: usize = 256 * 1024;

/// Session instance holding all necessary information to interact with the MQTT server.
/// All fields are defined with RefCell since they are exclusively referenced within a
/// single dedicated thread running in a LocalSet.
pub(crate) struct Session {
    options: RefCell<Options>,
    user_id: RefCell<Id>,
    device_id: RefCell<Id>,
    connected: RefCell<bool>,
    ready: RefCell<bool>,
    running: RefCell<bool>,
    mqtt: RefCell<Option<AsyncClient>>,
    listeners: Arc<SharedListeners>,
}

impl Session {
    pub(crate) fn new(options: Options, listeners: Arc<SharedListeners>) -> Result<Self> {
        let user_id = *options.user_id();
        let device_id = *options.device_id();
        Ok(Self {
            options: RefCell::new(options),
            user_id: RefCell::new(user_id),
            device_id: RefCell::new(device_id),
            connected: RefCell::new(false),
            ready: RefCell::new(false),
            running: RefCell::new(false),
            mqtt: RefCell::new(None),
            listeners,
        })
    }

    fn password(&self) -> Result<String> {
        let nonce = crate::cryptobox::Nonce::random();
        let options = self.options.borrow();
        let user_key = options.user_key();
        let device_key = options.device_key();

        let usign = user_key
            .private_key()
            .sign_into(nonce.as_bytes())
            .map_err(|error| Error::Auth(error.to_string()))?;
        let dsign = device_key
            .private_key()
            .sign_into(nonce.as_bytes())
            .map_err(|error| Error::Auth(error.to_string()))?;

        let mut password = Vec::with_capacity(nonce.size() + usign.len() + dsign.len());
        password.extend_from_slice(nonce.as_bytes());
        password.extend_from_slice(&usign);
        password.extend_from_slice(&dsign);

        Ok(bs58::encode(password).into_string())
    }

    fn notify_connection(&self, callback: impl Fn(&dyn ConnectionListener)) {
        let listeners = self.listeners.connection_listeners.read().unwrap().clone();
        for listener in listeners {
            callback(listener.as_ref());
        }
    }

    pub(crate) async fn start(self: &Rc<Self>) -> Result<()> {
        if *self.running.borrow() {
            return Ok(());
        }

        let endpoint = self
            .options
            .borrow()
            .service_endpoint()
            .cloned()
            .ok_or_else(|| {
                Error::State(
                    "service.endpoint is required: DHT service discovery is not yet wired \
                 into the updated Rust messaging Options"
                        .into(),
                )
            })?;
        tokio::fs::create_dir_all(self.options.borrow().data_dir()).await?;

        let host = endpoint
            .host_str()
            .ok_or_else(|| Error::Argument("service endpoint has no hostname".into()))?;
        let port = endpoint
            .port()
            .ok_or_else(|| Error::Argument("service endpoint has no port".into()))?;

        self.notify_connection(|listener| listener.on_connecting());

        let client_id =
            bs58::encode(md5::compute(self.device_id.borrow().as_bytes()).0).into_string();
        let mut mqtt_opts = MqttOptions::new(client_id, host.to_string(), port);
        mqtt_opts.set_credentials(self.user_id.borrow().to_string(), self.password()?);
        mqtt_opts.set_keep_alive(Duration::from_secs(60));
        mqtt_opts.set_clean_session(false);
        mqtt_opts.set_max_packet_size(MAX_MESSAGE_SIZE, MAX_MESSAGE_SIZE);
        if endpoint.scheme() == "mqtts" || endpoint.scheme() == "ssl" {
            mqtt_opts.set_transport(Transport::tls_with_default_config());
        }

        let (mqtt, mut eventloop) = AsyncClient::new(mqtt_opts, 32);
        let userid = self.user_id.borrow().to_string();
        mqtt.subscribe_many([
            SubscribeFilter::new(format!("inbox/{userid}"), QoS::AtLeastOnce),
            SubscribeFilter::new(format!("outbox/{userid}"), QoS::AtLeastOnce),
            SubscribeFilter::new("broadcast".to_string(), QoS::AtLeastOnce),
            SubscribeFilter::new(USER_INBOX.to_string(), QoS::AtLeastOnce),
            SubscribeFilter::new(USER_OUTBOX.to_string(), QoS::AtLeastOnce),
            SubscribeFilter::new(DEVICE_INBOX.to_string(), QoS::AtLeastOnce),
        ])
        .await
        .map_err(|error| Error::Io(std::io::Error::other(error)))?;

        *self.mqtt.borrow_mut() = Some(mqtt);
        self.running.replace(true);

        let session = self.clone();
        task::spawn_local(async move {
            while *session.running.borrow() {
                match eventloop.poll().await {
                    Ok(Event::Incoming(Incoming::ConnAck(connack))) => {
                        if connack.code == rumqttc::ConnectReturnCode::Success {
                            if !session.connected.replace(true) {
                                session.listeners.connected.store(true, Ordering::Release);
                                session.notify_connection(|l| l.on_connected());
                            }
                        } else {
                            log::warn!("Messaging MQTT ConnAck error code: {:?}", connack.code);
                        }
                    }
                    Ok(Event::Incoming(Incoming::SubAck(_))) => {
                        if !session.connected.replace(true) {
                            session.listeners.connected.store(true, Ordering::Release);
                            session.notify_connection(|l| l.on_connected());
                        }
                        if !session.ready.replace(true) {
                            session.listeners.ready.store(true, Ordering::Release);
                            session.notify_connection(|l| l.on_ready());
                        }
                    }
                    Ok(Event::Incoming(Incoming::Publish(publish))) => {
                        let s = session.clone();
                        task::spawn_local(async move {
                            s.handle_publish(publish).await;
                        });
                    }
                    Ok(_) => {}
                    Err(error) => {
                        if session.connected.replace(false) {
                            session.ready.replace(false);
                            session.listeners.connected.store(false, Ordering::Release);
                            session.listeners.ready.store(false, Ordering::Release);
                            session.notify_connection(|l| l.on_disconnected());
                        }
                        if *session.running.borrow() {
                            log::warn!("Messaging MQTT connection error: {error}");
                            tokio::time::sleep(Duration::from_secs(2)).await;
                        }
                    }
                }
            }
            if session.connected.replace(false) {
                session.listeners.connected.store(false, Ordering::Release);
                session.notify_connection(|l| l.on_disconnected());
            }
            session.ready.replace(false);
            session.listeners.ready.store(false, Ordering::Release);
        });

        Ok(())
    }

    async fn handle_publish(&self, _publish: Publish) {
        // Dedicated task to process incoming MQTT publish messages
    }

    pub(crate) async fn stop(&self) {
        if !self.running.replace(false) {
            return;
        }

        let mqtt = self.mqtt.borrow_mut().take();
        if let Some(mqtt) = mqtt {
            let _ = mqtt.disconnect().await;
        }

        self.connected.replace(false);
        self.ready.replace(false);
        self.listeners.connected.store(false, Ordering::Release);
        self.listeners.ready.store(false, Ordering::Release);
    }
}

pub(crate) struct VerticleClient {
    event_tx: mpsc::UnboundedSender<VerticleEvent>,
    handle: Option<JoinHandle<()>>,
    running: Arc<AtomicBool>,
}

enum VerticleEvent {
    Start {
        complete: oneshot::Sender<StdResult<(), String>>,
    },
    Stop {
        complete: oneshot::Sender<StdResult<(), String>>,
    },
}

impl VerticleClient {
    fn new(
        event_tx: mpsc::UnboundedSender<VerticleEvent>,
        handle: JoinHandle<()>,
        running: Arc<AtomicBool>,
    ) -> Self {
        Self {
            event_tx,
            handle: Some(handle),
            running,
        }
    }

    pub(crate) async fn start(&self) -> Result<()> {
        let (tx, rx) = oneshot::channel();
        self.event_tx
            .send(VerticleEvent::Start { complete: tx })
            .map_err(|_| Error::State("Messaging verticle event channel closed".into()))?;
        rx.await
            .map_err(|_| Error::State("Messaging verticle startup channel closed".into()))?
            .map_err(Error::State)?;
        Ok(())
    }

    pub(crate) async fn stop(&mut self) -> Result<()> {
        let (tx, rx) = oneshot::channel();
        if self
            .event_tx
            .send(VerticleEvent::Stop { complete: tx })
            .is_ok()
        {
            let _ = rx.await;
        }
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
        Ok(())
    }

    pub(crate) fn is_running(&self) -> bool {
        self.running.load(Ordering::Acquire)
    }
}

pub(crate) struct Verticle {
    session: Rc<Session>,
    event_rx: mpsc::UnboundedReceiver<VerticleEvent>,
    running_flag: Arc<AtomicBool>,
    quit: bool,
}

impl Verticle {
    fn new(
        options: Options,
        listeners: Arc<SharedListeners>,
        event_rx: mpsc::UnboundedReceiver<VerticleEvent>,
        running_flag: Arc<AtomicBool>,
    ) -> Result<Self> {
        let session = Rc::new(Session::new(options, listeners)?);
        Ok(Self {
            session,
            event_rx,
            running_flag,
            quit: false,
        })
    }

    fn handle_event(&mut self, event: VerticleEvent) {
        match event {
            VerticleEvent::Start { complete } => {
                let session = self.session.clone();
                let running_flag = self.running_flag.clone();
                task::spawn_local(async move {
                    let result = session.start().await;
                    if result.is_ok() {
                        running_flag.store(true, Ordering::Release);
                    }
                    let _ = complete.send(result.map_err(|e| e.to_string()));
                });
            }
            VerticleEvent::Stop { complete } => {
                self.quit = true;
                let session = self.session.clone();
                let running_flag = self.running_flag.clone();
                task::spawn_local(async move {
                    session.stop().await;
                    running_flag.store(false, Ordering::Release);
                    let _ = complete.send(Ok(()));
                });
            }
        }
    }

    async fn run_loop(&mut self) {
        while let Some(event) = self.event_rx.recv().await {
            self.handle_event(event);
            if self.quit {
                break;
            }
        }
    }
}

pub(crate) fn deploy(options: Options, listeners: Arc<SharedListeners>) -> Result<VerticleClient> {
    let (event_tx, event_rx) = mpsc::unbounded_channel::<VerticleEvent>();
    let (reply_tx, reply_rx) = std_mpsc::sync_channel::<StdResult<(), String>>(1);
    let running_flag = Arc::new(AtomicBool::new(false));
    let running_clone = running_flag.clone();

    let handle = std::thread::spawn(move || {
        let rt = runtime::Builder::new_current_thread()
            .enable_time()
            .enable_io()
            .build()
            .expect("Messaging runtime verticle should be built");

        let local = task::LocalSet::new();
        rt.block_on(local.run_until(async move {
            match Verticle::new(options, listeners, event_rx, running_clone) {
                Ok(mut v) => {
                    let _ = reply_tx.send(Ok(()));
                    v.run_loop().await;
                }
                Err(e) => {
                    let _ = reply_tx.send(Err(e.to_string()));
                }
            }
        }));
    });

    match reply_rx.recv() {
        Ok(Ok(())) => Ok(VerticleClient::new(event_tx, handle, running_flag)),
        Ok(Err(msg)) => Err(Error::State(msg)),
        Err(_) => Err(Error::State(
            "Messaging verticle startup channel closed".into(),
        )),
    }
}
