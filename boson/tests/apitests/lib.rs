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

// The API tests call libsodium directly; the workspace denies unsafe code elsewhere.
#![allow(unsafe_code)]

#[cfg(test)]
mod core {
    mod cryptobox;
    mod id;
    mod node_info;
    mod peer_info;
    mod signature;
    mod value;
}

#[cfg(test)]
mod dht {
    mod node;
    mod node_options;
}

#[cfg(test)]
mod activeproxy {
    mod client;
    mod options;
}

#[cfg(test)]
mod did {
    mod card;
    mod credential;
    mod diddoc;
    mod didurl;
    mod registry;
    mod resolution_cache;
    mod resolver;
    mod vc;
    mod verification_method;
    mod vouch;
    mod vp;
}

#[cfg(test)]
mod director {
    mod client;
    mod options;
}

#[cfg(test)]
mod messaging {
    mod client;
    mod message_builder;
    mod options;
}
// helper function
fn randomize_bytes<const N: usize>(array: &mut [u8; N]) {
    unsafe {
        libsodium_sys::randombytes_buf(array.as_mut_ptr() as *mut libc::c_void, N);
    }
}

fn create_random_bytes(len: usize) -> Vec<u8> {
    let mut bytes = vec![0u8; len];
    unsafe {
        libsodium_sys::randombytes_buf(bytes.as_mut_ptr() as *mut libc::c_void, len);
    }
    bytes
}

fn remove_working_path(input: &str) {
    if std::fs::metadata(&input).is_ok() {
        match std::fs::remove_dir_all(&input) {
            Ok(_) => {}
            Err(e) => {
                panic!("Failed to remove directory: {}", e);
            }
        }
    }
}

fn main() {}
