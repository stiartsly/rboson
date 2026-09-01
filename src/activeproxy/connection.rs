use std::{
    mem,
    cell::RefCell,
    rc::{Rc, Weak},
    net::SocketAddr,
    time::{Duration, SystemTime},
};
use tokio::io::{
    split,
    ReadHalf,
    WriteHalf,
    AsyncReadExt,
    AsyncWriteExt
};
use tokio::net::TcpStream;
use tokio::time::{self, Instant};
use log::{error, info, debug, trace, warn};

use crate::{
    Id,
    Result,
    elapsed_ms,
    cryptobox,
    CryptoContext,
    core::errors::{MalformedError, ProtocolError, StateError},
};

use super::{
    random_timeshift,
    packet,
    packet_type::PacketType,
    state::State,
    session::ProxySession,
};

// packet size (2 bytes) + packet type (1 byte)
const PACKET_HEADER_BYTES: usize = mem::size_of::<u16>() + mem::size_of::<u8>();
const KEEPALIVE_INTERVAL:    u128 = 60000;   // 60 seconds
const MAX_KEEP_ALIVE_RETRY:  u128 = 3;
const HEALTH_CHECK_INTERVAL: u64  = 10 * 1000; // 10 seconds, drives the run() loop's keepalive ticks
// A relayed connection is fully torn down only after three disconnect confirmations:
// the local upstream end, the server DISCONNECT, and the matching DISCONNECT_ACK.
const DISCONNECT_CONFIRMS:  i32 = 3;

static mut NEXT_CONNID: i32 = 0;
fn next_connection_id() -> i32 {
    unsafe {
        NEXT_CONNID += 1;
        if NEXT_CONNID == 0 {
            NEXT_CONNID += 1;
        }
        NEXT_CONNID
    }
}

fn get_packet_type(packet: &[u8]) -> Result<PacketType> {
    if packet.len() < PACKET_HEADER_BYTES {
        return Err(MalformedError::new("packet too short"));
    }

    let size = u16::from_be_bytes([packet[0], packet[1]]) as usize;
    if size != packet.len() {
        return Err(MalformedError::new("packet size mismatch"));
    }

    PacketType::from(packet[mem::size_of::<u16>()])
}

pub(crate) struct ProxyConnection {
    conn_id:            i32,
    state:              State,
    keepalive:          SystemTime,
    disconnect_confirms: i32,

    // Back-reference to the owning session (Java's `ProxyConnectionHandler handler`) and to this
    // connection's own shared handle, so hook callbacks can pass `this` back to the session.
    handler:            Weak<ProxySession>,
    self_ref:           Weak<RefCell<ProxyConnection>>,

    relay_reader:       Option<ReadHalf<TcpStream>>,
    relay_writer:       Option<WriteHalf<TcpStream>>,

    upstream_reader:    Option<ReadHalf<TcpStream>>,
    upstream_writer:    Option<WriteHalf<TcpStream>>,

    stickybuf:          Vec<u8>,

    // Session-scoped crypto contexts, shared with the owning `ProxySession` and its sibling connections.
    peer_context:       Rc<RefCell<CryptoContext>>,
    session_context:    Rc<RefCell<Option<CryptoContext>>>,
}

impl ProxyConnection {
    // Constructs a connection around an already-established relay socket, mirroring the Java
    // constructor which is likewise handed an open `NetSocket`. Dialing the relay socket and
    // resolving the crypto contexts is the owning `ProxySession`'s responsibility.
    pub(crate) fn new_shared(
        session: &Rc<ProxySession>,
        stream: TcpStream,
        peer_context: Rc<RefCell<CryptoContext>>,
        session_context: Rc<RefCell<Option<CryptoContext>>>,
    ) -> Rc<RefCell<Self>> {
        let (reader, writer) = split(stream);

        let connection = Rc::new_cyclic(|weak_self| {
            RefCell::new(Self {
                conn_id:            next_connection_id(),
                state:              State::Initializing,
                keepalive:          SystemTime::now(),
                disconnect_confirms: 0,

                handler:            Rc::downgrade(session),
                self_ref:           weak_self.clone(),

                relay_reader:       Some(reader),
                relay_writer:       Some(writer),
                upstream_reader:    None,
                upstream_writer:    None,

                stickybuf:          Vec::with_capacity(4 * 1024),

                peer_context,
                session_context,
            })
        });

        info!("Connection {} is created.", connection.borrow().cid());
        connection
    }

