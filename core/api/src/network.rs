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

use std::fmt;
use std::net::SocketAddr;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Network {
    IPv4 = 4,
    IPv6 = 6,
}

impl Network {
    pub fn is_ipv4(&self) -> bool {
        self == &Network::IPv4
    }

    pub fn is_ipv6(&self) -> bool {
        self == &Network::IPv6
    }

    pub fn can_use_address(&self, addr: &SocketAddr) -> bool {
        match self {
            Network::IPv4 => addr.is_ipv4(),
            Network::IPv6 => addr.is_ipv6(),
        }
    }
}

impl From<&SocketAddr> for Network {
    fn from(input: &SocketAddr) -> Self {
        match input.is_ipv4() {
            true => Network::IPv4,
            false => Network::IPv6,
        }
    }
}

impl fmt::Display for Network {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(match *self {
            Network::IPv4 => "v4",
            Network::IPv6 => "v6",
        })
    }
}
