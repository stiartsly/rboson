use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use crate::ConnectionListener;

/// Forwards lifecycle callbacks to the user listener and updates client state.
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

    /// Runs a user callback on a task so a slow listener cannot block the
    /// transport thread.
    fn notify(&self, callback: impl FnOnce(&dyn ConnectionListener) + Send + 'static) {
        let listener = Arc::clone(&self.listener);
        tokio::spawn(async move {
            callback(listener.as_ref());
        });
    }
}

impl ConnectionListener for ClientConnectionListener {
    fn on_connecting(&self) {
        self.notify(|listener| listener.on_connecting());
    }

    fn on_connected(&self) {
        self.connected.store(true, Ordering::Release);
        self.notify(|listener| listener.on_connected());
    }

    fn on_ready(&self) {
        self.ready.store(true, Ordering::Release);
        self.notify(|listener| listener.on_ready());
    }

    fn on_disconnected(&self) {
        self.connected.store(false, Ordering::Release);
        self.ready.store(false, Ordering::Release);
        self.notify(|listener| listener.on_disconnected());
    }
}
