mod connection;
mod connection_handler;
mod connection_registry;
mod packet;
mod packet_type;
mod session;
mod state;
mod utils;
mod verticle;

pub(crate) use crate::utils::{
    handler::LocalBoxHandler,
    timer_client::{LocalBoxTimerClient, LocalBoxTimerCmd},
    timer_manager::LocalBoxTimerManager,
};

pub mod client;
pub mod options;

#[cfg(test)]
mod unitests {
    mod test_options;
}

pub use {
    client::ActiveProxyClient,
    options::{Options, OptionsBuilder},
};

/*
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
*/
pub(crate) fn random_timeshift() -> u32 {
    unsafe {
        // max is 10s
        libsodium_sys::randombytes_random() % 10
    }
}