    pub(crate) fn cid(&self) -> i32 {
        self.conn_id
    }

    pub(crate) fn take_relay_reader(&mut self) -> Option<ReadHalf<TcpStream>> {
        self.relay_reader.take()
    }

    pub(crate) fn take_upstream_reader(&mut self) -> Option<ReadHalf<TcpStream>> {
        self.upstream_reader.take()
    }

    pub(crate) fn put_relay_reader(&mut self, reader: Option<ReadHalf<TcpStream>>) {
        self.relay_reader = reader;
    }

    pub(crate) fn put_upstream_reader(&mut self, reader: Option<ReadHalf<TcpStream>>) {
        self.upstream_reader = reader;
    }

    // Drives this connection for its whole lifetime: de-frames the relay stream, relays upstream
    // data, and runs the keepalive tick. Stands in for Java's reactive `NetSocket` handlers, which
    // Vert.x invokes on their own without an explicit read loop.
    pub(crate) async fn run(conn: Rc<RefCell<ProxyConnection>>) {
        let mut relay_data = vec![0u8; 0x7FFF];
        let mut upstream_data = vec![0u8; 0x7FFF];
        let duration = Duration::from_millis(HEALTH_CHECK_INTERVAL);
        let mut ticker = time::interval_at(Instant::now() + duration, duration);

        loop {
            let mut relay = conn.borrow_mut().take_relay_reader();
            let mut upstream = conn.borrow_mut().take_upstream_reader();

            let close_all = tokio::select! {
                res = read_stream(relay.as_mut(), &mut relay_data), if relay.is_some() => {
                    match res {
                        Err(e) => {
                            error!("Connection {} read relay stream error: {e}", conn.borrow().cid());
                            true
                        },
                        Ok(0) => {
                            info!("Connection {} read EOF from relay stream", conn.borrow().cid());
                            true
                        },
                        Ok(len) => {
                            if let Err(e) = conn.borrow_mut().on_relay_data(&relay_data[..len]).await {
                                error!("Connection {} relay handling error: {e}", conn.borrow().cid());
                                true
                            } else {
                                conn.borrow_mut().put_relay_reader(relay.take());
                                if upstream.is_some() {
                                    conn.borrow_mut().put_upstream_reader(upstream.take());
                                }
                                continue;
                            }
                        }
                    }
                },
                res = read_stream(upstream.as_mut(), &mut upstream_data), if upstream.is_some() => {
                    match res {
                        Err(e) => {
                            error!("Connection {} read upstream stream error: {e}", conn.borrow().cid());
                            false
                        },
                        Ok(0) => {
                            info!("Connection {} read EOF from upstream", conn.borrow().cid());
                            false
                        },
                        Ok(len) => {
                            if conn.borrow_mut().on_upstream_data(&upstream_data[..len]).await.is_ok() {
                                conn.borrow_mut().put_relay_reader(relay.take());
                                if upstream.is_some() {
                                    conn.borrow_mut().put_upstream_reader(upstream.take());
                                }
                                continue;
                            }
                            true
                        }
                    }
                },
                _ = ticker.tick() => {
                    if conn.borrow_mut().check_keepalive().await.is_ok() {
                        conn.borrow_mut().put_relay_reader(relay.take());
                        if upstream.is_some() {
                            conn.borrow_mut().put_upstream_reader(upstream.take());
                        }
                        continue;
                    }
                    true
                }
            };

            conn.borrow_mut().put_relay_reader(relay);
            if upstream.is_some() {
                conn.borrow_mut().put_upstream_reader(upstream);
            }

            if close_all {
                let _ = conn.borrow_mut().close().await;
                break;
            } else {
                let _ = conn.borrow_mut().close_upstream().await;
            }
        }
    }

    fn allow(&self, addr: SocketAddr) -> bool {
        self.handler.upgrade().map(|h| h.allow(addr)).unwrap_or(false)
    }

    // Invokes `f` with the owning session and this connection's own shared handle, mirroring the
    // `handler.xxx(this)` calls on the Java side. No-op once either has been dropped.
    fn with_handler(&self, f: impl FnOnce(&Rc<ProxySession>, &Rc<RefCell<ProxyConnection>>)) {
        if let (Some(handler), Some(me)) = (self.handler.upgrade(), self.self_ref.upgrade()) {
            f(&handler, &me);
        }
    }

