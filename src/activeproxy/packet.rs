// Ported from Packet.java; keep the wire layout and logic in sync with the original source.
use std::mem;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

use crate::{
    Id,
    Result,
    Identity,
    CryptoContext,
    Signature,
    cryptobox,
    signature,
    core::errors::MalformedError,
};

use super::packet_type::{
    PacketType,
    AuthType,
    AttachType,
    PingType,
    ConnType,
    DisconnType,
    DataType,
    ErrType,
};

pub(crate) const VERSION: i32 = 1;
const HEADER_BYTES: usize = mem::size_of::<u16>() + mem::size_of::<u8>();

pub(crate) struct Challenge {
    challenge: Vec<u8>,
}

impl Challenge {
    pub(crate) const MIN_BYTES: usize = mem::size_of::<u16>() + 32;

    pub(crate) fn new(challenge: Vec<u8>) -> Self {
        Self { challenge }
    }

    pub(crate) fn challenge(&self) -> &[u8] {
        &self.challenge
    }

    pub(crate) fn encode(&self) -> Vec<u8> {
        let size = Self::MIN_BYTES + self.challenge.len() - 32;

        let mut packet = Vec::with_capacity(size);
        packet.extend_from_slice(&(size as u16).to_be_bytes());
        packet.extend_from_slice(&self.challenge);
        packet
    }

    pub(crate) fn decode(packet: &[u8]) -> Result<Self> {
        if packet.len() < Self::MIN_BYTES {
            return Err(MalformedError::new("packet too short"));
        }

        let size = u16::from_be_bytes([packet[0], packet[1]]) as usize;
        if size != packet.len() {
            return Err(MalformedError::new("packet size mismatch"));
        }

        let challenge = packet[mem::size_of::<u16>()..].to_vec();
        Ok(Self {challenge})
    }
}

/*
 * AUTH packet payload:
 *   - plain: deviceId
 *   - encrypted:
 *      - version (short),
 *      - userId,
 *      - clientSessionPk,
 *      - nameAccess,
 *      - deviceSig[challenge],
 *      - padding
 */
pub(crate) struct Auth {
    version:            u16,
    user_id:            Id,
    device_id:          Id,
    client_session_pk:  cryptobox::PublicKey,
    name_access:        bool,
    device_sig:         Vec<u8>,
}

#[allow(dead_code)]
impl Auth {
    const SECRET_BYTES: usize = mem::size_of::<u16>()
            + Id::BYTES
            + cryptobox::PublicKey::BYTES
            + mem::size_of::<u8>()
            + Signature::BYTES;

    pub(crate) const BYTES: usize = HEADER_BYTES                        // packet header
            + Id::BYTES                                                 // device Id
            + cryptobox::Nonce::BYTES + cryptobox::CryptoBox::MAC_BYTES // encryption header
            + Self::SECRET_BYTES;

    pub(crate) fn new(
        version: u16,
        user_id: Id,
        device_id: Id,
        client_session_pk: cryptobox::PublicKey,
        name_access: bool,
        device_sig: Vec<u8>,
    ) -> Self {
        Self {
            version,
            user_id,
            device_id,
            client_session_pk,
            name_access,
            device_sig
        }
    }

    pub(crate) fn version(&self) -> u16 {
        self.version
    }

    pub(crate) fn user_id(&self) -> &Id {
        &self.user_id
    }

    pub(crate) fn device_id(&self) -> &Id {
        &self.device_id
    }

    pub(crate) fn client_session_pk(&self) -> &cryptobox::PublicKey {
        &self.client_session_pk
    }

    pub(crate) fn name_access(&self) -> bool {
        self.name_access
    }

    pub(crate) fn device_sig(&self) -> &[u8] {
        &self.device_sig
    }

