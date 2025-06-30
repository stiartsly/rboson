use std::sync::{Arc, Mutex};
use std::path::{PathBuf, Path};
use std::time::SystemTime;

use tokio::{
    io::ReadHalf,
    io::AsyncReadExt,
    time::Instant,
    time::Duration,
    net::TcpStream,
    task,
    time
};
use log::{info, debug, error};

use crate::{
    elapsed_ms,
    srv_addr,
    unwrap,
    Id,
    core::Result,
    Error,
    signature,
    dht::Node,
};

use super::{
    connection::ProxyConnection,
    managed::ManagedFields,
    client,
};

//const IDLE_CHECK_INTERVAL:      u128 = 60 * 1000;           // 60s
//const MAX_IDLE_TIME:            u128 = 5 * 60 * 1000;       // 5 minutes;
const HEALTH_CHECK_INTERVAL:    u64  = 10 * 1000;           // 10s
const RE_ANNOUNCE_INTERVAL:     u128 = 60 * 60 * 1000;      // 1hour
const PERSISTENCE_INTERVAL:     u128 = 60 * 60 * 1000;      // 1hour

pub(crate) struct ManagedWorker {
    node:               Arc<Mutex<Node>>,
    cached_dir:         PathBuf,
    managed:            Arc<Mutex<ManagedFields>>,

    remote_peerid:      Id
}

impl ManagedWorker {
    pub fn new(cached_dir: PathBuf,
        node: Arc<Mutex<Node>>,
        managed: Arc<Mutex<ManagedFields>>,
        peerid: Id,
    ) -> Self {
        Self {
            node,
            cached_dir,
            managed,

            // replaystream_failures: 0,
            // upstream_failures:  0,

            remote_peerid:      peerid
        }
    }

    fn remote_peerid(&self) -> &Id {
        &self.remote_peerid
    }

    fn cache_dir(&self) -> &Path {
        self.cached_dir.as_path()
    }

    async fn announce_peer(&self) {
        let peer = match self.managed.lock().unwrap().peer.as_ref() {
            Some(v) => v.clone(),
            None => return,
        };

       info!("Announce peer {}: {}", peer.id(), peer);
        if let Some(url) = peer.alternative_url() {
            info!("-**- ActiveProxy: peer server: {}:{}, domain: {} -**-",
                srv_addr!(self.managed).ip(),
                peer.port(),
                url
            );
        } else {
            info!("-**- ActiveProxy: peer server: {}:{} -**-",
                srv_addr!(self.managed).ip(),
                peer.port()
            );
        }

        let node = self.node.clone();
        _ = node.lock()
            .unwrap()
            .announce_peer(&peer, None)
            .await;
    }
}

pub(crate) async fn run_loop(
    worker: Arc<Mutex<ManagedWorker>>,
    _quit: Arc<Mutex<bool>>
) -> Result<()> {
    let duration = Duration::from_millis(1000 as u64);
    let mut interval = time::interval_at(Instant::now() + duration, duration);
    let managed = worker.lock().unwrap().managed.clone();

    let keypair = signature::KeyPair::random();

    loop {
        if managed.lock().unwrap().needs_new_connection() {
            debug!("ActiveProxy tried to create a new connectoin...");

            let mut conn = ProxyConnection::new(
                managed.clone(),
                &keypair
            );

            _ = conn.connect_server().await;

            managed.lock().unwrap().connections += 1;
            task::spawn(async move {
                _ = run_connection(conn).await;
            });
        } else {
            _ = interval.tick().await;
            let worker = worker.clone();
            task::spawn(async move {
                _ = run_iteraction(worker);
            });
        }
        task::yield_now().await;
    }
}

async fn run_connection(mut conn: ProxyConnection) {
    let mut relay_data    = vec![0u8; 0x7FFF];
    let mut upstream_data = vec![0u8; 0x7FFF];
    let duration = Duration::from_millis(HEALTH_CHECK_INTERVAL);
    let mut ticker = time::interval_at(Instant::now() + duration, duration);
    let mut quit = false;

    while !quit {
        let mut relay    = conn.take_relay_reader();
        let mut upstream = conn.take_upstream_reader();

        let res1 = tokio::select! {
            res = read_stream(relay.as_mut(), &mut relay_data), if relay.is_some() => {
                match res {
                    Err(e)  => error!("Connection {} read relay stream error: {e}.", conn.cid()),
                    Ok(0)   => info!("Connection {} read EOF from relay stream.", conn.cid()),
                    Ok(len) => {
                        if let Err(e) = conn.on_relay_data(&relay_data[..len]).await {
                            error!("{e}");
                        } else {
                            conn.put_relay_reader(relay);
                            if upstream.is_some() {
                                conn.put_upstream_reader(upstream);
                            }
                            // task::yield_now().await;
                            continue;
                        }
                    },
                };

                // If reading from or writing to relay stream fails, the entire connection must
                // be closed and all resources must be reclaimed.
                true
            }
            res = read_stream(upstream.as_mut(), &mut upstream_data), if upstream.is_some() => {
                match res {
                    Err(e)  => error!("Connection {} read upstream stream error: {e}.", conn.cid()),
                    Ok(0)   => info!("Connection {} read EOF from upstream.", conn.cid()),
                    Ok(len) => {
                        if let Ok(_) = conn.on_upstream_data(&upstream_data[..len]).await {
                            conn.put_relay_reader(relay);
                            if upstream.is_some() {
                                conn.put_upstream_reader(upstream);
                            }
                            continue;
                        }
                    }
                };

                // If reading from or writing to upstream fails, only that connection needs to be closed.
                false
            }
            _  = ticker.tick() => {
                if let Ok(_) = conn.check_keepalive().await {
                    conn.put_relay_reader(relay);
                    if upstream.is_some() {
                        conn.put_upstream_reader(upstream);
                    }
                    continue;
                }

                // If sending keep-alive fails, the entire connection must to be closed.
                true
            }

        };

        conn.put_relay_reader(relay);
        if upstream.is_some() {
            conn.put_upstream_reader(upstream);
        }

        match res1 {
            true => {
                 _ = conn.close().await;
                quit = true;
            },
            false => {
                _ = conn.close_upstream().await
            }
        }
    }
}

async fn read_stream(mut stream: Option<&mut ReadHalf<TcpStream>>, data: &mut [u8]) -> Result<usize> {
    let stream = match stream.as_mut() {
        Some(v) => v,
        None => return Ok(0)
    };

    stream.read(data).await.map_err(|e| Error::from(e))
}

async fn run_iteraction(worker: Arc<Mutex<ManagedWorker>>) {
    let managed = worker.lock().unwrap().managed.clone();
    let node    = worker.lock().unwrap().node.clone();

    if elapsed_ms!(managed.lock().unwrap().last_save_peer) >= PERSISTENCE_INTERVAL {
        managed.lock().unwrap().last_save_peer = SystemTime::now();
        let peerid = worker.lock().unwrap().remote_peerid().clone();
        _ = client::lookup_peer(node.clone(), &peerid).await.map(|v| {
            _ = client::save_peer(
                worker.lock().unwrap().cache_dir(),
                v
            )
        })
    }

    if managed.lock().unwrap().peer.is_some() &&
        elapsed_ms!(managed.lock().unwrap().last_announce_peer) >= RE_ANNOUNCE_INTERVAL {
        managed.lock().unwrap().last_announce_peer = SystemTime::now();
        _ = worker.lock().unwrap().announce_peer();
    }
}