    fn on_opened(&self) {
        self.with_handler(|h, me| h.connection_open_handler(me));
    }

    fn on_closed(&self) {
        self.with_handler(|h, me| h.connection_closed_handler(me));
    }

    fn on_busy(&self) {
        self.with_handler(|h, me| h.connection_busy_handler(me));
    }

    fn on_idle(&self) {
        self.with_handler(|h, me| h.connection_idle_handler(me));
    }

    async fn open_upstream(&mut self) -> Result<()> {
        let Some(handler) = self.handler.upgrade() else {
            return Err(StateError::new("proxy session is gone"));
        };

        let stream = handler.connect_upstream().await?;
        let (reader, writer) = split(stream);
        self.upstream_reader = Some(reader);
        self.upstream_writer = Some(writer);
        Ok(())
    }

    fn connect_upstream(&mut self) {
        if self.state == State::Connecting {
            self.state = State::Relaying;
        } else {
            // Disconnected from the client side before connecting to the upstream:
            // drop the socket, keep the state.
            debug!("Connection {} dropped the upstream socket in {} state", self.cid(), self.state);
            self.upstream_reader = None;
            self.upstream_writer = None;
        }
    }

    async fn send_packet(&mut self, label: &str, payload: Vec<u8>) -> Result<()> {
        let Some(writer) = self.relay_writer.as_mut() else {
            return Err(StateError::new("relay writer is not available"));
        };

        let mut written = 0;
        while written < payload.len() {
            match writer.write(&payload[written..]).await {
                Ok(len) => written += len,
                Err(e) => {
                    error!("Connection {} failed to send {label} packet to proxy socket: {e}", self.cid());
                    self.close().await?;
                    return Err(e.into());
                }
            }
        }

        trace!("Connection {} sent {label} packet to proxy socket", self.cid());
        Ok(())
    }

    pub(crate) async fn send_auth(
        &mut self,
        user_id: Id,
        device_id: Id,
        client_session_pk: cryptobox::PublicKey,
        name_access: bool,
        device_sig: Vec<u8>,
    ) -> Result<()> {
        if self.state == State::Closed {
            return Ok(());
        }
        self.state = State::Authenticating;

        let auth = packet::Auth::new(packet::VERSION as u16, user_id, device_id, client_session_pk, name_access, device_sig);
        let payload = auth.encode(&mut self.peer_context.borrow_mut())?;
        self.send_packet("AUTH", payload).await
    }

    pub(crate) async fn send_attach(
        &mut self,
        device_id: Id,
        client_session_pk: cryptobox::PublicKey,
        device_sig: Vec<u8>,
    ) -> Result<()> {
        if self.state == State::Closed {
            return Ok(());
        }
        self.state = State::Attaching;

        let attach = packet::Attach::new(device_id, client_session_pk, device_sig);
        let payload = attach.encode(&mut self.peer_context.borrow_mut())?;
        self.send_packet("ATTACH", payload).await
    }

    async fn send_ping(&mut self) -> Result<()> {
        if self.state == State::Closed {
            return Ok(());
        }
        self.send_packet("PING", packet::Ping::encode()).await
    }

    async fn send_connect_ack(&mut self, succeeded: bool) -> Result<()> {
        let payload = packet::ConnectAck::new(succeeded).encode();
        self.send_packet("CONNECT_ACK", payload).await
    }

    async fn send_disconnect(&mut self) -> Result<()> {
        if self.state == State::Closed {
            return Ok(());
        }
        self.send_packet("DISCONNECT", packet::Disconnect::encode()).await
    }

    async fn send_disconnect_ack(&mut self) -> Result<()> {
        if self.state == State::Closed {
            return Ok(());
        }
        self.send_packet("DISCONNECT_ACK", packet::DisconnectAck::encode()).await
    }

    async fn send_data(&mut self, data: Vec<u8>) -> Result<()> {
        let session_context = self.session_context.borrow().clone()
            .ok_or_else(|| StateError::new("session crypto context is not established"))?;
        let mut ctx = session_context;
        let payload = packet::Data::new(data).encode(&mut ctx)?;
        *self.session_context.borrow_mut() = Some(ctx);
        self.send_packet("DATA", payload).await
    }