    pub(crate) fn encode(&self, crypto_context: &mut CryptoContext) -> Result<Vec<u8>> {
        let padding = random_padding(Self::BYTES);
        let mut secret = vec![0u8; Self::SECRET_BYTES + padding.len()];

        let mut pos = 0;
        secret[pos..pos + 2].copy_from_slice(&self.version.to_be_bytes());
        pos += 2;

        secret[pos..pos + Id::BYTES].copy_from_slice(&self.user_id);
        pos += Id::BYTES;

        secret[pos..pos + cryptobox::PublicKey::BYTES].copy_from_slice(self.client_session_pk.as_bytes());
        pos += cryptobox::PublicKey::BYTES;

        secret[pos] = self.name_access as u8;
        pos += 1;

        secret[pos..pos + self.device_sig.len()].copy_from_slice(&self.device_sig);
        pos += self.device_sig.len();

        secret[pos..].copy_from_slice(&padding);

        let cipher = crypto_context.encrypt_into(&secret)?;

        let size = Self::BYTES + padding.len();
        let mut packet = Vec::with_capacity(size);
        packet.extend_from_slice(&(size as u16).to_be_bytes());
        packet.push(PacketType::Auth(AuthType::default()).value());
        packet.extend_from_slice(&self.device_id);
        packet.extend_from_slice(&cipher);
        Ok(packet)
    }

    pub(crate) fn verify(&self, challenge: &[u8]) -> Result<bool> {
        signature::verify(
            challenge,
            &self.device_sig,
            &self.device_id.to_signature_key()
        )
    }

    pub(crate) fn decode(packet: &[u8], identity: &dyn Identity) -> Result<Self> {
        if packet.len() < Self::BYTES {
            return Err(MalformedError::new("packet too short"));
        }

        let mut pos = HEADER_BYTES;
        let device_id = Id::try_from_bytes(&packet[pos..pos + Id::BYTES])?;
        pos += Id::BYTES;

        let cipher = &packet[pos..];
        let secret = identity.decrypt_into(&device_id, cipher)
            .map_err(|e| MalformedError::new(format!("failed to decrypt packet: {e}")))?;

        if secret.len() < Self::SECRET_BYTES {
            return Err(MalformedError::new("AUTH payload too short"));
        }

        let mut pos = 0;
        let version = u16::from_be_bytes([secret[pos], secret[pos + 1]]);
        pos += 2;

        let user_id = Id::try_from_bytes(&secret[pos..pos + Id::BYTES])?;
        pos += Id::BYTES;

        let client_session_pk = cryptobox::PublicKey::try_from(
            &secret[pos..pos + cryptobox::PublicKey::BYTES]
        )?;
        pos += cryptobox::PublicKey::BYTES;

        let name_access = secret[pos] == 1;
        pos += 1;

        let device_sig = secret[pos..pos + Signature::BYTES].to_vec();

        Ok(Self {
            version,
            user_id,
            device_id,
            client_session_pk,
            name_access,
            device_sig
        })
    }
}

/*
 * AUTH_ACK packet payload:
 *   - encrypted:
 *      - serverSessionPk,
 *      - maxConnections,
 *      - nameAccess,
 *      - endpoint\0,
 *      - namedEndpoint\0,
 *       - padding
 */
pub(crate) struct AuthAck {
    server_session_pk: cryptobox::PublicKey,
    max_connections:   u16,
    name_access:       bool,
    endpoint:          String,
    named_endpoint:    Option<String>,
}

#[allow(dead_code)]
impl AuthAck {
    const MIN_SECRET_BYTES: usize = cryptobox::PublicKey::BYTES
        + mem::size_of::<u16>()         // maxConnections
        + mem::size_of::<u8>()          // nameAccess
        + 2;

    pub(crate) const MIN_BYTES: usize = HEADER_BYTES                // packet header
        + cryptobox::Nonce::BYTES + cryptobox::CryptoBox::MAC_BYTES // encryption header
        + Self::MIN_SECRET_BYTES;

    pub(crate) fn new(
        server_session_pk: cryptobox::PublicKey,
        max_connections: u16,
        name_access: bool,
        endpoint: String,
        named_endpoint: Option<String>,
    ) -> Self {
        Self {
            server_session_pk,
            max_connections,
            name_access,
            endpoint,
            named_endpoint
        }
    }

    pub(crate) fn server_session_pk(&self) -> &cryptobox::PublicKey {
        &self.server_session_pk
    }

    pub(crate) fn max_connections(&self) -> u16 {
        self.max_connections
    }

