// Loopback benchmarks of the proxy data path (relay socket <-> ProxyConnection <-> upstream socket).
//
// They isolate the proxy from the WAN: every connection has a real TCP relay socket and a real
// TCP upstream socket on 127.0.0.1, and a peer thread plays the ActiveProxy server. Run with:
//
//   cargo test --release -p boson-active-proxy-client bench_ -- --ignored --nocapture --test-threads=1

use futures::future::LocalBoxFuture;
use std::{
    cell::RefCell,
    io::{Read, Write},
    net::{IpAddr, Ipv6Addr, SocketAddr, TcpListener as StdListener, TcpStream as StdStream},
    rc::Rc,
    sync::{Arc, Barrier},
    thread,
    time::Instant,
};
use tokio::{net::TcpStream, runtime, sync::oneshot, task};

use crate::{connection::ProxyConnection, connection_handler::ConnectionHandler, packet};
use boson_api::{cryptobox, CryptoContext, Id, Result};

const DATA_OVERHEAD: usize = 3 + cryptobox::Nonce::BYTES + cryptobox::CryptoBox::MAC_BYTES;
const CHUNK: usize = 32_000;

#[derive(Clone, Copy, PartialEq)]
enum Direction {
    // upstream -> proxy -> relay: a remote client downloading from the proxied service
    Download,
    // relay -> proxy -> upstream: a remote client uploading to the proxied service
    Upload,
}

struct BenchHandler {
    upstream: SocketAddr,
}

impl ConnectionHandler for BenchHandler {
    fn challenge(&self, _: &Rc<ProxyConnection>, _: &[u8]) {}
    fn authenticated(
        &self,
        _: &Rc<ProxyConnection>,
        _: &cryptobox::PublicKey,
        _: i32,
        _: bool,
        _: &str,
        _: Option<&str>,
    ) -> Option<CryptoContext> {
        None
    }
    fn open(&self, _: &Rc<ProxyConnection>) {}
    fn close(&self, _: &Rc<ProxyConnection>) {}
    fn idle(&self, _: &Rc<ProxyConnection>) {}
    fn busy(&self, _: &Rc<ProxyConnection>) {}
    fn allow(&self, _: SocketAddr) -> bool {
        true
    }
    fn connect_upstream(&self) -> LocalBoxFuture<'_, Result<TcpStream>> {
        let addr = self.upstream;
        Box::pin(async move {
            let stream = TcpStream::connect(addr).await?;
            let _ = stream.set_nodelay(true);
            Ok(stream)
        })
    }
}

fn context_pair() -> (CryptoContext, CryptoContext) {
    let client = cryptobox::KeyPair::random();
    let server = cryptobox::KeyPair::random();
    let client_ctx = CryptoContext::new(
        Id::random(),
        cryptobox::CryptoBox::try_from((&server.to_public_key(), client.private_key())).unwrap(),
    );
    let server_ctx = CryptoContext::new(
        Id::random(),
        cryptobox::CryptoBox::try_from((&client.to_public_key(), server.private_key())).unwrap(),
    );
    (client_ctx, server_ctx)
}

// Reads whole packets from the relay socket; `on_packet` gets each packet's size and returns
// false to stop.
fn read_packets(stream: &mut StdStream, mut on_packet: impl FnMut(usize) -> bool) {
    let mut buf = vec![0u8; 1 << 20];
    let mut acc: Vec<u8> = Vec::with_capacity(2 << 20);
    loop {
        let n = stream.read(&mut buf).unwrap_or(0);
        if n == 0 {
            return;
        }
        acc.extend_from_slice(&buf[..n]);
        let mut pos = 0;
        while acc.len() - pos >= 2 {
            let size = u16::from_be_bytes([acc[pos], acc[pos + 1]]) as usize;
            if acc.len() - pos < size {
                break;
            }
            pos += size;
            if !on_packet(size) {
                return;
            }
        }
        acc.drain(..pos);
    }
}

