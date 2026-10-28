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

mod client;
mod commands;
mod event;
mod runtime;
mod transport;

use std::{
    result::Result as StdResult,
    sync::Arc
};
use log::{debug, error};
use tokio::{
    sync::{mpsc, oneshot},
    task,
};

use boson_api::errors::{Result, StateError};

use self::runtime::Verticle;

use crate::{
    channel_listener::ChannelListener,
    connection_listener::ConnectionListener,
    contact_listener::ContactListener,
    friend_request_listener::FriendRequestListener,
    message_listener::MessageListener,
    session_listener::SessionListener,
    options::Options,
};

pub(crate) use client::VerticleClient;
pub(crate) use event::{Event, WithParams};

/// Everything needed to construct the messaging engine on its thread.
pub(crate) struct VerticleOptions {
    pub(crate) options: Arc<Options>,

    pub(crate) connection_listener: Arc<dyn ConnectionListener>,
    pub(crate) message_listener: Arc<dyn MessageListener>,
    pub(crate) channel_listener: Arc<dyn ChannelListener>,
    pub(crate) contact_listener: Arc<dyn ContactListener>,
    pub(crate) session_listener: Arc<dyn SessionListener>,
    pub(crate) friend_request_listener: Arc<dyn FriendRequestListener>,
}

/// Creates the one dedicated physical thread that owns the messaging engine.
pub(crate) async fn deploy(options: VerticleOptions) -> Result<VerticleClient> {
    let (event_tx, event_rx) = mpsc::unbounded_channel::<Event>();
    let (startup_tx, startup_rx) = oneshot::channel::<StdResult<(), String>>();

    let handle = std::thread::Builder::new()
        .name("messaging-verticle".into())
        .spawn(move || {
            let rt = match tokio::runtime::Builder::new_current_thread()
                .enable_time()
                .enable_io()
                .build()
            {
                Ok(rt) => rt,
                Err(error) => {
                    let _ = startup_tx.send(Err(format!(
                        "Building messaging verticle runtime failed: {error}"
                    )));
                    return;
                }
            };

            let local = task::LocalSet::new();
            rt.block_on(local.run_until(async move {
                let mut verticle = match Verticle::new(options, event_rx) {
                    Ok(verticle) => verticle,
                    Err(error) => {
                        let _ = startup_tx.send(Err(error.to_string()));
                        return;
                    }
                };
                let _ = startup_tx.send(Ok(()));
                verticle.run_loop().await;
            }));
        })
        .map_err(|error| {
            error!("Spawning messaging verticle thread failed: {error}");
            StateError::new(format!(
                "Spawning messaging verticle thread failed: {error}"
            ))
        })?;

    match startup_rx.await {
        Ok(Ok(())) => {
            debug!("Messaging verticle deployed successfully");
            Ok(VerticleClient::new(event_tx, handle))
        }
        Ok(Err(msg)) => {
            error!("Messaging verticle failed to deploy: {msg}");
            let _ = task::spawn_blocking(move || handle.join()).await;
            Err(StateError::new(msg))
        }
        Err(_) => {
            error!("Messaging verticle startup channel closed unexpectedly");
            let _ = task::spawn_blocking(move || handle.join()).await;
            Err(StateError::new("Messaging verticle startup channel closed"))
        }
    }
}
