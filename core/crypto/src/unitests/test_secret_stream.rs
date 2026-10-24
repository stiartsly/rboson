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

// Ported from the Java SDK's SecretStreamTests, plus tampering, ordering and truncation.

use crate::{
    random::secure_bytes,
    secret_stream::{decryption_stream, encryption_stream, ABYTES, HEADER_BYTES, KEY_BYTES},
};

fn encrypt(key: &[u8], data: &[u8], chunk: usize, additional: Option<&[u8]>) -> (Vec<u8>, Vec<Vec<u8>>) {
    let mut stream = encryption_stream(key).unwrap();
    let header = stream.header().to_vec();
    assert_eq!(header.len(), HEADER_BYTES);
    let chunks: Vec<&[u8]> = data.chunks(chunk).collect();
    let mut ciphers = Vec::new();
    for (i, plain) in chunks.iter().enumerate() {
        let cipher = if i == chunks.len() - 1 {
            stream.push_last(plain, additional).unwrap()
        } else {
            stream.push(plain, additional).unwrap()
        };
        assert_eq!(cipher.len(), plain.len() + ABYTES);
        ciphers.push(cipher);
    }
    assert!(stream.is_complete());
    (header, ciphers)
}

#[test]
fn encrypts_and_decrypts_a_stream() {
    for additional in [None, Some(b"Test additional data".as_slice())] {
        let key = secure_bytes(KEY_BYTES);
        let data = secure_bytes(1024 * 1024 + 123);
        let (header, ciphers) = encrypt(&key, &data, 4096, additional);

        let mut stream = decryption_stream(&header, &key).unwrap();
        let mut decrypted = Vec::new();
        for cipher in &ciphers {
            assert!(!stream.is_complete());
            let plain = stream.pull(cipher, additional).unwrap();
            assert_eq!(plain.len(), cipher.len() - ABYTES);
            decrypted.extend_from_slice(&plain);
        }
        assert!(stream.is_complete());
        assert_eq!(decrypted, data);
    }
}

#[test]
fn invalid_key_and_header_lengths_are_refused() {
    let key = secure_bytes(KEY_BYTES);
    assert!(encryption_stream(&[0u8; 16]).is_err());
    assert!(decryption_stream(&[0u8; 10], &key).is_err());
    let header = encryption_stream(&key).unwrap().header().to_vec();
    assert!(decryption_stream(&header, &[0u8; 16]).is_err());
}

#[test]
fn a_completed_stream_takes_no_more() {
    let key = secure_bytes(KEY_BYTES);
    let mut enc = encryption_stream(&key).unwrap();
    let cipher = enc.push_last(b"Hello World", None).unwrap();
    assert!(enc.is_complete());
    assert!(enc.push(b"Hello World", None).is_err());

    let mut dec = decryption_stream(enc.header(), &key).unwrap();
    assert_eq!(dec.pull(&cipher, None).unwrap(), b"Hello World");
    assert!(dec.is_complete());
    assert!(dec.pull(&cipher, None).is_err());
}

#[test]
fn tampering_reordering_and_wrong_data_are_detected() {
    let key = secure_bytes(KEY_BYTES);
    let data = secure_bytes(3 * 100);
    let (header, ciphers) = encrypt(&key, &data, 100, Some(b"ad"));

    let pull_first = |cipher: &[u8], additional: Option<&[u8]>, key: &[u8]| {
        decryption_stream(&header, key).unwrap().pull(cipher, additional)
    };
    assert!(pull_first(&ciphers[0], Some(b"ad"), &key).is_ok());
    assert!(pull_first(&ciphers[0], Some(b"other"), &key).is_err());
    assert!(pull_first(&ciphers[0], None, &key).is_err());
    assert!(pull_first(&ciphers[1], Some(b"ad"), &key).is_err());
    assert!(pull_first(&ciphers[0], Some(b"ad"), &secure_bytes(KEY_BYTES)).is_err());

    let mut tampered = ciphers[0].clone();
    tampered[5] ^= 1;
    assert!(pull_first(&tampered, Some(b"ad"), &key).is_err());
    assert!(pull_first(&ciphers[0][..ABYTES - 1], Some(b"ad"), &key).is_err());
}

#[test]
fn a_truncated_stream_is_not_complete() {
    let key = secure_bytes(KEY_BYTES);
    let (header, ciphers) = encrypt(&key, &secure_bytes(300), 100, None);
    let mut stream = decryption_stream(&header, &key).unwrap();
    stream.pull(&ciphers[0], None).unwrap();
    stream.pull(&ciphers[1], None).unwrap();
    assert!(!stream.is_complete());
}
