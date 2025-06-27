use std::mem;
use std::net::{IpAddr, SocketAddr};
use std::cell::RefCell;
use std::time::SystemTime;
use sha2::{Digest, Sha256};

use crate::{
    randomize_bytes,
    Id,
};

const TOKEN_TIMEOUT: u128 = 5 * 60 * 1000; // 5 minutes

pub(crate) struct TokenManager {
    session_secret: [u8; 32],
    timestamp: RefCell<SystemTime>,
    previous_timestamp: RefCell<SystemTime>,
}

impl TokenManager {
    pub(crate) fn new() -> Self {
        let mut seed = [0u8; 32];
        randomize_bytes(&mut seed);

        TokenManager {
            session_secret: seed,
            timestamp: RefCell::new(SystemTime::now()),
            previous_timestamp: RefCell::new(SystemTime::now()),
        }
    }

    fn update_token_timestamp(&self) {
        if self.timestamp.borrow().elapsed().unwrap().as_millis() > TOKEN_TIMEOUT {
            *self.previous_timestamp.borrow_mut() = *self.timestamp.borrow();
            *self.timestamp.borrow_mut() = SystemTime::now();
        }
    }

    pub(crate) fn generate_token(
        &self,
        nodeid: &Id,
        addr: &SocketAddr,
        target: &Id
    ) -> i32 {
        generate_token(nodeid, addr, target, &self.timestamp, &self.session_secret)
    }

    pub(crate) fn verify_token(&self,
        token: i32,
        nodeid: &Id,
        addr: &SocketAddr,
        target: &Id,
    ) -> bool {
        self.update_token_timestamp();
        if token == generate_token(
            nodeid,
            addr,
            target,
            &self.timestamp,
            &self.session_secret
        ) {
            return true
        }

        token == generate_token(
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
    timestamp: &RefCell<SystemTime>,
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
        + secret.len()
    );

    let duration = timestamp.borrow().duration_since(SystemTime::UNIX_EPOCH).unwrap();

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
    ((((digest[pos as usize] & 0xff) as u32) << 24) |
        (((digest[(pos as usize + 1) & 0x1f] & 0xff) as u32) << 16)|
        (((digest[(pos as usize + 2) & 0x1f] & 0xff) as u32) << 8) |
        ((digest[(pos as usize + 3) & 0x1f] & 0xff) as u32))
    as i32
}
