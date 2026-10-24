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

use boson_api::Id;
use sha2::{Digest, Sha256};
use std::{
    mem,
    net::{IpAddr, SocketAddr},
    sync::Mutex,
    time::SystemTime,
};

pub(crate) struct TokenManager {
    session_secret: [u8; 32],
    timestamp: Mutex<SystemTime>,
    previous_timestamp: Mutex<SystemTime>,
}

impl TokenManager {
    pub(crate) const TOKEN_TIMEOUT: u64 = 5 * 60 * 1000; // 5 minutes

    pub(crate) fn new() -> Self {
        let session_secret = boson_api::random_array::<32>();

        Self {
            session_secret,
            timestamp: Mutex::new(SystemTime::now()),
            previous_timestamp: Mutex::new(SystemTime::now()),
        }
    }

    pub(crate) fn update_token_timestamp(&self) {
        let mut tm = boson_api::locked!(self.timestamp);
        if boson_api::elapsed_ms!(tm) > Self::TOKEN_TIMEOUT as u128 {
            *self.previous_timestamp.lock().unwrap() = *tm;
            *tm = SystemTime::now();
        }
    }

    pub(crate) fn generate_token(&self, nodeid: &Id, addr: &SocketAddr, target: &Id) -> i32 {
        generate_token(nodeid, addr, target, &self.timestamp, &self.session_secret)
    }

    pub(crate) fn verify_token(
        &self,
        token: i32,
        nodeid: &Id,
        addr: &SocketAddr,
        target: &Id,
    ) -> bool {
        self.update_token_timestamp();
        if token == generate_token(nodeid, addr, target, &self.timestamp, &self.session_secret) {
            return true;
        }

        token
            == generate_token(
                nodeid,
                addr,
                target,
                &self.previous_timestamp,
                &self.session_secret,
            )
    }
}

fn generate_token(
    nodeid: &Id,
    addr: &SocketAddr,
    target: &Id,
    timestamp: &Mutex<SystemTime>,
    secret: &[u8],
) -> i32 {
    let mut input: Vec<u8> = Vec::with_capacity(
        mem::size_of::<u16>()   // port size
        + nodeid.as_bytes().len()
        + target.as_bytes().len()
        + match addr.ip() {
            IpAddr::V4(_) => 4,  // 4bytes for IPv4
            IpAddr::V6(_) => 16, // 6bytes for IPv6
        }
        + mem::size_of::<u64>() // timestamp in milliseconds (assuming u64)
        + secret.len(),
    );

    let duration = timestamp
        .lock()
        .unwrap()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap();

    // nodeId + ip + port + targetId + timestamp + sessionSecret
    input.extend_from_slice(nodeid.as_bytes());
    match addr.ip() {
        IpAddr::V4(ipv4) => input.extend_from_slice(&ipv4.octets()),
        IpAddr::V6(ipv6) => input.extend_from_slice(&ipv6.octets()),
    };
    input.extend_from_slice(&addr.port().to_le_bytes());
    input.extend_from_slice(target.as_bytes());
    input.extend_from_slice(&(duration.as_millis() as u64).to_le_bytes());
    input.extend_from_slice(secret);

    let digest = Sha256::digest(input);

    let pos = ((digest[0] & 0xff) & 0x1f) as u8; // mod 32
    ((((digest[pos as usize] & 0xff) as u32) << 24)
        | (((digest[(pos as usize + 1) & 0x1f] & 0xff) as u32) << 16)
        | (((digest[(pos as usize + 2) & 0x1f] & 0xff) as u32) << 8)
        | ((digest[(pos as usize + 3) & 0x1f] & 0xff) as u32)) as i32
}
