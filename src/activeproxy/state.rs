use std::fmt;
use super::{
    packet::Packet,
};

#[allow(dead_code)]
#[derive(Clone, Copy, PartialOrd, Ord, PartialEq, Eq)]
pub(crate) enum State {
    Initializing = 0,
    Authenticating,
    Attaching,
    Idling,
    Relaying,
    Disconnecting,
    Closed
}

impl State {
    pub(crate) fn accept(&self, pkt: &Packet) -> bool {
        match self {
            State::Initializing     => false,
            State::Authenticating   => matches!(pkt, Packet::AuthAck(_)),
            State::Attaching        => matches!(pkt, Packet::AttachAck(_)),
            State::Idling           => matches!(pkt, Packet::PingAck(_)) ||
                                       matches!(pkt, Packet::Connect(_)),
            State::Relaying         => matches!(pkt, Packet::PingAck(_)) ||
                                       matches!(pkt, Packet::Data(_)) ||
                                       matches!(pkt, Packet::Disconnect(_)),
            State::Disconnecting    => matches!(pkt, Packet::Disconnect(_)) ||
                                       matches!(pkt, Packet::Data(_)) ||
                                       matches!(pkt, Packet::DisconnectAck(_)),
            State::Closed           => false,
        }
    }
}

impl fmt::Display for State {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let str = match self {
            State::Initializing     => "Initializing",
            State::Authenticating   => "Authenticating",
            State::Attaching        => "Attaching",
            State::Idling           => "Idling",
            State::Relaying         => "Relaying",
            State::Disconnecting    => "Disconnecting",
            State::Closed           => "Closed",
        };

        write!(f, "{}", str)?;
        Ok(())
    }
}
