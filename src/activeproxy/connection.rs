use std::{
    mem,
    cell::RefCell,
    rc::Rc,
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
    connection_handler::ConnectionHandler,
};

// packet size (2 bytes) + packet type (1 byte)
const PACKET_HEADER_BYTES: usize = mem::size_of::<u16>() + mem::size_of::<u8>();
const KEEPALIVE_INTERVAL:    u128 = 60000;   // 60 seconds
const MAX_KEEP_ALIVE_RETRY:  u128 = 3;
const HEALTH_CHECK_INTERVAL: u64  = 10 * 1000; // 10 seconds, drives the run() loop's keepalive ticks
// A relayed connection is fully torn down only after three disconnect confirmations:
// the local upstream end, the server DISCONNECT, and the matching DISCONNECT_ACK.
const DISCONNECT_CONFIRMS:  i32 = 3;

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
    conn_id             : i32,
    state               : RefCell<State>,
    keepalive           : RefCell<SystemTime>,
    disconnect_confirms : RefCell<i32>,

    handler             : Rc<dyn ConnectionHandler>,

    relay_rx            : RefCell<Option<ReadHalf<TcpStream>>>,
    relay_tx            : RefCell<Option<WriteHalf<TcpStream>>>,

    upstream_rx         : RefCell<Option<ReadHalf<TcpStream>>>,
    upstream_tx         : RefCell<Option<WriteHalf<TcpStream>>>,

    stickybuf           : RefCell<Vec<u8>>,

    peer_context        : Rc<RefCell<CryptoContext>>,
    session_context     : Rc<RefCell<Option<CryptoContext>>>,
}

impl ProxyConnection {
    pub(crate) fn new(
        cid: i32,
        stream: TcpStream,
        peer_context: Rc<RefCell<CryptoContext>>,
        session_context: Rc<RefCell<Option<CryptoContext>>>,
        handler: Rc<dyn ConnectionHandler>,
    ) -> Rc<Self> {
        let (rx, tx) = split(stream);
        Rc::new(Self {
                conn_id:            cid,
                state:              RefCell::new(State::Initializing),
                keepalive:          RefCell::new(SystemTime::now()),
                disconnect_confirms: RefCell::new(0),

                handler,

                relay_rx:           RefCell::new(Some(rx)),
                relay_tx:           RefCell::new(Some(tx)),

                upstream_rx:        RefCell::new(None),
                upstream_tx:        RefCell::new(None),

                stickybuf:          RefCell::new(Vec::with_capacity(4 * 1024)),

                peer_context,
                session_context,
        })
    }

    pub(crate) fn cid(self: &Rc<Self>) -> i32 {
        self.conn_id
    }

    fn allow(self: &Rc<Self>, addr: SocketAddr) -> bool {
        self.handler.allow(addr)
    }

    fn on_opened(self: &Rc<Self>) {
        self.handler.open(self);
    }

    fn on_closed(self: &Rc<Self>) {
        self.handler.close(self);
    }

    fn _on_busy(self: &Rc<Self>) {
        self.handler.busy(self);
    }

    fn on_idle(self: &Rc<Self>) {
        self.handler.idle(self);
    }

    async fn open_upstream(self: &Rc<Self>) -> Result<()> {
        let stream = self.handler.connect_upstream().await?;
        let (rx, tx) = split(stream);
        *self.upstream_rx.borrow_mut() = Some(rx);
        *self.upstream_tx.borrow_mut() = Some(tx);
        Ok(())
    }

    fn connect_upstream(self: &Rc<Self>) {
        if *self.state.borrow() == State::Connecting {
            *self.state.borrow_mut() = State::Relaying;
        } else {
            // Disconnected from the client side before connecting to the upstream:
            // drop the socket, keep the state.
            debug!("Connection {} dropped the upstream socket in {} state", self.cid(), self.state.borrow());
            *self.upstream_rx.borrow_mut() = None;
            *self.upstream_tx.borrow_mut() = None;
        }
    }