    pub(crate) fn name_access(&self) -> bool {
        self.name_access
    }

    pub(crate) fn endpoint(&self) -> &str {
        &self.endpoint
    }

    pub(crate) fn named_endpoint(&self) -> Option<&str> {
        self.named_endpoint.as_deref()
    }

    pub(crate) fn encode(&self, crypto_context: &mut CryptoContext) -> Result<Vec<u8>> {
        let endpoints_size = self.endpoint.len()
            + self.named_endpoint.as_ref().map_or(0, |s| s.len());

        let padding = random_padding(Self::MIN_BYTES + endpoints_size);
        let mut secret = vec![0u8; Self::MIN_SECRET_BYTES + endpoints_size + padding.len()];

        let mut pos = 0;
        secret[pos..pos + cryptobox::PublicKey::BYTES].copy_from_slice(self.server_session_pk.as_bytes());
        pos += cryptobox::PublicKey::BYTES;

        secret[pos..pos + mem::size_of::<u16>()].copy_from_slice(&self.max_connections.to_be_bytes());
        pos += mem::size_of::<u16>();

        secret[pos] = self.name_access as u8;
        pos += 1;

        let bytes = self.endpoint.as_bytes();
        secret[pos..pos + bytes.len()].copy_from_slice(bytes);
        pos += bytes.len();
        secret[pos] = 0;
        pos += 1;

        if let Some(named) = &self.named_endpoint {
            let bytes = named.as_bytes();
            secret[pos..pos + bytes.len()].copy_from_slice(bytes);
            pos += bytes.len();
        }
        secret[pos] = 0;
        pos += 1;

        secret[pos..].copy_from_slice(&padding);

        let cipher = crypto_context.encrypt_into(&secret)?;

        let size = HEADER_BYTES + cipher.len();
        let mut packet = Vec::with_capacity(size);
        packet.extend_from_slice(&(size as u16).to_be_bytes());
        packet.push(PacketType::AuthAck(AuthType::default()).value());
        packet.extend_from_slice(&cipher);
        Ok(packet)
    }

    pub(crate) fn decode(packet: &[u8], crypto_context: &CryptoContext) -> Result<Self> {
        if packet.len() < Self::MIN_BYTES {
            return Err(MalformedError::new("packet too short"));
        }

        let cipher = &packet[HEADER_BYTES..];
        let secret = crypto_context.decrypt_into(cipher)
            .map_err(|e| MalformedError::new(format!("failed to decrypt packet: {e}")))?;

        if secret.len() < Self::MIN_SECRET_BYTES {
            return Err(MalformedError::new("AUTH_ACK payload too short"));
        }

        let mut pos = 0;
        let server_session_pk = cryptobox::PublicKey::try_from(
            &secret[pos..pos + cryptobox::PublicKey::BYTES]
        )?;
        pos += cryptobox::PublicKey::BYTES;

        let max_connections = u16::from_be_bytes([secret[pos], secret[pos + 1]]);
        pos += 2;

        let name_access = secret[pos] == 1;
        pos += 1;

        let mut endpoint = None;
        if secret[pos] != 0 {
            let end = find_terminator(&secret, pos)
                .ok_or_else(|| MalformedError::new("missing null terminator for the endpoint"))?;
            endpoint = Some(String::from_utf8_lossy(&secret[pos..end]).into_owned());
            pos = end + 1;
        }
        let endpoint = endpoint.ok_or_else(|| MalformedError::new("missing endpoint"))?;

        let mut named_endpoint = None;
        if pos < secret.len() && secret[pos] != 0 {
            let end = find_terminator(&secret, pos)
                .ok_or_else(|| MalformedError::new("missing null terminator for the named endpoint"))?;
            named_endpoint = Some(String::from_utf8_lossy(&secret[pos..end]).into_owned());
        }

        if name_access && named_endpoint.is_none() {
            return Err(MalformedError::new("missing named endpoint"));
        }

        Ok(Self {
            server_session_pk,
            max_connections,
            name_access,
            endpoint,
            named_endpoint
        })
    }
}

