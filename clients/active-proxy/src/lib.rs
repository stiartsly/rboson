// Migrated code: it calls libsodium directly and is not documented yet. Remove these allows as the
// crate is brought in line with the workspace lints.
#![allow(unsafe_code)]
#![allow(missing_docs)]

mod connection;
mod connection_handler;
mod connection_registry;
mod packet;
mod packet_type;
mod session;
mod state;
mod verticle;

pub(crate) use boson_api::utils::{
    handler::LocalBoxHandler,
    timer_client::{LocalBoxTimerClient, LocalBoxTimerCmd},
    timer_manager::LocalBoxTimerManager,
};

pub mod client;
pub mod options;

pub use {
    client::ActiveProxyClient as Client,
    options::{AnnouncePeerHandler, Options, OptionsBuilder},
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
