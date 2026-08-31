use std::fmt;
use crate::{
    Result,
    core::errors::StateError,
};

const ACK_MASK  :u8 = 0x80;
const TYPE_MASK :u8 = 0x7F;

fn randv(min:u8, max: u8) -> u8 {
    (unsafe {
        libsodium_sys::randombytes_uniform((max - min + 1) as u32)
    }) as u8
}

/*
#[derive(Default,PartialEq, Eq)]
pub(crate) struct ChanllengeType;
impl ChanllengeType {
    pub(crate) fn value(&self) -> u8 {
        0x00
    }
}
*/

#[derive(Default,PartialEq, Eq)]
pub(crate) struct AuthType;
impl AuthType {
    const MIN: u8 = 0x00;
    const MAX: u8 = 0x07;

    pub(crate) fn value(&self) -> u8 {
        randv(Self::MIN, Self::MAX) + Self::MIN
    }

    fn from(ack: bool) -> Result<PacketType> {
        let t = Self::default();
        match ack {
            true => Ok(PacketType::AuthAck(t)),
            false => Ok(PacketType::Auth(t))
        }
    }
}

#[derive(Default,PartialEq, Eq)]
pub(crate) struct AttachType;
impl AttachType {
    const MIN: u8 = 0x08;
    const MAX: u8 = 0x0F;

    fn value(&self) -> u8 {
        randv(Self::MIN, Self::MAX) + Self::MIN
    }

    fn from(ack: bool) -> Result<PacketType> {
        let t = Self::default();
        match ack {
            true => Ok(PacketType::AttachAck(t)),
            false => Ok(PacketType::Attach(t))
        }
    }
}

#[derive(Default,PartialEq, Eq)]
pub(crate) struct PingType;
impl PingType {
    const MIN: u8 = 0x10;
    const MAX: u8 = 0x1F;

    fn value(&self) -> u8 {
        randv(Self::MIN, Self::MAX) + Self::MIN
    }

    fn from(ack: bool) -> Result<PacketType> {
        let t = Self::default();
        match ack {
            true => Ok(PacketType::PingAck(t)),
            false => Ok(PacketType::Ping(t))
        }
    }
}

#[derive(Default,PartialEq, Eq)]
pub(crate) struct ConnType;
impl ConnType {
    const MIN: u8 = 0x20;
    const MAX: u8 = 0x2F;

    fn value(&self) -> u8 {
        randv(Self::MIN, Self::MAX) + Self::MIN
    }

    fn from(ack: bool) -> Result<PacketType> {
        let t = Self::default();
        match ack {
            true => Ok(PacketType::ConnectAck(t)),
            false => Ok(PacketType::Connect(t))
        }
    }
}

#[derive(Default,PartialEq, Eq)]
pub(crate) struct DisconnType;
impl DisconnType {
    const MIN: u8 = 0x30;
    const MAX: u8 = 0x3F;

    fn value(&self) -> u8 {
        randv(Self::MIN, Self::MAX) + Self::MIN
    }

    fn from(ack: bool) -> Result<PacketType> {
        let t = Self::default();
        match ack {
            true => Ok(PacketType::DisconnectAck(t)),
            false => Ok(PacketType::Disconnect(t))
        }
    }
}

#[derive(Default,PartialEq, Eq)]
pub(crate) struct DataType;
impl DataType{
    const MIN: u8 = 0x40;
    const MAX: u8 = 0x6F;

    fn value(&self) -> u8 {
        randv(Self::MIN, Self::MAX) + Self::MIN
    }

    fn from(ack: bool) -> Result<PacketType> {
        let t = Self::default();
        match ack {
            true => Err(StateError::new("Should never happen: Data packet should not be with ack")),
            false => Ok(PacketType::Data(t))
        }
    }
}

#[derive(Default,PartialEq, Eq)]
pub(crate) struct ErrType;
impl ErrType{
    const MIN: u8 = 0x70;
    const MAX: u8 = 0x7F;

    fn value(&self) -> u8 {
        randv(Self::MIN, Self::MAX) + Self::MIN
    }

    fn from(ack: bool) -> Result<PacketType> {
        let t = Self::default();
        match ack {
            true => Err(StateError::new("Should never happen: Error packet should not be with ack")),
            false => Ok(PacketType::Error(t))
        }
    }
}

#[derive(PartialEq, Eq)]
pub(crate) enum PacketType {
    // Challenge(ChallengeType),
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

impl PacketType {
    pub(crate) fn from(input: u8) -> Result<PacketType> {
        let ack = (input & ACK_MASK) != 0;
        let val = input & TYPE_MASK;

        match val {
            AuthType::MIN..=AuthType::MAX       => AuthType::from(ack),
            AttachType::MIN..=AttachType::MAX   => AttachType::from(ack),
            PingType::MIN..=PingType::MAX       => PingType::from(ack),
            ConnType::MIN..=ConnType::MAX       => ConnType::from(ack),
            DisconnType::MIN..=DisconnType::MAX => DisconnType::from(ack),
            DataType::MIN..=DataType::MAX       => DataType::from(ack),
            ErrType::MIN..=ErrType::MAX         => ErrType::from(ack),
            _ => Err(StateError::new(format!("Invalid packet type: {}", input)))
        }
    }

    pub(crate) fn value(&self) -> u8 {
        match self {
            Self::Auth(v)         => v.value(),
            Self::AuthAck(v)      => v.value() | ACK_MASK,
            Self::Attach(v)       => v.value(),
            Self::AttachAck(v)    => v.value() | ACK_MASK,
            Self::Ping(v)         => v.value(),
            Self::PingAck(v)      => v.value() | ACK_MASK,
            Self::Connect(v)      => v.value(),
            Self::ConnectAck(v)   => v.value() | ACK_MASK,
            Self::Disconnect(v)   => v.value(),
            Self::DisconnectAck(v)=> v.value() | ACK_MASK,
            Self::Data(v)         => v.value(),
            Self::Error(v)        => v.value()
        }
    }

    /*
    pub(crate) fn ack(&self) -> bool {
        (self.value() & ACK_MASK) != 0
    }
    */
}

impl fmt::Display for PacketType  {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let str = match self {
            Self::Auth(_)         => "AUTH",
            Self::AuthAck(_)      => "AUTH ACK",
            Self::Attach(_)       => "ATTACH",
            Self::AttachAck(_)    => "ATTACH ACK",
            Self::Ping(_)         => "PING",
            Self::PingAck(_)      => "PING ACK",
            Self::Connect(_)      => "CONNECT",
            Self::ConnectAck(_)   => "CONNECT ACK",
            Self::Disconnect(_)   => "DISCONNECT",
            Self::DisconnectAck(_)=> "DISCONNECT ACK",
            Self::Data(_)         => "DATA",
            Self::Error(_)        => "ERROR"
        };
        write!(f, "{}", str)?;
        Ok(())
    }
}