// Server side of one connection: accepts the relay and upstream sockets and sends CONNECT.
fn accept_and_connect(
    relay_l: &StdListener,
    upstream_l: &StdListener,
    server_ctx: &mut CryptoContext,
) -> (StdStream, StdStream) {
    let (mut relay, _) = relay_l.accept().unwrap();
    let _ = relay.set_nodelay(true);
    let connect = packet::Connect::new(IpAddr::V6(Ipv6Addr::LOCALHOST), 80)
        .encode(server_ctx)
        .unwrap();
    relay.write_all(&connect).unwrap();
    let (upstream, _) = upstream_l.accept().unwrap();
    let _ = upstream.set_nodelay(true);
    (relay, upstream)
}

fn download_peer(
    relay_l: StdListener,
    upstream_l: StdListener,
    mut server_ctx: CryptoContext,
    total: usize,
    start: Arc<Barrier>,
    done: oneshot::Sender<f64>,
) {
    let (mut relay, mut upstream) = accept_and_connect(&relay_l, &upstream_l, &mut server_ctx);

    start.wait();
    let t0 = Instant::now();
    let source = thread::spawn(move || {
        let buf = vec![0xAB; 65_536];
        let mut sent = 0;
        while sent < total {
            let n = buf.len().min(total - sent);
            if upstream.write_all(&buf[..n]).is_err() {
                break;
            }
            sent += n;
        }
        upstream
    });

    let mut got = 0usize;
    let mut first = true;
    read_packets(&mut relay, |size| {
        if first {
            first = false; // CONNECT_ACK
        } else {
            got += size - DATA_OVERHEAD;
        }
        got < total
    });
    let secs = t0.elapsed().as_secs_f64();
    let _ = source.join();
    let _ = done.send(secs);
}

fn upload_peer(
    relay_l: StdListener,
    upstream_l: StdListener,
    mut server_ctx: CryptoContext,
    total: usize,
    start: Arc<Barrier>,
    done: oneshot::Sender<f64>,
) {
    let (mut relay, mut upstream) = accept_and_connect(&relay_l, &upstream_l, &mut server_ctx);

    // Pre-encrypt a ring of packets so the peer is never the bottleneck.
    let packets: Vec<Vec<u8>> = (0..512)
        .map(|_| {
            packet::Data::new(vec![0xCD; CHUNK])
                .encode(&mut server_ctx)
                .unwrap()
        })
        .collect();
    read_packets(&mut relay, |_| false); // CONNECT_ACK

    let sink = thread::spawn(move || {
        let mut buf = vec![0u8; 1 << 20];
        let mut got = 0;
        while got < total {
            let n = upstream.read(&mut buf).unwrap_or(0);
            if n == 0 {
                break;
            }
            got += n;
        }
    });

    start.wait();
    let t0 = Instant::now();
    let mut sent = 0;
    let mut i = 0;
    while sent < total {
        relay.write_all(&packets[i % packets.len()]).unwrap();
        sent += CHUNK;
        i += 1;
    }
    sink.join().unwrap();
    let _ = done.send(t0.elapsed().as_secs_f64());
}

// Runs `conns` connections on this thread's own current-thread runtime.
fn proxy_thread(dir: Direction, conns: usize, mb: usize, start: Arc<Barrier>) -> Vec<f64> {
    let rt = runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let local = task::LocalSet::new();
    rt.block_on(local.run_until(async move {
        let mut dones = Vec::new();
        let mut keep = Vec::new();
        for _ in 0..conns {
            let relay_l = StdListener::bind("127.0.0.1:0").unwrap();
            let upstream_l = StdListener::bind("127.0.0.1:0").unwrap();
            let relay_addr = relay_l.local_addr().unwrap();
            let upstream_addr = upstream_l.local_addr().unwrap();
            let (client_ctx, server_ctx) = context_pair();
            let (tx, rx) = oneshot::channel();
            let total = mb * 1_000_000;
            let start = start.clone();
            thread::spawn(move || match dir {
                Direction::Download => {
                    download_peer(relay_l, upstream_l, server_ctx, total, start, tx)
                }
                Direction::Upload => upload_peer(relay_l, upstream_l, server_ctx, total, start, tx),
            });

            let stream = TcpStream::connect(relay_addr).await.unwrap();
            let _ = stream.set_nodelay(true);
            let handler: Rc<dyn ConnectionHandler> = Rc::new(BenchHandler {
                upstream: upstream_addr,
            });
            let peer_ctx = Rc::new(RefCell::new(context_pair().0));
            let session_ctx = Rc::new(RefCell::new(Some(client_ctx)));
            let conn = ProxyConnection::new(0, stream, peer_ctx, session_ctx, handler);
            conn.force_idle();
            task::spawn_local(conn.clone().run());
            keep.push(conn);
            dones.push(rx);
        }
        let mut secs = Vec::new();
        for rx in dones {
            secs.push(rx.await.unwrap());
        }
        drop(keep);
        secs
    }))
}

