mod packet;
mod state;
mod connection;
mod managed;
mod worker;
pub mod client;

#[cfg(test)]
mod unitests {
    mod test_activeproxy;
}

pub use {
    client::ProxyClient as ActiveProxyClient,
};

pub(crate)
fn random_padding() -> u32 {
    unsafe {
        libsodium_sys::randombytes_random() % 32
    }
}

pub(crate)
fn random_boolean(input: bool) -> u8 {
    let val = unsafe {
        libsodium_sys::randombytes_random()
    } as u8;

    match input {
        true => val | 0x01,
        false => val & 0xFE
    }
}

pub(crate)
fn random_timeshift() -> u32 {
    unsafe { // max is 10s
        libsodium_sys::randombytes_random() % 10
    }
}