/*
 * ATTACH packet payload:
 *   - plain:
 *      - deviceId
 *   - encrypted:
 *      - clientSessionPk,
 *      - deviceSig[challenge],
 *      - padding
 */
pub(crate) struct Attach {
    device_id:          Id,
    client_session_pk:  cryptobox::PublicKey,
    device_sig:         Vec<u8>,
}

#[allow(dead_code)]
impl Attach {
    const SECRET_BYTES: usize = cryptobox::PublicKey::BYTES + Signature::BYTES;
    pub(crate) const BYTES: usize = HEADER_BYTES                    // packet header
        + Id::BYTES                                                 // plain device id
        + cryptobox::Nonce::BYTES + cryptobox::CryptoBox::MAC_BYTES // encryption header
        + Self::SECRET_BYTES;

    pub(crate) fn new(
        device_id: Id,
        client_session_pk: cryptobox::PublicKey,
        device_sig: Vec<u8>
    ) -> Self {
        Self {
            device_id,
            client_session_pk,
            device_sig
        }
    }

    pub(crate) fn device_id(&self) -> &Id {
        &self.device_id
    }

    pub(crate) fn client_session_pk(&self) -> &cryptobox::PublicKey {
        &self.client_session_pk
    }

    pub(crate) fn device_sig(&self) -> &[u8] {
        &self.device_sig
    }

    pub(crate) fn encode(&self, crypto_context: &mut CryptoContext) -> Result<Vec<u8>> {
        let padding = random_padding(Self::BYTES);
        let mut secret = vec![0u8; Self::SECRET_BYTES + padding.len()];

        let mut pos = 0;
        secret[pos..pos + cryptobox::PublicKey::BYTES].copy_from_slice(self.client_session_pk.as_bytes());
        pos += cryptobox::PublicKey::BYTES;

        secret[pos..pos + self.device_sig.len()].copy_from_slice(&self.device_sig);
        pos += self.device_sig.len();

        secret[pos..].copy_from_slice(&padding);

        let cipher = crypto_context.encrypt_into(&secret)?;

        let size = Self::BYTES + padding.len();
        let mut packet = Vec::with_capacity(size);
        packet.extend_from_slice(&(size as u16).to_be_bytes());
        packet.push(PacketType::Attach(AttachType::default()).value());
        packet.extend_from_slice(&self.device_id);
        packet.extend_from_slice(&cipher);
        Ok(packet)
    }

    pub(crate) fn verify(&self, challenge: &[u8]) -> Result<bool> {
        signature::verify(
            challenge,
            &self.device_sig,
            &self.device_id.to_signature_key()
        )
    }

    pub(crate) fn decode(packet: &[u8], identity: &dyn Identity) -> Result<Self> {
        if packet.len() < Self::BYTES {
            return Err(MalformedError::new("packet too short"));
        }

        let mut pos = HEADER_BYTES;
        let device_id = Id::try_from_bytes(&packet[pos..pos + Id::BYTES])?;
        pos += Id::BYTES;

        let cipher = &packet[pos..];
        let secret = identity.decrypt_into(&device_id, cipher)
            .map_err(|e| MalformedError::new(format!("failed to decrypt packet: {e}")))?;

        if secret.len() < Self::SECRET_BYTES {
            return Err(MalformedError::new("ATTACH payload too short"));
        }

        let mut pos = 0;
        let client_session_pk = cryptobox::PublicKey::try_from(
            &secret[pos..pos + cryptobox::PublicKey::BYTES]
        )?;
        pos += cryptobox::PublicKey::BYTES;

        let device_sig = secret[pos..pos + Signature::BYTES].to_vec();

        Ok(Self {
            device_id,
            client_session_pk,
            device_sig
        })
    }
}

pub(crate) struct AttachAck;

#[allow(dead_code)]
impl AttachAck {
    pub(crate) const BYTES: usize = HEADER_BYTES;

    pub(crate) fn encode() -> Vec<u8> {
        encode_empty_payload(PacketType::AttachAck(AttachType::default()))
    }

    pub(crate) fn decode(_packet: &[u8]) -> Result<Self> {
        Ok(Self)
    }
}

pub(crate) struct Ping;

