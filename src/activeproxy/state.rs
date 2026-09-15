use super::packet_type::PacketType;
use std::fmt;

#[derive(Clone, Copy, PartialOrd, Ord, PartialEq, Eq)]
pub(crate) enum State {
    Initializing = 0,
    Authenticating,
    Attaching,
    Idling,
    Connecting,
    Relaying,
    Disconnecting,
    Closed,
}

impl State {
    pub(crate) fn accept(&self, pkt: &PacketType) -> bool {
        if matches!(pkt, PacketType::Error(_)) {
            return !matches!(self, State::Initializing | State::Closed);
        }

        match self {
            State::Initializing => false,
            State::Authenticating => matches!(pkt, PacketType::AuthAck(_)),
            State::Attaching => matches!(pkt, PacketType::AttachAck(_)),
            State::Idling => {
                matches!(pkt, PacketType::PingAck(_)) || matches!(pkt, PacketType::Connect(_))
            }
            State::Connecting => {
                true // TODO
            }
            State::Relaying => {
                matches!(pkt, PacketType::PingAck(_))
                    || matches!(pkt, PacketType::Data(_))
                    || matches!(pkt, PacketType::Disconnect(_))
            }
            State::Disconnecting => {
                matches!(pkt, PacketType::Disconnect(_))
                    || matches!(pkt, PacketType::Data(_))
                    || matches!(pkt, PacketType::DisconnectAck(_))
            }
            State::Closed => false,
        }
    }
}

impl fmt::Display for State {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let str = match self {
            State::Initializing => "Initializing",
            State::Authenticating => "Authenticating",
            State::Attaching => "Attaching",
            State::Idling => "Idling",
            State::Connecting => "Connecting",
            State::Relaying => "Relaying",
            State::Disconnecting => "Disconnecting",
            State::Closed => "Closed",
        };

        write!(f, "{}", str)?;
        Ok(())
    }
}