    pub(crate) async fn on_relay_data(&mut self, input: &[u8]) -> Result<()> {
        self.keepalive = SystemTime::now();

        let mut pos = 0;
        let mut remaining = input.len();

        if !self.stickybuf.is_empty() {
            if self.stickybuf.len() < PACKET_HEADER_BYTES {
                let need = PACKET_HEADER_BYTES - self.stickybuf.len();
                if remaining < need {
                    self.stickybuf.extend_from_slice(input);
                    return Ok(());
                }

                self.stickybuf.extend_from_slice(&input[..need]);
                pos += need;
                remaining -= need;
            }

            let packet_size = u16::from_be_bytes([self.stickybuf[0], self.stickybuf[1]]) as usize;
            if packet_size < PACKET_HEADER_BYTES {
                error!("Connection {} got malformed packet (declared size {packet_size}) from proxy socket", self.cid());
                return self.close().await;
            }

            let need = packet_size - self.stickybuf.len();
            if remaining < need {
                self.stickybuf.extend_from_slice(&input[pos..pos + remaining]);
                return Ok(());
            }

            self.stickybuf.extend_from_slice(&input[pos..pos + need]);
            pos += need;
            remaining -= need;

            let packet = mem::take(&mut self.stickybuf);
            self.packet_handler(&packet).await?;

            if self.state == State::Closed {
                return Ok(());
            }
        }

        while remaining > 0 {
            if remaining < PACKET_HEADER_BYTES {
                self.stickybuf.extend_from_slice(&input[pos..pos + remaining]);
                return Ok(());
            }

            let packet_size = u16::from_be_bytes([input[pos], input[pos + 1]]) as usize;
            if packet_size < PACKET_HEADER_BYTES {
                error!("Connection {} got malformed packet (declared size {packet_size}) from proxy socket", self.cid());
                return self.close().await;
            }

            if remaining < packet_size {
                self.stickybuf.extend_from_slice(&input[pos..pos + remaining]);
                return Ok(());
            }

            self.packet_handler(&input[pos..pos + packet_size]).await?;
            pos += packet_size;
            remaining -= packet_size;

            if self.state == State::Closed {
                return Ok(());
            }
        }

        Ok(())
    }

    async fn packet_handler(&mut self, packet: &[u8]) -> Result<()> {
        if self.state != State::Initializing {
            let packet_type = match get_packet_type(packet) {
                Ok(t) => t,
                Err(e) => {
                    error!("Connection {} got malformed packet from proxy socket: {e}", self.cid());
                    return self.close().await;
                }
            };

            trace!("Connection {} got {packet_type} packet ({} bytes) from proxy socket", self.cid(), packet.len());

            if !self.state.accept(&packet_type) {
                error!("Connection {} cannot accept {packet_type} packet in {} state", self.cid(), self.state);
                return self.close().await;
            }

            if let Err(e) = self.dispatch_packet(&packet_type, packet).await {
                error!("Connection {} got invalid {packet_type} packet from proxy socket: {e}", self.cid());
                return self.close().await;
            }

            return Ok(());
        }

        match packet::Challenge::decode(packet) {
            Ok(challenge) => {
                if let Err(e) = self.handle_challenge(challenge).await {
                    error!("Connection {} got invalid CHALLENGE packet from proxy socket: {e}", self.cid());
                    return self.close().await;
                }
                Ok(())
            },
            Err(e) => {
                error!("Connection {} got malformed CHALLENGE packet from proxy socket: {e}", self.cid());
                self.close().await
            }
        }
    }