#[allow(dead_code)]
impl Ping {
    pub(crate) const BYTES: usize = HEADER_BYTES;

    pub(crate) fn encode() -> Vec<u8> {
        encode_empty_payload(PacketType::Ping(PingType::default()))
    }

    pub(crate) fn decode(_packet: &[u8]) -> Result<Self> {
        Ok(Self)
    }
}

pub(crate) struct PingAck;

#[allow(dead_code)]
impl PingAck {
    pub(crate) const BYTES: usize = HEADER_BYTES;

    pub(crate) fn encode() -> Vec<u8> {
        encode_empty_payload(PacketType::PingAck(PingType::default()))
    }

    pub(crate) fn decode(_packet: &[u8]) -> Result<Self> {
        Ok(Self)
    }
}

/*
 * CONNECT packet payload:
 *   - encrypted:
 *      - port,
 *      - addrlen,
 *      - addr[16 bytes both for IPv4 or IPv6],
 *      - padding
 */
pub(crate) struct Connect {
    address:    IpAddr,
    port:       u16,
}

#[allow(dead_code)]
impl Connect {
    const SECRET_BYTES: usize = mem::size_of::<u16>()   // port
        + mem::size_of::<u8>()                          // addrlen
        + 16;                                           // addr (16 bytes for both IPv4 and IPv6)

    pub(crate) const BYTES: usize = HEADER_BYTES        // packet header
        + cryptobox::Nonce::BYTES + cryptobox::CryptoBox::MAC_BYTES // encryption header
        + Self::SECRET_BYTES;

    pub(crate) fn new(address: IpAddr, port: u16) -> Self {
        Self { address, port }
    }

    pub(crate) fn address(&self) -> IpAddr {
        self.address
    }

    pub(crate) fn port(&self) -> u16 {
        self.port
    }

    pub(crate) fn encode(&self, crypto_context: &mut CryptoContext) -> Result<Vec<u8>> {
        let padding = random_padding(Self::BYTES);
        let mut secret = vec![0u8; Self::SECRET_BYTES + padding.len()];

        let mut pos = 0;
        secret[pos..pos + 2].copy_from_slice(&self.port.to_be_bytes());
        pos += 2;

        let addr = match self.address {
            IpAddr::V4(v4) => v4.octets().to_vec(),
            IpAddr::V6(v6) => v6.octets().to_vec(),
        };
        secret[pos] = addr.len() as u8;
        pos += 1;

        secret[pos..pos + addr.len()].copy_from_slice(&addr);
        pos += addr.len();

        secret[pos..].copy_from_slice(&padding);

        let cipher = crypto_context.encrypt_into(&secret)?;

        let size = Self::BYTES + padding.len();
        let mut packet = Vec::with_capacity(size);
        packet.extend_from_slice(&(size as u16).to_be_bytes());
        packet.push(PacketType::Connect(ConnType::default()).value());
        packet.extend_from_slice(&cipher);
        Ok(packet)
    }

    pub(crate) fn decode(packet: &[u8], crypto_context: &CryptoContext) -> Result<Self> {
        if packet.len() < Self::BYTES {
            return Err(MalformedError::new("packet too short"));
        }

        let cipher = &packet[HEADER_BYTES..];
        let secret = crypto_context.decrypt_into(cipher)
            .map_err(|e| MalformedError::new(format!("failed to decrypt packet: {e}")))?;

        if secret.len() < Self::SECRET_BYTES {
            return Err(MalformedError::new("CONNECT payload too short"));
        }

        let mut pos = 0;
        let port = u16::from_be_bytes([secret[pos], secret[pos + 1]]);
        pos += 2;

        let addr_len = secret[pos] as usize;
        pos += 1;
        if addr_len > secret.len() - pos {
            return Err(MalformedError::new("invalid address length"));
        }

        let address = match addr_len {
            4 => IpAddr::V4(Ipv4Addr::new(
                secret[pos],
                secret[pos + 1],
                secret[pos + 2],
                secret[pos + 3])
            ),
            16 => {
                let mut octets = [0u8; 16];
                octets.copy_from_slice(&secret[pos..pos + 16]);
                IpAddr::V6(Ipv6Addr::from(octets))
            },
            _ => return Err(MalformedError::new(format!("invalid address length: {addr_len}"))),
        };

        Ok(Self { address, port })
    }
}

