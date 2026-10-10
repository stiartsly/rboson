
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering}
};
use crate::messaging::ConnectionListener;

pub(crate) struct ClientConnectionListener {
    listener: Arc<dyn ConnectionListener>,
    connected: Arc<AtomicBool>,
    ready: Arc<AtomicBool>,
}

impl ClientConnectionListener {
    pub(crate) fn new(
        listener: Arc<dyn ConnectionListener>,
        connected: Arc<AtomicBool>,
        ready: Arc<AtomicBool>,
    ) -> Self {
        Self {
            listener,
            connected,
            ready,
        }
    }
}

impl ConnectionListener for ClientConnectionListener {
    fn on_connecting(&self) {
        let listener = self.listener.clone();
        tokio::spawn(async move {
            listener.on_connecting();
        });
    }
    fn on_connected(&self) {
        self.connected.store(true, Ordering::Release);
        let listener = self.listener.clone();

        tokio::spawn(async move {
            listener.on_connected();
        });
    }
    fn on_ready(&self) {
        self.ready.store(true, Ordering::Release);
        let listener = self.listener.clone();

        tokio::spawn(async move {
            listener.on_ready();
        });
    }
    fn on_disconnected(&self) {
        self.connected.store(false, Ordering::Release);
        self.ready.store(false, Ordering::Release);
        let listener = self.listener.clone();

        tokio::spawn(async move {
            listener.on_disconnected();
        });
    }
}