    async fn dispatch_packet(&mut self, packet_type: &PacketType, packet: &[u8]) -> Result<()> {
        match packet_type {
            PacketType::AuthAck(_) => {
                let ack = packet::AuthAck::decode(packet, &self.peer_context.borrow())?;
                self.handle_auth_ack(ack)
            },
            PacketType::AttachAck(_) => {
                let ack = packet::AttachAck::decode(packet)?;
                self.handle_attach_ack(ack)
            },
            PacketType::PingAck(_) => {
                let ack = packet::PingAck::decode(packet)?;
                self.handle_ping_ack(ack)
            },
            PacketType::Connect(_) => {
                let session_context = self.session_context.borrow().clone()
                    .ok_or_else(|| StateError::new("session crypto context is not established"))?;
                let conn = packet::Connect::decode(packet, &session_context)?;
                self.handle_connect(conn).await
            },
            PacketType::Data(_) => {
                let session_context = self.session_context.borrow().clone()
                    .ok_or_else(|| StateError::new("session crypto context is not established"))?;
                let data = packet::Data::decode(packet, &session_context)?;
                self.handle_data(data).await
            },
            PacketType::Disconnect(_) => {
                let d = packet::Disconnect::decode(packet)?;
                self.handle_disconnect(d).await
            },
            PacketType::DisconnectAck(_) => {
                let d = packet::DisconnectAck::decode(packet)?;
                self.handle_disconnect_ack(d).await
            },
            PacketType::Error(_) => {
                let session_context = self.session_context.borrow().clone()
                    .ok_or_else(|| StateError::new("session crypto context is not established"))?;
                let err = packet::Error::decode(packet, &session_context)?;
                error!("Connection {} got ERROR response from the server, error: {}: {}",
                    self.cid(), err.code(), err.message().unwrap_or_default());
                Err(ProtocolError::new("Packet error"))
            },
            _ => {
                error!("INTERNAL ERROR: Connection {} got wrong {packet_type} packet in {} state", self.cid(), self.state);
                Ok(())
            }
        }
    }

    async fn handle_challenge(&mut self, challenge: packet::Challenge) -> Result<()> {
        let Some(handler) = self.handler.upgrade() else {
            return Err(StateError::new("proxy session is gone"));
        };
        let Some(me) = self.self_ref.upgrade() else {
            return Err(StateError::new("connection handle is gone"));
        };
        handler.connection_challenge_handler(&me, challenge.challenge());
        Ok(())
    }

    fn handle_auth_ack(&mut self, ack: packet::AuthAck) -> Result<()> {
        let Some(handler) = self.handler.upgrade() else {
            return Err(StateError::new("proxy session is gone"));
        };
        handler.authenticated_handler(
            ack.server_session_pk(), ack.max_connections() as i32, ack.name_access(),
            ack.endpoint(), ack.named_endpoint(),
        )?;
        self.state = State::Idling;
        self.on_opened();
        info!("Connection {} opened.", self.cid());
        Ok(())
    }

    fn handle_attach_ack(&mut self, _ack: packet::AttachAck) -> Result<()> {
        self.state = State::Idling;
        self.on_opened();
        info!("Connection {} opened.", self.cid());
        Ok(())
    }

    fn handle_ping_ack(&mut self, _ack: packet::PingAck) -> Result<()> {
        // keep-alive timestamp is already updated on receipt of any relay data.
        Ok(())
    }

    async fn handle_connect(&mut self, conn: packet::Connect) -> Result<()> {
        let addr = SocketAddr::new(conn.address(), conn.port());
        if !self.allow(addr) {
            return self.send_connect_ack(false).await;
        }

        self.state = State::Connecting;
        // Reset the disconnect handshake count at the start of a new relay cycle so that a DISCONNECT
        // racing this CONNECT keeps its confirmation instead of being wiped by a late callback.
        self.disconnect_confirms = 0;
        self.on_busy();

        debug!("Connection {} connecting to the upstream...", self.cid());
        match self.open_upstream().await {
            Ok(()) => {
                debug!("Connection {} connected to the upstream", self.cid());
                self.connect_upstream();
                self.send_connect_ack(true).await
            },
            Err(e) => {
                self.state = State::Idling;
                self.on_idle();
                error!("Connection {} failed to connect to upstream: {e}", self.cid());
                self.send_connect_ack(false).await
            }
        }
    }

    async fn handle_data(&mut self, data: packet::Data) -> Result<()> {
        if self.state != State::Relaying {
            trace!("Connection {} dropping DATA packet because the connection is not in the relaying state",
                self.cid());
            return Ok(());
        }

        let Some(writer) = self.upstream_writer.as_mut() else {
            return Err(StateError::new("upstream writer is not available"));
        };

        let payload = data.data();
        let mut written = 0;
        let mut close_upstream = false;
        {
            while written < payload.len() {
                match writer.write(&payload[written..]).await {
                    Ok(len) => written += len,
                    Err(e) => {
                        error!("Connection {} failed to write data to upstream: {e}", self.cid());
                        close_upstream = true;
                        break;
                    }
                }
            }
        }

        if close_upstream {
            let _ = self.close_upstream().await;
            return Ok(());
        }

        trace!("Connection {} sent {} bytes data to upstream", self.cid(), payload.len());
        Ok(())
    }

