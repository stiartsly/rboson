use std::net::{IpAddr, SocketAddr};
use std::time::SystemTime;
use sha2::{Digest, Sha256};

use crate::{
    as_millis,
    Id,
};

#[allow(dead_code)]
const TOKEN_TIMEOUT: u128 = 5 * 60 * 1000; // 5 minutes

#[allow(dead_code)]
pub(crate) struct TokenManager {
    session_secret: [u8; 32],
    timestamp: SystemTime,
    previous_timestamp: SystemTime,
}

#[allow(dead_code)]
impl TokenManager {
    pub(crate) fn new() -> Self {
        let mut seed = [0u8; 32];
        unsafe {
            libsodium_sys::randombytes_buf(
                seed.as_mut_ptr() as *mut libc::c_void,
                32
            );
        }
        TokenManager {
            session_secret: seed,
            timestamp: SystemTime::now(),
            previous_timestamp: SystemTime::UNIX_EPOCH,
        }
    }

    fn update_token_timestamp(&mut self) {
        while self.timestamp.elapsed().unwrap().as_millis() > TOKEN_TIMEOUT {
            self.previous_timestamp = self.timestamp;
            self.timestamp = SystemTime::now();
            break;
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

    pub(crate) fn verify_token(
        &mut self,
        _token: i32,
        _nodeid: &Id,
        _addr: &SocketAddr,
        _target: &Id,
    ) -> bool {
/*
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
        );
*/
        // TODO:
        true
    }
}

fn generate_token(
    nodeid: &Id,
    addr: &SocketAddr,
    target: &Id,
    timestamp: &SystemTime,
    secret: &[u8],
) -> i32 {
    let mut input = Vec::with_capacity(2 // port size
        + nodeid.as_bytes().len()
        + target.as_bytes().len()
        + match addr.ip() {
            IpAddr::V4(_) => 4,  // 4bytes for IPv4
            IpAddr::V6(_) => 16, // 6bytes for IPv6
        }
        + 8  // timestamp in milliseconds (assuming u64)
        + secret.len()
    );

    input.extend_from_slice(nodeid.as_bytes());
    input.extend_from_slice(&addr.port().to_le_bytes());
    input.extend_from_slice(target.as_bytes());

    match addr.ip() {
        IpAddr::V4(ipv4) => input.extend_from_slice(&ipv4.octets()),
        IpAddr::V6(ipv6) => input.extend_from_slice(&ipv6.octets()),
    };

    input.extend_from_slice(&(as_millis!(timestamp) as u64).to_le_bytes());
    input.extend_from_slice(secret);

    let digest = Sha256::digest(input);
    let pos = (digest[0] & 0xff) & 0x1f; // mod 32
    let token = ((digest[pos as usize] as u32) << 24)
        | ((digest[(pos as usize + 1) & 0x1f] as u32) << 16)
        | ((digest[(pos as usize + 2) & 0x1f] as u32) << 8)
        |  (digest[(pos as usize + 3) & 0x1f] as u32);

    token as i32
}
