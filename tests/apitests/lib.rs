#[cfg(test)]
mod core {
    mod id;
    mod prefix;
    mod signature;
    mod cryptobox;
    mod node_info;
    mod peer_info;
    mod value;
    mod config;
}

#[cfg(test)]
mod dht {
    mod node;
}

#[cfg(test)]
mod did {
    mod didurl;
    mod verification_method;
}

use std::{
    env,
    fs,
    net::IpAddr
};

pub(crate) fn local_addr(ipv4: bool) -> Option<IpAddr>{
    let if_addrs = match get_if_addrs::get_if_addrs() {
        Ok(v) => v,
        Err(_) => return None
    };

    for iface in if_addrs {
        let ip = iface.ip();
        if !ip.is_loopback() &&
            ((ipv4 && ip.is_ipv4()) ||
            (!ipv4 && ip.is_ipv6())) {
            return Some(ip)
        }
    }
    None
}

pub(crate) fn randomize_bytes<const N: usize>(array: &mut [u8; N]) {
    unsafe {
        libsodium_sys::randombytes_buf(
            array.as_mut_ptr() as *mut libc::c_void,
            N
        );
    }
}

pub(crate) fn create_random_bytes(len: usize) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(len);
    unsafe {
        libsodium_sys::randombytes_buf(
            bytes.as_mut_ptr()as *mut libc::c_void,
            len
        );
        bytes.set_len(len);
    }
    bytes
}

pub(crate) fn working_path(input: &str) -> String {
    let path = env::current_dir().unwrap().join(input);
    if !fs::metadata(&path).is_ok() {
        match fs::create_dir(&path) {
            Ok(_) => {}
            Err(e) => {
                panic!("Failed to create directory: {}", e);
            }
        }
    }
    path.display().to_string()
}

pub(crate) fn remove_working_path(input: &str) {
    if fs::metadata(&input).is_ok() {
        match fs::remove_dir_all(&input) {
            Ok(_) => {}
            Err(e) => {
                panic!("Failed to remove directory: {}", e);
            }
        }
    }
}

fn main() {}
