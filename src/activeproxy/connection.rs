use std::mem;
use std::fmt;
use std::str;
use std::cell::RefCell;
use std::sync::{Arc, Mutex};
use std::time::SystemTime;
use std::net::{
    SocketAddr,
    IpAddr,
    Ipv4Addr,
    Ipv6Addr
};
use tokio::io::{
    split,
    ReadHalf,
    WriteHalf,
    AsyncWriteExt
};
use tokio::net::{
    TcpStream,
    TcpSocket
};
use log::{warn, error,info, debug, trace};

use crate::{
    as_millis,
    srv_endp,
    srv_addr,
    srv_peer,
    ups_endp,
    ups_addr,
    session_keypair,
    enbox,
    unwrap,
    random_bytes,
    id,
    Id,
    Node,
    Error,
    error::Result,
    cryptobox, CryptoBox,
    signature,
    Signature,
    PeerBuilder,
    Identity,
    CryptoContext
};

use crate::activeproxy::{
    random_padding,
    random_timeshift,
    random_boolean,
    managed::ManagedFields,
    packet::{Packet, AttachType, AuthType, ConnType, DisconnType, DataType, PingType},
    state::State,
};

// packet size (2bytes) + packet type(1bytes)
const PACKET_HEADER_BYTES: usize = mem::size_of::<u16>() + mem::size_of::<u8>();
const KEEPALIVE_INTERVAL:   u128 = 60000;      // 60 seconds
const MAX_KEEP_ALIVE_RETRY: u128 = 3;

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

pub(crate) struct ProxyConnection {
    node:               Arc<Mutex<Node>>,
    conn_id:            i32,
    state:              State,
    keepalive:          SystemTime,
    disconnect_confirms: i32,

    inners:             Arc<Mutex<ManagedFields>>,

    relay_reader:       Option<ReadHalf<TcpStream>>,
    relay_writer:       Option<WriteHalf<TcpStream>>,

    upstream_reader:    Option<ReadHalf<TcpStream>>,
    upstream_writer:    Option<WriteHalf<TcpStream>>,

    stickybuf:          Option<Vec<u8>>,
    nonce:              cryptobox::Nonce,

    deviceid:           Id,
    signature_keypair:  signature::KeyPair,
    crypto_context:     RefCell<CryptoContext>,

    authorized_cb:      Box<dyn Fn(&ProxyConnection, &cryptobox::PublicKey, u16, bool) + Send>,
    opened_cb:          Box<dyn Fn(&ProxyConnection) + Send>,
    open_failed_cb:     Box<dyn Fn(&ProxyConnection) + Send>,
    closed_cb:          Box<dyn Fn(&ProxyConnection) + Send>,
    busy_cb:            Box<dyn Fn(&ProxyConnection) + Send>,
    idle_cb:            Box<dyn Fn(&ProxyConnection) + Send>,
}

impl Identity for ProxyConnection {
    fn id(&self) -> &Id {
        &self.deviceid
    }

    fn sign(&self, data: &[u8], signature: &mut [u8]) -> Result<usize> {
        signature::sign(data, signature, self.signature_keypair.private_key())
    }

    fn sign_into(&self, data: &[u8]) -> Result<Vec<u8>> {
        signature::sign_into(data, self.signature_keypair.private_key())
    }

    fn verify(&self, data: &[u8], signature: &[u8]) -> Result<()> {
        signature::verify(data, signature, self.signature_keypair.public_key())
    }

    fn encrypt(&self, _recipient: &Id, plain: &[u8], cipher: &mut [u8]) -> Result<usize> {
        self.crypto_context.borrow_mut().encrypt(
            plain,
            cipher
        )
    }

    fn decrypt(&self, _sender: &Id, cipher: &[u8], plain: &mut [u8]) -> Result<usize> {
        self.crypto_context.borrow_mut().decrypt(
            cipher,
            plain
        )
    }

    fn encrypt_into(&self, _recipient: &Id, data: &[u8]) -> Result<Vec<u8>> {
        self.crypto_context.borrow_mut().encrypt_into(data)
    }

    fn decrypt_into(&self, _sender: &Id, cipher: &[u8]) -> Result<Vec<u8>> {
        self.crypto_context.borrow_mut().decrypt_into(cipher)
    }

    fn create_crypto_context(&self, _id: &Id) -> Result<CryptoContext> {
        unimplemented!()
    }

}

