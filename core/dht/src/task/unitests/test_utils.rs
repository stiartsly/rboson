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

use crate::{
    connection_status_listener::ConnectionStatusListener,
    dht::DHT,
    dht_verticle::VerticleOptions,
    storage::{data_storage::DataStorage, sqlite_storage::SqliteStorage},
    token_manager::TokenManager,
};
use boson_api::{CryptoIdentity, LocalBoxTimerClient, LocalBoxTimerCmd, Network};
use std::{
    rc::Rc,
    sync::{Arc, Mutex},
};
use tokio::sync::mpsc;

struct NoopConnectionStatusListener;
impl ConnectionStatusListener for NoopConnectionStatusListener {}

/// Create a minimal in-process DHT for unit tests. The node binds to
/// `host:0` (OS-assigned port), uses an in-memory SQLite store, and
/// has no bootstrap nodes.
pub(super) fn make_test_dht(network: Network, host: &str) -> Rc<DHT> {
    let identity = Arc::new(CryptoIdentity::new());
    let storage: Arc<Mutex<dyn DataStorage>> = Arc::new(Mutex::new(SqliteStorage::new()));
    let token_man = Arc::new(TokenManager::new());
    let listener: Arc<dyn ConnectionStatusListener> = Arc::new(NoopConnectionStatusListener);

    let options = VerticleOptions {
        identity,
        storage,
        token_man,
        listener,
        bootstrap_nodes: Vec::new(),
        developer_mode: false,
    };
    let (tx, _rx) = mpsc::unbounded_channel::<LocalBoxTimerCmd>();
    let timer_client = Rc::new(LocalBoxTimerClient::new(tx));

    DHT::new(options, network, host.to_string(), 39001, None, timer_client)
}