    async fn send_packet(self: &Rc<Self>, label: &str, payload: Vec<u8>) -> Result<()> {
        let Some(mut writer) = self.relay_tx.borrow_mut().take() else {
            return Err(StateError::new("relay writer is not available"));
        };

        let mut written = 0;
        while written < payload.len() {
            match writer.write(&payload[written..]).await {
                Ok(len) => written += len,
                Err(e) => {
                    error!("Connection {} failed to send {label} packet to proxy socket: {e}", self.cid());
                    *self.relay_tx.borrow_mut() = Some(writer);
                    self.close().await?;
                    return Err(e.into());
                }
            }
        }

        *self.relay_tx.borrow_mut() = Some(writer);
        trace!("Connection {} sent {label} packet to proxy socket", self.cid());
        Ok(())
    }

    pub(crate) async fn send_auth(
        self: &Rc<Self>,
        user_id: Id,
        device_id: Id,
        client_session_pk: cryptobox::PublicKey,
        name_access: bool,
        device_sig: Vec<u8>,
    ) -> Result<()> {
        if *self.state.borrow() == State::Closed {
            return Ok(());
        }
        *self.state.borrow_mut() = State::Authenticating;

        let auth = packet::Auth::new(
            packet::VERSION as u16,
            user_id,
            device_id,
            client_session_pk,
            name_access,
            device_sig
        );
        let payload = auth.encode(&mut self.peer_context.borrow_mut())?;
        self.send_packet("AUTH", payload).await
    }

    pub(crate) async fn send_attach(
        self: &Rc<Self>,
        device_id: Id,
        client_session_pk: cryptobox::PublicKey,
        device_sig: Vec<u8>,
    ) -> Result<()> {
        if *self.state.borrow() == State::Closed {
            return Ok(());
        }
        *self.state.borrow_mut() = State::Attaching;

        let attach = packet::Attach::new(device_id, client_session_pk, device_sig);
        let payload = attach.encode(&mut self.peer_context.borrow_mut())?;
        self.send_packet("ATTACH", payload).await
    }

    async fn send_ping(self: &Rc<Self>) -> Result<()> {
        if *self.state.borrow() == State::Closed {
            return Ok(());
        }
        self.send_packet("PING", packet::Ping::encode()).await
    }

    async fn send_connect_ack(self: &Rc<Self>, succeeded: bool) -> Result<()> {
        let payload = packet::ConnectAck::new(succeeded).encode();
        self.send_packet("CONNECT_ACK", payload).await
    }

    async fn send_disconnect(self: &Rc<Self>) -> Result<()> {
        if *self.state.borrow() == State::Closed {
            return Ok(());
        }
        self.send_packet("DISCONNECT", packet::Disconnect::encode()).await
    }

    async fn send_disconnect_ack(self: &Rc<Self>) -> Result<()> {
        if *self.state.borrow() == State::Closed {
            return Ok(());
        }
        self.send_packet("DISCONNECT_ACK", packet::DisconnectAck::encode()).await
    }

    async fn send_data(self: &Rc<Self>, data: Vec<u8>) -> Result<()> {
        let session_context = self.session_context.borrow().clone()
            .ok_or_else(|| StateError::new("session crypto context is not established"))?;
        let mut ctx = session_context;
        let payload = packet::Data::new(data).encode(&mut ctx)?;
        *self.session_context.borrow_mut() = Some(ctx);
        self.send_packet("DATA", payload).await
    }

