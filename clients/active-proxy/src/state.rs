// Copyright (c) 2023 -      bosonnetwork.io
//
// Permission is hereby granted, free of charge, to any person obtaining a copy
// of this software and associated documentation files (the "Software"), to deal
// in the Software without restriction, including without limitation the rights
// to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
// copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions:
//
// The above copyright notice and this permission notice shall be included in all
// copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
// OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
// SOFTWARE.

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
                matches!(pkt, PacketType::PingAck(_))
                    || matches!(pkt, PacketType::Disconnect(_))
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
