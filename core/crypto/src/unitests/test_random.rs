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

use crate::random::{random_bytes_secure, secure_bytes, secure_u32, secure_uniform};

#[test]
fn uniform_stays_below_its_bound() {
    for bound in [2u32, 10, 255, 256, 1000] {
        for _ in 0..1000 {
            assert!(secure_uniform(bound) < bound);
        }
    }
    // libsodium returns 0 for bounds below 2.
    assert_eq!(secure_uniform(0), 0);
    assert_eq!(secure_uniform(1), 0);
}

#[test]
fn values_vary() {
    let values: std::collections::HashSet<u32> = (0..64).map(|_| secure_u32()).collect();
    assert!(values.len() > 60);
    assert_ne!(secure_bytes(32), secure_bytes(32));

    let mut empty = [0u8; 0];
    random_bytes_secure(&mut empty);
}