/*
 * CONNECT_ACK packet payload
 *  - plain (not encrypted):
 *      - succeeded,
 *      - padding
 */
pub(crate) struct ConnectAck {
    succeeded: bool,
}

#[allow(dead_code)]
impl ConnectAck {
    pub(crate) const BYTES: usize = HEADER_BYTES + mem::size_of::<u8>();

    pub(crate) fn new(succeeded: bool) -> Self {
        Self { succeeded }
    }

    pub(crate) fn succeeded(&self) -> bool {
        self.succeeded
    }

    pub(crate) fn encode(&self) -> Vec<u8> {
        let padding = random_padding(Self::BYTES);
        let size = Self::BYTES + padding.len();

        let mut packet = Vec::with_capacity(size);
        packet.extend_from_slice(&(size as u16).to_be_bytes());
        packet.push(PacketType::ConnectAck(ConnType::default()).value());
        packet.push(random_ack_byte(self.succeeded));
        packet.extend_from_slice(&padding);
        packet
    }

    pub(crate) fn decode(packet: &[u8]) -> Result<Self> {
        if packet.len() < Self::BYTES {
            return Err(MalformedError::new("packet too short"));
        }
        let succeeded = (packet[HEADER_BYTES] & 0x01) != 0;
        Ok(Self { succeeded })
    }
}

pub(crate) struct Disconnect;

#[allow(dead_code)]
impl Disconnect {
    pub(crate) const BYTES: usize = HEADER_BYTES;

    pub(crate) fn encode() -> Vec<u8> {
        encode_empty_payload(PacketType::Disconnect(DisconnType::default()))
    }

    pub(crate) fn decode(_packet: &[u8]) -> Result<Self> {
        Ok(Self)
    }
}

pub(crate) struct DisconnectAck;

#[allow(dead_code)]
impl DisconnectAck {
    pub(crate) const BYTES: usize = HEADER_BYTES;

    pub(crate) fn encode() -> Vec<u8> {
        encode_empty_payload(PacketType::DisconnectAck(DisconnType::default()))
    }

    pub(crate) fn decode(_packet: &[u8]) -> Result<Self> {
        Ok(Self)
    }
}

/*
 * DATA packet payload:
 *   - encrypted: data
 */
pub(crate) struct Data {
    data: Vec<u8>,
}

#[allow(dead_code)]
impl Data {
    pub(crate) const MIN_BYTES: usize = HEADER_BYTES                // packet header
        + cryptobox::Nonce::BYTES + cryptobox::CryptoBox::MAC_BYTES; // encryption header

    pub(crate) fn new(data: Vec<u8>) -> Self {
        Self { data }
    }

    pub(crate) fn data(&self) -> &[u8] {
        &self.data
    }

    pub(crate) fn encode(&self, crypto_context: &mut CryptoContext) -> Result<Vec<u8>> {
        let cipher = crypto_context.encrypt_into(&self.data)?;

        let size = HEADER_BYTES + cipher.len();
        let mut packet = Vec::with_capacity(size);
        packet.extend_from_slice(&(size as u16).to_be_bytes());
        packet.push(PacketType::Data(DataType::default()).value());
        packet.extend_from_slice(&cipher);
        Ok(packet)
    }

    pub(crate) fn decode(packet: &[u8], crypto_context: &CryptoContext) -> Result<Self> {
        if packet.len() < Self::MIN_BYTES {
            return Err(MalformedError::new("packet too short"));
        }

        let cipher = &packet[HEADER_BYTES..];
        let data = crypto_context.decrypt_into(cipher)
            .map_err(|e| MalformedError::new(format!("failed to decrypt packet: {e}")))?;

        Ok(Self { data })
    }
}

pub(crate) struct Error {
    code:       i16,
    message:    Option<String>,
}