impl ProxyConnection {
    pub(crate) fn new(node: Arc<Mutex<Node>>, inners: Arc<Mutex<ManagedFields>>) -> Self {
        let keypair = signature::KeyPair::random();
        let encryption_keypair = cryptobox::KeyPair::from(&keypair);

        let mut connection = Self {
            node,
            inners:             inners.clone(),

            conn_id:            next_connection_id(),
            state:              State::Initializing,
            keepalive:          SystemTime::now(),
            disconnect_confirms: 0,

            relay_reader:       None,
            relay_writer:       None,
            upstream_reader:    None,
            upstream_writer:    None,

            stickybuf:          Some(Vec::with_capacity(4*1024)),
            nonce:              cryptobox::Nonce::random(),

            deviceid:           Id::from(keypair.to_public_key()),
            signature_keypair:  keypair,
            crypto_context:     RefCell::new(CryptoContext::new(
                srv_peer!(inners).lock().unwrap().id(),
                encryption_keypair.private_key()
            )),

            authorized_cb:      Box::new(|_,_,_,_|{}),
            opened_cb:          Box::new(|_|{}),
            open_failed_cb:     Box::new(|_|{}),
            closed_cb:          Box::new(|_|{}),
            busy_cb:            Box::new(|_|{}),
            idle_cb:            Box::new(|_|{}),
        };

        connection.authorized_cb = Box::new(move |conn, pk, port, domain_enabled| {
            let mut inners = conn.inners.lock().unwrap();
            let sk = unwrap!(inners.session_keypair).private_key().clone();

            inners.relay_port = Some(port);
            inners.cryptobox  = cryptobox::CryptoBox::try_from((pk, &sk)).ok();
            inners.domain_enabled = domain_enabled;

            if inners.peer_keypair.is_none() {
                return;
            }

            let peer = PeerBuilder::new(unwrap!(inners.remote_node).lock().unwrap().id())
                .with_keypair(inners.peer_keypair.as_ref())
                .with_alternative_url(inners.peer_domain.as_ref().map(|v|v.as_str()))
                .with_port(port)
                .build();

            if let Some(url) = peer.alternative_url() {
                info!("-**- ActiveProxy: peer server: {}:{}, domain: {} -**-",
                    unwrap!(inners.remote_addr).ip(),
                    peer.port(),
                    url
                );
            } else {
                info!("-**- ActiveProxy: peer server: {}:{} -**-",
                    unwrap!(inners.remote_addr).ip(),
                    peer.port()
                );
            }
            inners.peer = Some(peer);
        });

        connection.opened_cb = Box::new(move |conn| {
            let mut inners = conn.inners.lock().unwrap();
            inners.server_failures = 0;
            inners.reconnect_delay = 0;
        });

        connection.open_failed_cb = Box::new(move|conn| {
            let mut inners = conn.inners.lock().unwrap();
            let failures = inners.server_failures;

            inners.server_failures = failures + 1;
            if inners.reconnect_delay < 64 {
                inners.reconnect_delay = (1 << failures) * 1000;
            }
        });

        connection.closed_cb = Box::new(move |conn| {
            conn.inners.lock().unwrap().connections -= 1;
        });

        connection.busy_cb = Box::new(move |conn| {
            let mut inners = conn.inners.lock().unwrap();
            inners.inflights += 1;
            inners.last_idle_check = SystemTime::UNIX_EPOCH;
        });

        connection.idle_cb = Box::new(move |conn| {
            let mut inners = conn.inners.lock().unwrap();
            inners.inflights -= 1;
            if inners.inflights == 0 {
                inners.last_idle_check = SystemTime::now();
            }
        });

        info!("Connection {} is created.", connection.id());
        connection
    }

    pub(crate) fn cid(&self) -> i32 {
        self.conn_id
    }