// `threads` proxy threads, `conns` connections per thread, `mb` MB per connection.
fn run(dir: Direction, threads: usize, conns: usize, mb: usize) -> f64 {
    let start = Arc::new(Barrier::new(threads * conns));
    let handles: Vec<_> = (0..threads)
        .map(|_| {
            let start = start.clone();
            thread::spawn(move || proxy_thread(dir, conns, mb, start))
        })
        .collect();
    let mut slowest: f64 = 0.0;
    for h in handles {
        for secs in h.join().unwrap() {
            slowest = slowest.max(secs);
        }
    }
    (threads * conns * mb) as f64 / slowest
}

fn report(label: &str, dir: Direction, threads: usize, conns: usize, mb: usize) {
    let mbps = run(dir, threads, conns, mb);
    let name = if dir == Direction::Download { "download" } else { "upload  " };
    println!(
        "BENCH {label:<28} {name} threads={threads} conns/thread={conns} -> {mbps:9.1} MB/s ({:.2} Gbit/s)",
        mbps * 8.0 / 1000.0
    );
}

#[test]
#[ignore]
fn bench_single_connection() {
    for dir in [Direction::Download, Direction::Upload] {
        report("1 connection", dir, 1, 1, 256);
    }
}

#[test]
#[ignore]
fn bench_thread_model() {
    for dir in [Direction::Download, Direction::Upload] {
        report("8 conns, 1 thread (current)", dir, 1, 8, 64);
        report("8 conns, 2 threads", dir, 2, 4, 64);
        report("8 conns, 4 threads", dir, 4, 2, 64);
    }
}

fn time_mbps(label: &str, bytes_per_iter: usize, iters: usize, mut f: impl FnMut()) {
    let t0 = Instant::now();
    for _ in 0..iters {
        f();
    }
    let secs = t0.elapsed().as_secs_f64();
    println!(
        "BENCH codec {label:<34} {:9.1} MB/s",
        (bytes_per_iter * iters) as f64 / secs / 1e6
    );
}

// Per-packet cost of each stage on one thread: shows how much of the budget is crypto itself.
#[test]
#[ignore]
fn bench_codec() {
    let (mut client, server) = context_pair();
    let plain = vec![0xABu8; CHUNK];
    let iters = 20_000;

    time_mbps("memcpy (reference)", CHUNK, iters, || {
        let v = std::hint::black_box(plain.to_vec());
        std::hint::black_box(&v);
    });
    time_mbps("encrypt_into (alloc+crypto+copies)", CHUNK, iters, || {
        std::hint::black_box(client.encrypt_into(&plain).unwrap());
    });
    time_mbps("Data::encode (full send path)", CHUNK, iters, || {
        std::hint::black_box(packet::Data::new(plain.to_vec()).encode(&mut client).unwrap());
    });
    let cipher = client.encrypt_into(&plain).unwrap();
    time_mbps("decrypt_into (alloc+crypto+copies)", CHUNK, iters, || {
        server.reset_nonce();
        std::hint::black_box(server.decrypt_into(&cipher).unwrap());
    });
}
