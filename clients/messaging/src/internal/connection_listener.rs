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