    pub(crate) fn inners(&self) -> Arc<Mutex<ManagedFields>> {
        self.inners.clone()
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

    fn stickybuf_mut(&mut self) -> &mut Vec<u8> {
        self.stickybuf.as_mut().unwrap()
    }

    fn stickybuf(&self) -> &[u8] {
        self.stickybuf.as_ref().unwrap()
    }

    fn allow(&self, _: &SocketAddr) -> bool {
        true
    }

    fn on_authorized(&mut self, pk: &cryptobox::PublicKey, port: u16, domain_enabled: bool) {
        (self.authorized_cb)(self, pk, port, domain_enabled);
    }

    fn on_opened(&mut self) {
        (self.opened_cb)(self);
    }

    fn on_closed(&mut self) {
        (self.closed_cb)(self);
    }

    fn on_open_failed(&mut self) {
        (self.open_failed_cb)(self)
    }

    fn on_busy(&mut self) {
        (self.busy_cb)(self);
    }

    fn on_idle(&mut self) {
        (self.idle_cb)(self);
    }

    pub(crate) async fn close(&mut self) -> Result<()> {
        if self.state == State::Closed {
            return Ok(())
        }

        let old_state = self.state.clone();
        self.state = State::Closed;

        info!("Connection {} is closing...", self.cid());

        if old_state <= State::Attaching {
            self.on_open_failed();
        }
        if old_state == State::Relaying {
            self.on_idle();
        }

        let reader = self.relay_reader.take();
        let writer = self.relay_writer.take();

        assert!(reader.is_some());
        assert!(writer.is_some());

        let mut stream = reader.unwrap().unsplit(writer.unwrap());
        _ = tokio::spawn(async move {
            _ = stream.flush().await;
            _ = stream.shutdown().await;
        }).await;

        let reader = self.upstream_reader.take();
        let writer = self.upstream_writer.take();

        if let Some(reader) = reader {
            assert!(writer.is_some());

            let mut stream = reader.unsplit(writer.unwrap());
            _ = tokio::spawn(async move {
                _ = stream.flush().await;
                _ = stream.shutdown().await;
            }).await
        }

        self.on_closed();

        info!("Connection {} is closed...", self.cid());
        Ok(())
    }

    async fn open_upstream(&mut self) -> Result<()> {
        debug!("Connection {} connecting to upstream {}...", self.cid(), ups_endp!(self.inners));

        let raddr = ups_addr!(self.inners).clone();
        let socket = TcpSocket::new_v4()?;  // TODO: ip v4 addr?;
        let result = socket.connect(raddr).await;
        match result {
            Ok(stream) => {
                info!("Connection {} has connected to upstream {}", self.cid(), ups_endp!(self.inners));
                let (reader, writer) = split(stream);
                self.upstream_reader = Some(reader);
                self.upstream_writer = Some(writer);
            },
            Err(e) => {
                error!("Connection {} connect to upstream {} failed: {}", self.cid(), ups_endp!(self.inners), e);
                self.close_upstream2().await?;
                self.state = State::Idling;
                self.on_idle();
            }
        };

        if self.upstream_reader.is_some() {
            self.send_connect_response(true).await
        } else {
            self.send_connect_response(false).await
        }
    }

    async fn close_upstream2(&mut self) -> Result<()> {
        if  self.state == State::Closed ||
            self.state == State::Idling {
            return Ok(())
        }

        let reader = self.upstream_reader.take();
        let writer = self.upstream_writer.take();

        if let Some(reader) = reader {
            assert!(writer.is_some());
            let mut stream = reader.unsplit(writer.unwrap());
            _ = tokio::spawn(async move {
                _ = stream.flush().await;
                _ = stream.shutdown().await;
            }).await
        }

        info!("Connection {} closed upstream {}", self.cid(), ups_endp!(self.inners));
        Ok(())
    }

    pub(crate) async fn close_upstream(&mut self) -> Result<()> {
        if  self.state == State::Closed ||
            self.state == State::Idling  {
            return Ok(())
        }

        info!("Connection {} closing upstream {}", self.cid(), ups_endp!(self.inners));

        self.state = State::Disconnecting;

        _ = self.send_disconnect_request().await;
        _ = self.close_upstream2().await;

        Ok(())
    }

    pub(crate) async fn check_keepalive(&mut self) -> Result<()> {
        if self.state == State::Relaying {
            return Ok(())
        }

        if as_millis!(self.keepalive) > MAX_KEEP_ALIVE_RETRY * KEEPALIVE_INTERVAL {
            warn!("Connection {} is dead and should be obsolete.", self.cid());
            return Err(Error::State(format!("Connection {} is dead", self.cid())));
        }

        // keepalive check.
        let random_shift = random_timeshift() as u128; // max  10 seconds;
        if self.state == State::Idling &&
            as_millis!(self.keepalive) >= KEEPALIVE_INTERVAL - random_shift {
            return self.send_ping_request().await;
        }
        return Ok(())
    }

    pub(crate) async fn connect_server(&mut self) -> Result<()> {
        info!("Connection {} is connecting to the server {}...", self.cid(), srv_endp!(self.inners));

        let raddr = srv_addr!(self.inners).clone();
        let socket = TcpSocket::new_v4()?;  // TODO: ip v4 addr?;
        let result = socket.connect(raddr).await;
        match result {
            Ok(stream) => {
                info!("Connection {} has connected to server {}", self.cid(), srv_endp!(self.inners));

                let (reader, writer) = split(stream);
                self.relay_reader = Some(reader);
                self.relay_writer = Some(writer);
                Ok(())
            },
            Err(e) => {
                error!("Connection {} connect to server {} failed: {}", self.cid(), srv_endp!(self.inners), e);
                Err(Error::from(e))
            }
        }
    }

    pub(crate) async fn on_relay_data(&mut self, input: &[u8]) -> Result<()> {
        self.keepalive = SystemTime::now();

        let mut pos = 0;
        let mut remain = input.len();
        if self.stickybuf_mut().len() > 0 {
            if self.stickybuf().len() < PACKET_HEADER_BYTES {
                let rs = PACKET_HEADER_BYTES - self.stickybuf().len();
                //  Read header data, but insufficient to form a complete header
                if remain < rs {
                    self.stickybuf_mut().extend_from_slice(input);
                    return Ok(());
                }

                // A complete packet header has been read.
                self.stickybuf_mut().extend_from_slice(&input[..rs]);
                pos += rs;
                remain -= rs;
            }

            // Parse the header to determine packet size.
            let packet_sz = u16::from_be_bytes(
                    self.stickybuf()[..size_of::<u16>()].try_into().unwrap()
                ) as usize;
            let rs = packet_sz - self.stickybuf().len();
            if remain < rs {
                // Reader packet data but insufficient to form a complete packet
                self.stickybuf_mut().extend_from_slice(&input[pos..pos+remain]);
                return Ok(());
            }

            // A complete packet has been successfully read.
            self.stickybuf_mut().extend_from_slice(&input[pos..pos+rs]);
            pos += rs;
            remain -= rs;

            let stickybuf = self.stickybuf.take().unwrap();
            self.stickybuf = Some(Vec::with_capacity(4*1024));
            self.process_relay_packet(&stickybuf).await?;
        }

        // Continue parsing the remaining data from input buffer.
        while remain > 0 {
            // clean sticky buffer to prepare for new packet.
            if remain < PACKET_HEADER_BYTES {
                self.stickybuf_mut().extend_from_slice(&input[pos..pos + remain]);
                return Ok(())
            }

            let packet_sz = u16::from_be_bytes(input[pos..pos+size_of::<u16>()].try_into().unwrap()) as usize;
            if remain < packet_sz {
                // Reader packet data but insufficient to form a complete packet
                self.stickybuf_mut().extend_from_slice(&input[pos..pos+remain]);
                return Ok(())
            }

            self.process_relay_packet(&input[pos..pos+packet_sz]).await?;
            pos += packet_sz;
            remain -= packet_sz;
        }
        Ok(())
    }

    async fn process_relay_packet(&mut self, input: &[u8]) -> Result<()> {
        let pos = mem::size_of::<u16>();
        if self.state == State::Initializing {
            return self.on_challenge(&input[pos..]).await;
        }

        // packet format
        // - u16: packet size,
        // - u8: packet flag.
        let result = Packet::from(input[pos]);
        if let Err(e) = result {
            error!("Received an invalid packet type: {}", e);
            return Err(e);
        }

        let packet = result.unwrap();
        debug!("Connection {} got packet from server {}: type={}, ack={}, size={}",
            self.cid(), srv_endp!(self.inners), packet, packet.ack(), input.len());

        if matches!(packet, Packet::Error(_)) {
            let len = input.len() - PACKET_HEADER_BYTES;
            let mut plain = vec![0u8; len];
            _ = enbox!(self.inners).decrypt(
                &input[PACKET_HEADER_BYTES..],
                &mut plain[..]
            ).map_err(|e| {
                error!("Connection {} decrypt packet from server {} error {e}",
                    self.cid(),
                    srv_endp!(self.inners)
                ); e
            })?;

            let mut pos = 0;
            let end = mem::size_of::<u16>();
            let ecode = u16::from_be_bytes(plain[pos..end].try_into().unwrap());

            pos = end;
            let data = &plain[pos..];
            let errstr = str::from_utf8(data).unwrap().to_string();

            error!("Connection {} got ERR response from the server {}, error:{}:{}",
                self.cid(), srv_endp!(self.inners), ecode, errstr);

            return Err(Error::Protocol(format!("Packet error")));
        }

        if !self.state.accept(&packet) {
            error!("Connection {} is not allowed for {} packet at {} state", self.cid(), packet, self.state);
            return Err(Error::Permission(format!("Permission denied")));
        }

        match packet {
            Packet::AuthAck(_)      => self.on_authenticate_response(input),
            Packet::AttachAck(_)    => self.on_attach_reponse(input),
            Packet::PingAck(_)      => self.on_ping_response(input),
            Packet::Connect(_)      => self.on_connect_request(input).await,
            Packet::Data(_)         => self.on_data_request(input).await,
            Packet::Disconnect(_)   => self.on_disconnect_request(input).await,
            Packet::DisconnectAck(_)=> self.on_disconnect_response(input),
            _ => {
                error!("INTERNAL ERROR: Connection {} got wrong {} packet in {} state", self.cid(), packet, self.state);
                Err(Error::Protocol(format!("Wrong expected packet {} received", packet)))
            }
        }
    }

    /*
    * Challenge packet
    * - plain
    *   - Random challenge bytes.
    */
    async fn on_challenge(&mut self, input: &[u8]) -> Result<()> {
        if input.len() < 32 || input.len() > 256 {
            error!("Connection {} got invalid challenge from server {}, expected range {}:{}, acutal length:{}!",
                self.cid(),
                srv_endp!(self.inners),
                32,
                256,

                input.len()
            );
            return Ok(())
        }

        // Sign the challenge, send auth or attach with siguature
        let sig = self.sign_into(input)?;
        // TODO: device signature
        if self.inners.lock().unwrap().is_authenticated() {
            self.send_attach_request(&sig).await
        } else {
            self.send_authenticate_request(&sig, &sig).await
        }
    }

    /*
    * AUTHACK packet payload:
    * - encrypted
    *   - sessionPk[server]
    *   - port[uint16]
    *   - domainEnabled[uint8]
    */
    const AUTH_ACK_SIZE: usize = PACKET_HEADER_BYTES    // header.
        + cryptobox::CryptoBox::MAC_BYTES               // MAC BYTES.
        + cryptobox::PublicKey::BYTES                   // public key.
        + mem::size_of::<u16>()                         // port.
        + mem::size_of::<u16>()                         // max connections allowed.
        + mem::size_of::<bool>();

    fn on_authenticate_response(&mut self, input: &[u8]) -> Result<()> {
        if input.len() < Self::AUTH_ACK_SIZE {
            error!("Connection {} got invalid AUTH ACK from server {}, expected minimum length {}, actual found: {}",
                self.cid(),
                srv_endp!(self.inners),
                Self::AUTH_ACK_SIZE,
                input.len()
            );
            return Err(Error::Protocol(format!("Invalid AUTH ACK packet")));
        }

        debug!("Connection {} got AUTH ACK from server {}", self.cid(), srv_endp!(self.inners));

        let plain_len = Self::AUTH_ACK_SIZE - PACKET_HEADER_BYTES - CryptoBox::MAC_BYTES;
        let mut plain = vec![0u8; plain_len];

        _ = self.decrypt(
            srv_peer!(self.inners).lock().unwrap().id(),
            &input[PACKET_HEADER_BYTES..Self::AUTH_ACK_SIZE],
            &mut plain[..]
        ).map_err(|e| {
            error!("Connection {} decrypt AUTH ACK from server {} error {e}.",
                self.cid(),
                srv_endp!(self.inners())
            ); e
        })?;

        let mut pos = 0;
        let mut end = pos + cryptobox::PublicKey::BYTES;
        let server_pk = cryptobox::PublicKey::try_from( // extract server public key.
            &plain[pos..end]
        )?;

        pos = end;
        end += mem::size_of::<u16>();
        let port = u16::from_be_bytes(                  // extract port.
            plain[pos..end].try_into().unwrap()
        );

        pos = end;
        end += mem::size_of::<u16>();
        let max_connections = u16::from_be_bytes(       // extract max connections allowed
            plain[pos..end].try_into().unwrap()
        ) as usize;

        self.inners.lock().unwrap().capacity = max_connections;

        pos = end;
        let domain_enabled = input[pos] != 0;           // extract flag whether domain enabled or not.

        self.on_authorized(&server_pk, port, domain_enabled);

        self.state = State::Idling;
        self.on_opened();
        info!("Connection {} opened.", self.cid());
        Ok(())
    }

    /*
     * No Payload.
     */
    fn on_attach_reponse(&mut self, _input: &[u8]) -> Result<()> {
        debug!("Connection {} got ATTACH ACK from server {}", self.cid(), srv_endp!(self.inners));
        self.state = State::Idling;
        self.on_opened();
        info!("Connection {} opened.", self.cid());
        Ok(())
    }

    /*
     * No Payload.
     */
    fn on_ping_response(&mut self, _input: &[u8]) -> Result<()> {
        debug!("Connection {} got PING ACK from server {}", self.cid(), srv_endp!(self.inners));
        // ignore the random padding payload.
        // keep-alive time stamp already update when we got the server data.
        // so nothing to do here.
        Ok(())
    }

    const CONNECT_REQ_SIZE: usize = PACKET_HEADER_BYTES
        + CryptoBox::MAC_BYTES
        + mem::size_of::<u8>()
        + 16
        + mem::size_of::<u16>();

    /*
     * CONNECT packet payload:
     * - encrypted
     *   - addrlen[uint8]
     *   - addr[16 bytes both for IPv4 or IPv6]
     *   - port[uint16]
     */
    async fn on_connect_request(&mut self, input: &[u8]) -> Result<()> {
        if input.len() < Self::CONNECT_REQ_SIZE {
            error!("Connection {} got invalid CONNECT request from server {}, expected length: {}, acutal length:{}",
                self.cid(),
                srv_endp!(self.inners),
                Self::CONNECT_REQ_SIZE,
                input.len()
            );
            return Err(Error::Protocol(format!("Invalid CONNECT packet")));
        }

        debug!("Connection {} got CONNECT from server {}", self.cid(), srv_endp!(self.inners));
        self.state = State::Relaying;
        self.on_busy();

        let plain_len = Self::CONNECT_REQ_SIZE - PACKET_HEADER_BYTES - CryptoBox::MAC_BYTES;  // TODO:
        let mut plain = vec![0u8; plain_len];
        _ = enbox!(self.inners).decrypt(
            &input[PACKET_HEADER_BYTES..Self::CONNECT_REQ_SIZE],
            &mut plain[..]
        ).map_err(|e| {
            error!("Connection {} decrypt CONNECT request packet from server {} error: {e}",
                self.cid(),
                srv_endp!(self.inners)
            ); e
        })?;

        let mut pos = 0;
        let addr_len = plain[pos] as usize;

        pos += mem::size_of::<u8>();
        let ip = match (addr_len * 8) as u32 {
            Ipv4Addr::BITS => {
                let bytes = input[pos..pos + addr_len].try_into().unwrap();
                let bits = u32::from_be_bytes(bytes);
                IpAddr::V4(Ipv4Addr::from(bits))
            },
            Ipv6Addr::BITS => {
                let bytes = input[pos..pos + addr_len].try_into().unwrap();
                let bits = u128::from_be_bytes(bytes);
                IpAddr::V6(Ipv6Addr::from(bits))
            },
            _ => return Err(Error::Protocol(format!("Unsupported address family."))),
        };

        pos += 16;      // the length of the buffer for address.
        let end = pos + mem::size_of::<u16>();
        let port = u16::from_be_bytes(input[pos..end].try_into().unwrap());
        let addr = SocketAddr::new(ip, port);

        if self.allow(&addr) {
            self.open_upstream().await
        } else {
            self.send_connect_response(false).await?;
            self.state = State::Idling;
            self.on_idle();
            Ok(())
        }
    }

    /*
     * DATA packet payload:
     * - encrypted
     *   - data
     */
    async fn on_data_request(&mut self, input: &[u8]) -> Result<()> {
        debug!("Connection {} got DATA({}) from server {}", self.cid(), input.len(), srv_endp!(self.inners));

        let plain_len = input.len() - PACKET_HEADER_BYTES - CryptoBox::MAC_BYTES;
        let mut data = Box::new(vec![0u8; plain_len]);

        _ = enbox!(self.inners).decrypt(
            &input[PACKET_HEADER_BYTES..],
            &mut data[..]
        ).map_err(|e| {
            error!("Connection {} decrypt DATA packet from server {} error : {e}",
                self.cid(),
                srv_endp!(self.inners)
            ); e
        })?;

        trace!("Connection {} sending {} bytes data to upstream {}",
            self.cid(),
            data.len(),
            ups_endp!(self.inners)
        );

        let mut written = 0;
        while written < data.len() {
            let slen = match self.upstream_writer.as_mut().unwrap().write(&data[written..]).await {
                Ok(len) => len,
                Err(e) => {
                    error!("Connection {} send DATA to upstream {} error: {e}",
                        self.cid(),
                        srv_endp!(self.inners)
                    );
                    return Err(Error::from(e))
                }
            };
            written += slen;
        }

        debug!("Connection {} sended DATA (len:{}) to upstream {}.",
            self.cid(),
            data.len(),
            ups_endp!(self.inners)
        );

        Ok(())
    }

    /*
     * No payload
     */
    async fn on_disconnect_request(&mut self, _input: &[u8]) -> Result<()> {
        debug!("Connection {} got DISCONNECT from server {}", self.cid(), srv_endp!(self.inners));

        _ = self.close_upstream();
        _ = self.send_disconnect_response().await?;

        self.disconnect_confirms += 1;
        if self.disconnect_confirms == 2 {
            self.disconnect_confirms = 0;
            self.state = State::Idling;
            self.on_idle();
        }
        Ok(())
    }

    /*
    * No payload
    */
    fn on_disconnect_response(&mut self, _input: &[u8]) -> Result<()> {
        debug!("Connection {} got DISCONNECT_ACK from server {}", self.cid(), srv_endp!(self.inners));

        self.disconnect_confirms += 1;
        if self.disconnect_confirms == 2 {
            self.disconnect_confirms = 0;
            self.state = State::Idling;
            self.on_idle();
        }
        Ok(())
    }

    /*
    * ATTACH packet:
    *   - plain
    *     - clientNodeId
    *   - encrypted
    *     - sessionPk[client]
    *     - connectionNonce
    *     - signature[challenge]
    *   - plain
    *     - padding
    */
    async fn send_attach_request(&mut self, dev_sig: &[u8]) -> Result<()> {
        assert!(dev_sig.len() == Signature::BYTES);
        if self.state == State::Closed {
            return Ok(())
        }

        self.state = State::Attaching;

        let len = Signature::BYTES;                 // signature of challenge.
        let mut plain:Vec<u8> = Vec::with_capacity(len);
        plain.extend_from_slice(dev_sig);           // signature of challenge.

        let len = id::ID_BYTES                      // plain device id
            + cryptobox::Nonce::BYTES  + cryptobox::CryptoBox::MAC_BYTES // encryption padding of nonce + MAC
            + plain.len();

        let mut payload =vec![0u8;len];
        payload[..id::ID_BYTES].copy_from_slice(self.node.lock().unwrap().id().as_bytes());
        self.encrypt(
            srv_peer!(self.inners).lock().unwrap().id(),
            &plain,
            &mut payload[id::ID_BYTES..]
        ).map_err(|e| {
            error!("Connection {} failed to encrypt attach request: {e}", self.cid());
            e
        })?;

        self.send_relay_packet(
            Packet::Attach(AttachType),
            Some(&payload)
        ).await
    }


    /*  PACKET_HEADER_BYTES                 // header.
        ID BYTES                            // device id.
        cryptobox::Nonce::BYTES             // nonce.
        cryptobox::CryptoBox::MAC_BYTES     // MAC BYTES.
        ID BYTES                            // client user id.
        cryptobox::PublicKey::BYTES         // session public key
        mem::size_of::<8>()                 // domain size.
        Signature::BYTES                    // signature of challenge from user client.
        Signature::BYTES                    // signature of challenge from device node.
    */
    async fn send_authenticate_request(&mut self, user_sig: &[u8], dev_sig: &[u8]) -> Result<()> {
        assert!(user_sig.len() == Signature::BYTES);
        assert!(dev_sig.len() == Signature::BYTES);

        if self.state == State::Closed {
            return Ok(())
        }

        self.state = State::Authenticating;

        //let domain_len = self.inners.lock().unwrap().peer_domain.as_ref().map_or(0, |v|v.len());
        //let padding_sz = (random_padding() % 256) as usize;

        let len = id::ID_BYTES                  // client user id.
            + cryptobox::PublicKey::BYTES       // client public key.
            + Signature::BYTES                  // signature of challenge from user client.
            + Signature::BYTES                  // signature of challenge from device node.
            + mem::size_of::<u8>();              // the value to domain length.
           // + domain_len                        // domain string.
           // + padding_sz;

        let mut plain = Vec::with_capacity(len);

        plain.extend_from_slice(Id::random().as_bytes());     // userid
        plain.extend_from_slice(session_keypair!(self.inners).public_key().as_bytes());// client public key
        plain.extend_from_slice(&[false as u8]);                // boolean for domain DNS
        plain.extend_from_slice(user_sig);              // signature of challenge.
        plain.extend_from_slice(dev_sig);               // signature of challenge.
        // TODO: Random padding

        let len = id::ID_BYTES                          // plain device id
            + cryptobox::Nonce::BYTES  + cryptobox::CryptoBox::MAC_BYTES // encryption padding of nonce + MAC
            + plain.len();                              // encyption payload

        let mut payload =vec![0u8;len];
        payload[..id::ID_BYTES].copy_from_slice(self.deviceid.as_bytes());
        self.encrypt(
            srv_peer!(self.inners).lock().unwrap().id(),
            &plain,
            &mut payload[id::ID_BYTES..]
        ).map_err(|e| {
            error!("Connection {} failed to encrypt authentication request: {e}", self.cid());
            e
        })?;

        self.send_relay_packet(
            Packet::Auth(AuthType),
            Some(&payload)
        ).await
    }

    /*
     * PING packet:
     *   - plain
     *     - padding
     */
    async fn send_ping_request(&mut self) -> Result<()> {
        if self.state == State::Closed {
            return Ok(())
        }

        self.send_relay_packet(
            Packet::Ping(PingType),
            None
        ).await
    }

    /*
     * CONNECTACK packet payload:
     * - plain
     *   - success[uint8]
     *   - padding
     */
    async fn send_connect_response(&mut self, success: bool) -> Result<()> {
        let data = random_boolean(success);
        self.send_relay_packet(
            Packet::ConnectAck(ConnType),
            Some(&[data])
        ).await
    }

    /*
     * DISCONNECT packet:
     *   - plain
     *     - padding
     */
    async fn send_disconnect_request(&mut self) -> Result<()> {
        if self.state == State::Closed {
            return Ok(())
        }

        self.send_relay_packet(
            Packet::Disconnect(DisconnType),
            None
        ).await
    }

    /*
     * DISCONNECT packet:
     *   - plain
     *     - padding
     */
    async fn send_disconnect_response(&mut self) -> Result<()> {
        if self.state == State::Closed {
            return Ok(())
        }

        self.send_relay_packet(
            Packet::DisconnectAck(DisconnType),
            None
        ).await
    }

    async fn send_relay_packet(&mut self,
        pkt: Packet,
        input: Option<&[u8]>
    ) -> Result<()> {
        if self.state == State::Closed {
            warn!("Connection {} is already closed, but still try to send {} to upstream.", self.cid(), pkt);
            return Ok(());
        }

        let mut sz: u16 = (PACKET_HEADER_BYTES + input.map_or(0, |v|v.len())) as u16;
        let mut padding_sz = 0;
        if !(matches!(pkt, Packet::Auth(_)) ||
            matches!(pkt, Packet::Data(_))  ||
            matches!(pkt, Packet::Error(_))) {

            padding_sz = random_padding() as usize;
            if padding_sz == 0 {
                padding_sz += 1;
            }

            sz += padding_sz as u16
        }

        let len = PACKET_HEADER_BYTES               // packet header.
             + input.map_or(0, |v|v.len())          // packet payload.
             + padding_sz as usize;                 // padding size for randomness.

        let mut data = Vec::with_capacity(len);
        data.extend_from_slice(&sz.to_be_bytes());   // packet size.
        data.extend_from_slice(&[pkt.value()]);      // packet flag.
        if let Some(payload) = input.as_ref() {
            data.extend_from_slice(payload)          // packet payload
        }
        if padding_sz > 0 {
            let padding = random_bytes(padding_sz); // padding
            data.extend_from_slice(&padding)
        }

        let mut written = 0;
        while written < data.len() {
            let slen = match self.relay_writer.as_mut().unwrap().write(&data[written..]).await {
                Ok(len) => len,
                Err(e) => {
                    error!("Connection {} failed to send {} to server {} with error: {e}",
                        self.cid(),
                        pkt,
                        srv_endp!(self.inners)
                    );
                    return Err(Error::from(e))
                }
            };
            written += slen;
        };

        debug!("Connection {} send {}(len:{}) to server {}. ",
            self.cid(),
            pkt,
            data.len(),
            srv_endp!(self.inners)
        );
        Ok(())
    }

    pub(crate) async fn on_upstream_data(&mut self, input: &[u8]) -> Result<()> {
        let len = id::ID_BYTES                          // plain device id
            + cryptobox::Nonce::BYTES  + cryptobox::CryptoBox::MAC_BYTES // encryption padding of nonce + MAC
            + input.len();

        let mut cipher = vec![0u8; len];
        _ = enbox!(self.inners).encrypt(
            &input[..],
            &mut cipher[..],
            &self.nonce
        ).map_err(|e| {
            error!("Connection {} encrypt DATA packet to server {} error: {e}",
                self.cid(),
                srv_endp!(self.inners)
            ); e
        })?;

        self.send_relay_packet(
            Packet::Data(DataType),
            Some(&cipher[..]),
        ).await
    }
}



impl fmt::Display for ProxyConnection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Connection[{}]: state={}", self.cid(), self.state)?;
        Ok(())
    }
}