    pub(crate) async fn on_relay_data(self: &Rc<Self>, input: &[u8]) -> Result<()> {
        *self.keepalive.borrow_mut() = SystemTime::now();

        let mut pos = 0;
        let mut remaining = input.len();

        if !self.stickybuf.borrow().is_empty() {
            if self.stickybuf.borrow().len() < PACKET_HEADER_BYTES {
                let need = PACKET_HEADER_BYTES - self.stickybuf.borrow().len();
                if remaining < need {
                    self.stickybuf.borrow_mut().extend_from_slice(input);
                    return Ok(());
                }

                self.stickybuf.borrow_mut().extend_from_slice(&input[..need]);
                pos += need;
                remaining -= need;
            }

            let packet_size = {
                let stickybuf = self.stickybuf.borrow();
                u16::from_be_bytes([stickybuf[0], stickybuf[1]]) as usize
            };
            if packet_size < PACKET_HEADER_BYTES {
                error!("Connection {} got malformed packet (declared size {packet_size}) from proxy socket", self.cid());
                return self.close().await;
            }

            let need = packet_size - self.stickybuf.borrow().len();
            if remaining < need {
                self.stickybuf.borrow_mut().extend_from_slice(&input[pos..pos + remaining]);
                return Ok(());
            }

            self.stickybuf.borrow_mut().extend_from_slice(&input[pos..pos + need]);
            pos += need;
            remaining -= need;

            let packet = mem::take(&mut *self.stickybuf.borrow_mut());
            self.packet_handler(&packet).await?;

            if *self.state.borrow() == State::Closed {
                return Ok(());
            }
        }

        while remaining > 0 {
            if remaining < PACKET_HEADER_BYTES {
                self.stickybuf.borrow_mut().extend_from_slice(&input[pos..pos + remaining]);
                return Ok(());
            }

            let packet_size = u16::from_be_bytes([input[pos], input[pos + 1]]) as usize;
            if packet_size < PACKET_HEADER_BYTES {
                error!("Connection {} got malformed packet (declared size {packet_size}) from proxy socket", self.cid());
                return self.close().await;
            }

            if remaining < packet_size {
                self.stickybuf.borrow_mut().extend_from_slice(&input[pos..pos + remaining]);
                return Ok(());
            }

            self.packet_handler(&input[pos..pos + packet_size]).await?;
            pos += packet_size;
            remaining -= packet_size;

            if *self.state.borrow() == State::Closed {
                return Ok(());
            }
        }

        Ok(())
    }

