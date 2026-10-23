# `src/core/cryptobox.rs`

The `cryptobox` module wraps libsodium `crypto_box` primitives for public-key authenticated encryption. It uses X25519 key exchange, a 24-byte nonce, and a 16-byte MAC.

## Main types

| Type | Size | Purpose |
| --- | ---: | --- |
| `cryptobox::PrivateKey` | 32 bytes | X25519 private key. |
| `cryptobox::PublicKey` | 32 bytes | X25519 public key. |
| `cryptobox::KeyPair` | private + public | X25519 key pair. |
| `Nonce` | 24 bytes | Per-message nonce. |
| `CryptoBox` | 32 bytes | Precomputed shared symmetric key from remote public key + local private key. |

The key, nonce, and precomputed box types clear internal byte buffers in `Drop`.

## Key creation and conversion

Create a native encryption key pair with `KeyPair::random()` or `KeyPair::try_from_seed()`.

```rust
use boson::cryptobox;

let encryption_keypair = cryptobox::KeyPair::random();
```

The module can also convert Ed25519 signing keys to X25519 encryption keys. This is used when a Boson `Id` or signature identity must be reused for encrypted transport.

```rust
use boson::{cryptobox, signature};

let signing_keypair = signature::KeyPair::random();
let encryption_keypair = cryptobox::KeyPair::from(&signing_keypair);
```

## Nonces

Use a unique nonce per `(sender private key, recipient public key)` encryption context.

```rust
use boson::cryptobox::Nonce;

let nonce = Nonce::random();
```

`Nonce::increment()` mutates the nonce in place and is useful when generating a sequence of nonces from a random starting point.

## One-shot encryption

The encrypted output format produced by this module is:

```text
nonce || ciphertext || mac
```

The nonce is copied into the first `Nonce::BYTES` bytes of the output buffer, so callers do not need to store it separately.

```rust
use boson::cryptobox;

let sender = cryptobox::KeyPair::random();
let recipient = cryptobox::KeyPair::random();
let nonce = cryptobox::Nonce::random();

let cipher = cryptobox::encrypt_into(
    b"secret",
    &nonce,
    recipient.public_key(),
    sender.private_key(),
)?;

let plain = cryptobox::decrypt_into(
    &cipher,
    sender.public_key(),
    recipient.private_key(),
)?;
assert_eq!(plain, b"secret");
```

## Precomputed `CryptoBox`

Use `CryptoBox` when encrypting multiple messages to the same peer.

```rust
use boson::cryptobox::CryptoBox;

let shared = CryptoBox::try_from((recipient.public_key(), sender.private_key()))?;
let cipher = shared.encrypt_into(b"secret", &nonce)?;
let plain = shared.decrypt_into(&cipher)?;
```

## Buffer sizing

For encryption, the destination buffer must be at least:

```text
plain.len() + CryptoBox::MAC_BYTES + Nonce::BYTES
```

For decryption, the destination buffer must be at least:

```text
cipher.len() - CryptoBox::MAC_BYTES - Nonce::BYTES
```

Use `encrypt_into()` and `decrypt_into()` when the module should allocate the buffers.

