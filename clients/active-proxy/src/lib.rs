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

// Migrated code that is not documented yet. Remove this allow as the crate is brought in line
// with the workspace lints.
#![allow(missing_docs)]

mod connection;
mod connection_handler;
mod connection_registry;
mod packet;
mod packet_type;
mod session;
mod state;
mod verticle;

#[cfg(test)]
#[path = "unitests/bench_relay.rs"]
mod bench_relay;

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
    boson_api::random::secure_u32() % 32
}

pub(crate)
fn random_boolean(input: bool) -> u8 {
    let val = boson_api::random::secure_u32() as u8;

    match input {
        true => val | 0x01,
        false => val & 0xFE
    }
}
*/
pub(crate) fn random_timeshift() -> u32 {
    // max is 10s
    boson_api::random::secure_u32() % 10
}
