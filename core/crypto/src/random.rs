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

use libsodium_sys::randombytes_buf;

pub fn random_array_secure<const N: usize>(array: &mut [u8; N]) {
    unsafe {
        randombytes_buf(
            array.as_mut_ptr() as *mut libc::c_void,
            N
        );
    }
}

pub fn secure_array<const N: usize>() -> [u8; N] {
    let mut array = [0u8; N];
    random_array_secure(&mut array);
    array
}

pub fn random_bytes_secure(buffer: &mut [u8]) {
    if buffer.is_empty() {
        return;
    }
    unsafe {
        randombytes_buf(
            buffer.as_mut_ptr() as *mut libc::c_void,
            buffer.len(),
        );
    }
}

pub fn secure_bytes(length: usize) -> Vec<u8> {
    let mut bytes = vec![0u8; length];
    random_bytes_secure(&mut bytes);
    bytes
}

pub fn bytes(length: usize) -> Vec<u8> {
    let mut bytes = vec![0u8; length];
    rand::fill(bytes.as_mut_slice());
    bytes
}

pub fn fill(buffer: &mut [u8]) {
    rand::fill(buffer);
}