    async fn packet_handler(self: &Rc<Self>, packet: &[u8]) -> Result<()> {
        if *self.state.borrow() != State::Initializing {
            let packet_type = match get_packet_type(packet) {
                Ok(t) => t,
                Err(e) => {
                    error!("Connection {} got malformed packet from proxy socket: {e}", self.cid());
                    return self.close().await;
                }
            };

            trace!("Connection {} got {packet_type} packet ({} bytes) from proxy socket", self.cid(), packet.len());

            if !self.state.borrow().accept(&packet_type) {
                error!("Connection {} cannot accept {packet_type} packet in {} state", self.cid(), self.state.borrow());
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

    async fn dispatch_packet(self: &Rc<Self>, packet_type: &PacketType, packet: &[u8]) -> Result<()> {
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
                error!("INTERNAL ERROR: Connection {} got wrong {packet_type} packet in {} state", self.cid(), self.state.borrow());
                Ok(())
            }
        }
    }

    async fn handle_challenge(self: &Rc<Self>, challenge: packet::Challenge) -> Result<()> {
        self.handler.challenge(self, challenge.challenge());
        Ok(())
    }

    fn handle_auth_ack(self: &Rc<Self>, ack: packet::AuthAck) -> Result<()> {
        self.handler.authenticated(
            self,
            ack.server_session_pk(), ack.max_connections() as i32, ack.name_access(),
            ack.endpoint(), ack.named_endpoint(),
        );
        *self.state.borrow_mut() = State::Idling;
        self.on_opened();
        info!("Connection {} opened.", self.cid());
        Ok(())
    }

    fn handle_attach_ack(self: &Rc<Self>, _ack: packet::AttachAck) -> Result<()> {
        *self.state.borrow_mut() = State::Idling;
        self.on_opened();
        info!("Connection {} opened.", self.cid());
        Ok(())
    }

    fn handle_ping_ack(self: &Rc<Self>, _ack: packet::PingAck) -> Result<()> {
        // keep-alive timestamp is already updated on receipt of any relay data.
        Ok(())
    }

    async fn handle_connect(self: &Rc<Self>, conn: packet::Connect) -> Result<()> {
        let addr = SocketAddr::new(conn.address(), conn.port());
        if !self.allow(addr) {
            let _ = self.send_connect_ack(false).await;
            return Ok(());
        }

        *self.state.borrow_mut() = State::Connecting;

        // Reset the disconnect handshake count at the start of a new relay cycle
        // so that a DISCONNECT racing this CONNECT keeps its confirmation
        // instead of being wiped by a late callback.
        *self.disconnect_confirms.borrow_mut() = 0;

        let handler = self.handler.clone();
        let connection = self.clone();
        tokio::task::spawn_local(async move {
            handler.busy(&connection);
        });


        debug!("Connection {} connecting to the upstream...", self.cid());
        match self.open_upstream().await {
            Ok(()) => {
                debug!("Connection {} connected to the upstream", self.cid());
                self.connect_upstream();
                self.send_connect_ack(true).await
            },
            Err(e) => {
                *self.state.borrow_mut() = State::Idling;
                self.on_idle();
                error!("Connection {} failed to connect to upstream: {e}", self.cid());
                self.send_connect_ack(false).await
            }
        }
    }

    async fn handle_data(self: &Rc<Self>, data: packet::Data) -> Result<()> {
        if *self.state.borrow() != State::Relaying {
            trace!("Connection {} dropping DATA packet because the connection is not in the relaying state",
                self.cid());
            return Ok(());
        }

        let Some(mut tx) = self.upstream_tx.borrow_mut().take() else {
            return Err(StateError::new("upstream writer is not available"));
        };

        let payload = data.data();
        let mut written = 0;
        let mut close_upstream = false;
        while written < payload.len() {
            match tx.write(&payload[written..]).await {
                Ok(len) => written += len,
                Err(e) => {
                    error!("Connection {} failed to write data to upstream: {e}", self.cid());
                    close_upstream = true;
                    break;
                }
            }
        }
        *self.upstream_tx.borrow_mut() = Some(tx);

        if close_upstream {
            let _ = self.close_upstream().await;
            return Ok(());
        }

        trace!("Connection {} sent {} bytes data to upstream", self.cid(), payload.len());
        Ok(())
    }

    async fn handle_disconnect(self: &Rc<Self>, _pkt: packet::Disconnect) -> Result<()> {
        debug!("Connection {} got DISCONNECT from server", self.cid());

        // Disconnected from the client side before connecting to the upstream:
        // assume the upstream is already gone and account
        // for that leg of the handshake before sending our own DISCONNECT.
        if *self.state.borrow() == State::Connecting && self.upstream_tx.borrow().is_none() {
            self.confirm_disconnect();
            let _ = self.send_disconnect().await;
        }

        *self.state.borrow_mut() = State::Disconnecting;
        self.disconnect_upstream().await;
        self.send_disconnect_ack().await
    }

    async fn handle_disconnect_ack(self: &Rc<Self>, _pkt: packet::DisconnectAck) -> Result<()> {
        debug!("Connection {} got DISCONNECT_ACK from server", self.cid());
        self.disconnect_upstream().await;
        Ok(())
    }

    async fn disconnect_upstream(self: &Rc<Self>) {
        if let Some(mut tx) = self.upstream_tx.borrow_mut().take() {
            let _ = tx.shutdown().await;
        }
        *self.upstream_rx.borrow_mut() = None;
        self.confirm_disconnect();
    }

    // A relayed connection returns to Idling only after DISCONNECT_CONFIRMS confirmations are observed:
    // the local upstream end, the server DISCONNECT, and the matching DISCONNECT_ACK.
    fn confirm_disconnect(self: &Rc<Self>) {
        let mut disconnect_confirms = self.disconnect_confirms.borrow_mut();
        *disconnect_confirms += 1;
        if *disconnect_confirms == DISCONNECT_CONFIRMS {
            trace!("Connection {} disconnect confirmed, changing state to idle", self.cid());
            *self.state.borrow_mut() = State::Idling;
            *disconnect_confirms = 0;
            drop(disconnect_confirms);
            self.on_idle();
        }
    }

    pub(crate) async fn on_upstream_data(self: &Rc<Self>, data: &[u8]) -> Result<()> {
        if *self.state.borrow() != State::Relaying {
            trace!("Connection {} dropping data from upstream because the connection is not in the relaying state", self.cid());
            return Ok(());
        }
        self.send_data(data.to_vec()).await
    }

    pub(crate) async fn check_keepalive(self: &Rc<Self>) -> Result<()> {
        if elapsed_ms!(*self.keepalive.borrow()) >= MAX_KEEP_ALIVE_RETRY * KEEPALIVE_INTERVAL {
            warn!("Connection {} keep alive timeout, closing now", self.cid());
            return Err(StateError::new(format!("Connection {} is dead", self.cid())));
        }

        let random_shift = random_timeshift() as u128 * 1000; // max 10 seconds
        if elapsed_ms!(*self.keepalive.borrow()) >= KEEPALIVE_INTERVAL - random_shift {
            return self.send_ping().await;
        }
        Ok(())
    }

    pub(crate) async fn close_upstream(self: &Rc<Self>) -> Result<()> {
        if *self.state.borrow() == State::Closed || *self.state.borrow() == State::Idling {
            return Ok(());
        }

        info!("Connection {} closing upstream", self.cid());

        *self.state.borrow_mut() = State::Disconnecting;
        let _ = self.send_disconnect().await;
        self.disconnect_upstream().await;
        Ok(())
    }

    pub(crate) async fn close(self: &Rc<Self>) -> Result<()> {
        if *self.state.borrow() == State::Closed {
            return Ok(());
        }
        *self.state.borrow_mut() = State::Closed;

        info!("Connection {} is closing...", self.cid());

        if let Some(mut writer) = self.upstream_tx.borrow_mut().take() {
            let _ = writer.shutdown().await;
            info!("Connection {} upstream socket closed", self.cid());
        }
        *self.upstream_rx.borrow_mut() = None;

        if let Some(mut writer) = self.relay_tx.borrow_mut().take() {
            let _ = writer.shutdown().await;
            info!("Connection {} proxy socket closed", self.cid());
        }
        *self.relay_rx.borrow_mut() = None;

        self.stickybuf.borrow_mut().clear();
        self.on_closed();

        info!("Connection {} closed", self.cid());
        Ok(())
    }

    pub(crate) async fn run(self: Rc<Self>) {
        enum Action {
            Continue,
            Close,
            CloseUpstream,
        }

        enum Event {
            Relay(Result<usize>),
            Upstream(Result<usize>),
            Tick,
        }

        let mut relay_buff    = vec![0u8; 0x7FFF];
        let mut upstream_buff = vec![0u8; 0x7FFF];
        let duration = Duration::from_millis(HEALTH_CHECK_INTERVAL);
        let mut ticker = time::interval_at(Instant::now() + duration, duration);

        loop {
            let event = tokio::select! {
                rc = async {
                    let mut reader = self.relay_rx.borrow_mut();
                    let Some(stream) = reader.as_mut() else {
                        return Ok(0);
                    };
                    stream.read(&mut relay_buff).await.map_err(|e| e.into())
                }, if self.relay_rx.borrow().is_some() => {
                    Event::Relay(rc)
                },
                rc = async {
                    let mut reader = self.upstream_rx.borrow_mut();
                    let Some(stream) = reader.as_mut() else {
                        return Ok(0);
                    };
                    stream.read(&mut upstream_buff).await.map_err(|e| e.into())
                }, if self.upstream_rx.borrow().is_some() => {
                    Event::Upstream(rc)
                },
                _ = ticker.tick() => Event::Tick,
            };

            let action = match event {
                Event::Relay(rc) => match rc {
                    Err(e) => {
                        error!("Connection {} read relay stream error: {e}", self.cid());
                        Action::Close
                    },
                    Ok(0) => {
                        info!("Connection {} read EOF from relay stream", self.cid());
                        Action::Close
                    },
                    Ok(len) => {
                        if let Err(e) = self.on_relay_data(&relay_buff[..len]).await {
                            error!("Connection {} relay handling error: {e}", self.cid());
                            Action::Close
                        } else {
                            Action::Continue
                        }
                    }
                },
                Event::Upstream(rc) => match rc {
                    Err(e) => {
                        error!("Connection {} read upstream stream error: {e}", self.cid());
                        Action::CloseUpstream
                    },
                    Ok(0) => {
                        info!("Connection {} read EOF from upstream", self.cid());
                        Action::CloseUpstream
                    },
                    Ok(len) => {
                        if self.on_upstream_data(&upstream_buff[..len]).await.is_ok() {
                            Action::Continue
                        } else {
                            Action::Close
                        }
                    }
                },
                Event::Tick => {
                    if self.check_keepalive().await.is_ok() {
                        Action::Continue
                    } else {
                        Action::Close
                    }
                },
            };

            match action {
                Action::Continue if *self.state.borrow() != State::Closed => {}
                Action::Continue => continue,
                Action::Close => {
                    let _ = self.close().await;
                    break;
                }
                Action::CloseUpstream => {
                    let _ = self.close_upstream().await;
                }
            }
        }
    }
}
