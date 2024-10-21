use std::fmt;
use crate::{
    Error,
    error::Result,
};

const AUTH_MIN          :u8 = 0x00;
const AUTH_MAX          :u8 = 0x07;
const ATTACH_MIN        :u8 = 0x08;
const ATTACH_MAX        :u8 = 0x0F;
const PING_MIN          :u8 = 0x10;
const PING_MAX          :u8 = 0x1F;
const CONNECT_MIN       :u8 = 0x20;
const CONNECT_MAX       :u8 = 0x2F;
const DISCONNECT_MIN    :u8 = 0x30;
const DISCONNECT_MAX    :u8 = 0x3F;
const DATA_MIN          :u8 = 0x40;
const DATA_MAX          :u8 = 0x6F;
const ERROR_MIN         :u8 = 0x70;
const ERROR_MAX         :u8 = 0x7F;

const ACK_MASK          :u8 = 0x80;
const TYPE_MASK         :u8 = 0x7F;

fn randv(min:u8, max: u8) -> u8 {
    (unsafe {
        libsodium_sys::randombytes_uniform((max - min + 1) as u32)
    }) as u8
}

#[derive(Default,PartialEq, Eq)]
pub(crate) struct AuthType;
impl AuthType {
    pub(crate) fn value(&self) -> u8 {
        randv(AUTH_MIN, AUTH_MAX) + AUTH_MIN
    }
}

#[derive(Default,PartialEq, Eq)]
pub(crate) struct AttachType;
impl AttachType {
    pub(crate) fn value(&self) -> u8 {
        randv(ATTACH_MIN, ATTACH_MAX) + ATTACH_MIN
    }
}

#[derive(Default,PartialEq, Eq)]
pub(crate) struct PingType;
impl PingType {
    pub(crate) fn value(&self) -> u8 {
        randv(PING_MIN, PING_MAX) + PING_MIN
    }
}

#[derive(Default,PartialEq, Eq)]
pub(crate) struct ConnType;
impl ConnType {
    pub(crate) fn value(&self) -> u8 {
        randv(CONNECT_MIN, CONNECT_MAX) + CONNECT_MIN
    }
}

#[derive(Default,PartialEq, Eq)]
pub(crate) struct DisconnType;
impl DisconnType {
    pub(crate) fn value(&self) -> u8 {
        randv(DISCONNECT_MIN, DISCONNECT_MAX) + DISCONNECT_MIN
    }
}

#[derive(Default,PartialEq, Eq)]
pub(crate) struct DataType;
impl DataType{
    pub(crate) fn value(&self) -> u8 {
        randv(DATA_MIN, DATA_MAX) + DATA_MIN
    }
}

#[derive(Default,PartialEq, Eq)]
pub(crate) struct ErrType;
impl ErrType{
    pub(crate) fn value(&self) -> u8 {
        randv(ERROR_MIN, ERROR_MAX) + ERROR_MIN
    }
}

#[derive(PartialEq, Eq)]
pub(crate) enum Packet {
    Auth(AuthType),
    AuthAck(AuthType),
    Attach(AttachType),
    AttachAck(AttachType),
    Ping(PingType),
    PingAck(PingType),
    Connect(ConnType),
    ConnectAck(ConnType),
    Disconnect(DisconnType),
    DisconnectAck(DisconnType),
    Data(DataType),
    Error(ErrType)
}

fn create_packet<T: Default>(ack: bool,
    with_ack: fn(T) -> Packet,
    no_ack: fn(T) -> Packet
) -> Result<Packet> {

    let t = T::default();
    match ack {
        true => Ok(with_ack(t)),
        false => Ok(no_ack(t))
    }
}

fn create_packet_on_err<T: Default>(ack: bool,
    with_ack: fn(T) -> Packet,
    no_ack_err: Error
) -> Result<Packet> {

    match ack {
        true => Ok(with_ack(T::default())),
        false => Err(no_ack_err)
    }
}

#[allow(dead_code)]
impl Packet {
    pub(crate) fn from(input: u8) -> Result<Packet> {
        let ack = (input & ACK_MASK) != 0;
        let val = input & TYPE_MASK;

        match val {
            AUTH_MIN..=AUTH_MAX     => create_packet::<AuthType>(ack, Packet::AuthAck, Packet::Auth),
            ATTACH_MIN..=ATTACH_MAX => create_packet::<AttachType>(ack, Packet::AttachAck, Packet::Attach),
            PING_MIN..=PING_MAX     => create_packet::<PingType>(ack, Packet::PingAck, Packet::Ping),
            CONNECT_MIN..=CONNECT_MAX
                                    => create_packet::<ConnType>(ack, Packet::ConnectAck, Packet::Connect),
            DISCONNECT_MIN..=DISCONNECT_MAX
                                    => create_packet::<DisconnType>(ack, Packet::DisconnectAck, Packet::Disconnect),
            DATA_MIN..=DATA_MAX     => create_packet_on_err::<DataType>(ack, Packet::Data,
                Error::State(format!("Should never happen: Data type should not be with ack"))
            ),
            ERROR_MIN..=ERROR_MAX   => create_packet_on_err::<ErrType>(ack, Packet::Error,
                Error::State(format!("Should never happen: Error type should not be with ack"))
            ),
            _ => Err(Error::State(format!("Invalid packet type: {}", input)))
        }
    }

    pub(crate) fn value(&self) -> u8 {
        match self {
            Packet::Auth(v)         => v.value(),
            Packet::AuthAck(v)      => v.value() | ACK_MASK,
            Packet::Attach(v)       => v.value(),
            Packet::AttachAck(v)    => v.value() | ACK_MASK,
            Packet::Ping(v)         => v.value(),
            Packet::PingAck(v)      => v.value() | ACK_MASK,
            Packet::Connect(v)      => v.value(),
            Packet::ConnectAck(v)   => v.value() | ACK_MASK,
            Packet::Disconnect(v)   => v.value(),
            Packet::DisconnectAck(v)=> v.value() | ACK_MASK,
            Packet::Data(v)         => v.value(),
            Packet::Error(v)        => v.value()
        }
    }

    pub(crate) fn type_(&self) -> i32 {
        unimplemented!()
    }

    pub(crate) fn ack(&self) -> bool {
        (self.value() & ACK_MASK) != 0
    }
}

impl fmt::Display for Packet {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let str = match self {
            Packet::Auth(_)         => "AUTH",
            Packet::AuthAck(_)      => "AUTH ACK",
            Packet::Attach(_)       => "ATTACH",
            Packet::AttachAck(_)    => "ATTACH ACK",
            Packet::Ping(_)         => "PING",
            Packet::PingAck(_)      => "PING ACK",
            Packet::Connect(_)      => "CONNECT",
            Packet::ConnectAck(_)   => "CONNECT ACK",
            Packet::Disconnect(_)   => "DISCONNECT",
            Packet::DisconnectAck(_)=> "DISCONNECT ACK",
            Packet::Data(_)         => "DATA",
            Packet::Error(_)        => "ERROR"
        };
        write!(f, "{}", str)?;
        Ok(())
    }
}