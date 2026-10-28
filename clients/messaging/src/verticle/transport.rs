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

use tokio::sync::{mpsc, oneshot};

use crate::{mqtt::TransportEvent, rpc_server::RpcServer};

/// One transport event plus the acknowledgement that lets the pump poll again.
///
/// The event loop's `poll` future is not cancellation-safe. Keeping the pump in
/// its own local task means `select!` never drops it; the acknowledgement keeps
/// `refresh_credentials` from racing with the event loop lock.
pub(super) type Delivery = (TransportEvent, oneshot::Sender<()>);

/// Polls the MQTT transport continuously on the verticle's own thread.
///
/// The pump deliberately runs as its own task instead of a `select!` branch:
/// dropping a partially polled `rumqttc::EventLoop` can lose packets or corrupt
/// its reconnect state. The main loop acknowledges each event after it has
/// refreshed credentials and scheduled the dispatch work.
pub(super) async fn run(rpc: Rc<RpcServer>, tx: mpsc::UnboundedSender<Delivery>) {
    while rpc.is_running() {
        let event = rpc.receive_event().await;
        let (ack_tx, ack_rx) = oneshot::channel();
        if tx.send((event, ack_tx)).is_err() {
            break;
        }
        if ack_rx.await.is_err() {
            break;
        }
    }
}