#[allow(dead_code)]
impl Error {
    const MIN_SECRET_BYTES: usize = mem::size_of::<u16>() + 1;
    pub(crate) const MIN_BYTES: usize = HEADER_BYTES            // packet header
        + cryptobox::Nonce::BYTES + cryptobox::CryptoBox::MAC_BYTES // encryption header
        + Self::MIN_SECRET_BYTES;

    pub(crate) fn new(code: i16, message: Option<String>) -> Self {
        Self { code, message }
    }

    pub(crate) fn code(&self) -> i16 {
        self.code
    }

    pub(crate) fn message(&self) -> Option<&str> {
        self.message.as_deref()
    }

    pub(crate) fn encode(&self, crypto_context: &mut CryptoContext) -> Result<Vec<u8>> {
        let message_len = self.message.as_ref().map_or(0, |m| m.len());
        let padding = random_padding(Self::MIN_BYTES + message_len);
        let mut secret = vec![0u8; Self::MIN_SECRET_BYTES + message_len + padding.len()];

        let mut pos = 0;
        secret[pos..pos + 2].copy_from_slice(&self.code.to_be_bytes());
        pos += 2;

        if let Some(message) = &self.message {
            let bytes = message.as_bytes();
            secret[pos..pos + bytes.len()].copy_from_slice(bytes);
            pos += bytes.len();
        }
        secret[pos] = 0;
        pos += 1;

        secret[pos..].copy_from_slice(&padding);

        let cipher = crypto_context.encrypt_into(&secret)?;

        let size = HEADER_BYTES + cipher.len();
        let mut packet = Vec::with_capacity(size);
        packet.extend_from_slice(&(size as u16).to_be_bytes());
        packet.push(PacketType::Error(ErrType::default()).value());
        packet.extend_from_slice(&cipher);
        Ok(packet)
    }

    pub(crate) fn decode(packet: &[u8], crypto_context: &CryptoContext) -> Result<Self> {
        if packet.len() < Self::MIN_BYTES {
            return Err(MalformedError::new("packet too short"));
        }

        let cipher = &packet[HEADER_BYTES..];
        let secret = crypto_context.decrypt_into(cipher)
            .map_err(|e| MalformedError::new(format!("failed to decrypt packet: {e}")))?;

        if secret.len() < Self::MIN_SECRET_BYTES {
            return Err(MalformedError::new("ERROR payload too short"));
        }

        let mut pos = 0;
        let code = u16::from_be_bytes([secret[pos], secret[pos + 1]]) as i16;
        pos += 2;

        let mut message = None;
        if secret[pos] != 0 {
            let end = find_terminator(&secret, pos)
                .ok_or_else(|| MalformedError::new("missing null terminator for the message"))?;
            message = Some(String::from_utf8_lossy(&secret[pos..end]).into_owned());
        }

        Ok(Self { code, message })
    }
}

// Finds the offset of the next 0x00 byte at or after `start`.
fn find_terminator(buf: &[u8], start: usize) -> Option<usize> {
    buf[start..].iter().position(|&b| b == 0).map(|i| start + i)
}

// Rounds up to the nearest multiple of 256 and picks a random padding length within that bound.
fn padding_size(size: usize) -> usize {
    let bound = (size + 255) & !255;
    let range = (bound - size + 1) as u32;
    (unsafe { libsodium_sys::randombytes_uniform(range) }) as usize
}

fn random_padding(size: usize) -> Vec<u8> {
    let len = padding_size(size);
    let mut padding = vec![0u8; len];
    if len > 0 {
        unsafe {
            libsodium_sys::randombytes_buf(padding.as_mut_ptr() as *mut _, len);
        }
    }
    padding
}

fn random_ack_byte(succeeded: bool) -> u8 {
    let r = (unsafe { libsodium_sys::randombytes_uniform(255) }) as u8;
    match succeeded {
        true => r | 0x01,
        false => r & 0xFE,
    }
}

fn encode_empty_payload(ptype: PacketType) -> Vec<u8> {
    let padding = random_padding(HEADER_BYTES);
    let size = HEADER_BYTES + padding.len();

    let mut packet = Vec::with_capacity(size);
    packet.extend_from_slice(&(size as u16).to_be_bytes());
    packet.push(ptype.value());
    packet.extend_from_slice(&padding);
    packet
}
