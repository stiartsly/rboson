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

// Ported from Boson.Native.

use crate::key_derivation::{context_bytes, derive_key, MASTER_KEY_BYTES};

#[test]
fn derives_deterministically_and_enforces_inputs() {
    let key = [7u8; MASTER_KEY_BYTES];
    let context = *b"BosonKdf";
    assert_eq!(derive_key(&key, 1, &context, 32).unwrap(), derive_key(&key, 1, &context, 32).unwrap());
    assert_ne!(derive_key(&key, 1, &context, 32).unwrap(), derive_key(&key, 2, &context, 32).unwrap());
    assert!(derive_key(&key[..31], 1, &context, 32).is_err());
    assert!(derive_key(&key, 1, &context[..7], 32).is_err());
    assert!(derive_key(&key, 1, &context, 8).is_err());
    assert!(context_bytes("").is_err());
}