    async fn handle_disconnect(&mut self, _pkt: packet::Disconnect) -> Result<()> {
        debug!("Connection {} got DISCONNECT from server", self.cid());

        // Disconnected from the client side before connecting to the upstream:
        // assume the upstream is already gone and account
        // for that leg of the handshake before sending our own DISCONNECT.
        if self.state == State::Connecting && self.upstream_writer.is_none() {
            self.confirm_disconnect();
            let _ = self.send_disconnect().await;
        }

        self.state = State::Disconnecting;
        self.disconnect_upstream().await;
        self.send_disconnect_ack().await
    }

    async fn handle_disconnect_ack(&mut self, _pkt: packet::DisconnectAck) -> Result<()> {
        debug!("Connection {} got DISCONNECT_ACK from server", self.cid());
        self.disconnect_upstream().await;
        Ok(())
    }

    async fn disconnect_upstream(&mut self) {
        if let Some(mut writer) = self.upstream_writer.take() {
            let _ = writer.shutdown().await;
        }
        self.upstream_reader = None;
        self.confirm_disconnect();
    }

    // A relayed connection returns to Idling only after DISCONNECT_CONFIRMS confirmations are observed:
    // the local upstream end, the server DISCONNECT, and the matching DISCONNECT_ACK.
    fn confirm_disconnect(&mut self) {
        self.disconnect_confirms += 1;
        if self.disconnect_confirms == DISCONNECT_CONFIRMS {
            trace!("Connection {} disconnect confirmed, changing state to idle", self.cid());
            self.state = State::Idling;
            self.disconnect_confirms = 0;
            self.on_idle();
        }
    }

    pub(crate) async fn on_upstream_data(&mut self, data: &[u8]) -> Result<()> {
        if self.state != State::Relaying {
            trace!("Connection {} dropping data from upstream because the connection is not in the relaying state", self.cid());
            return Ok(());
        }
        self.send_data(data.to_vec()).await
    }

    pub(crate) async fn check_keepalive(&mut self) -> Result<()> {
        if elapsed_ms!(self.keepalive) >= MAX_KEEP_ALIVE_RETRY * KEEPALIVE_INTERVAL {
            warn!("Connection {} keep alive timeout, closing now", self.cid());
            return Err(StateError::new(format!("Connection {} is dead", self.cid())));
        }

        let random_shift = random_timeshift() as u128 * 1000; // max 10 seconds
        if elapsed_ms!(self.keepalive) >= KEEPALIVE_INTERVAL - random_shift {
            return self.send_ping().await;
        }
        Ok(())
    }

    pub(crate) async fn close_upstream(&mut self) -> Result<()> {
        if self.state == State::Closed || self.state == State::Idling {
            return Ok(());
        }

        info!("Connection {} closing upstream", self.cid());

        self.state = State::Disconnecting;
        let _ = self.send_disconnect().await;
        self.disconnect_upstream().await;
        Ok(())
    }

    pub(crate) async fn close(&mut self) -> Result<()> {
        if self.state == State::Closed {
            return Ok(());
        }
        self.state = State::Closed;

        info!("Connection {} is closing...", self.cid());

        if let Some(mut writer) = self.upstream_writer.take() {
            let _ = writer.shutdown().await;
            info!("Connection {} upstream socket closed", self.cid());
        }
        self.upstream_reader = None;

        if let Some(mut writer) = self.relay_writer.take() {
            let _ = writer.shutdown().await;
            info!("Connection {} proxy socket closed", self.cid());
        }
        self.relay_reader = None;

        self.stickybuf.clear();
        self.on_closed();

        info!("Connection {} closed", self.cid());
        Ok(())
    }
}

async fn read_stream(stream: Option<&mut ReadHalf<TcpStream>>, data: &mut [u8]) -> Result<usize> {
    let Some(stream) = stream else {
        return Ok(0);
    };

    stream.read(data).await.map_err(|e| e.into())
}
