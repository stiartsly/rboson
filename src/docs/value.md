# `src/core/value.rs`

`Value` is the DHT value payload type. It supports immutable values, signed mutable values, and encrypted signed mutable values.

## Value kinds

| Kind | Builder | Public key | Signature | Recipient | Data |
| --- | --- | --- | --- | --- | --- |
| Immutable | `ImmutableBuilder` | no | no | no | plaintext |
| Signed mutable | `SignedBuilder` | yes | yes | no | plaintext |
| Encrypted mutable | `EncryptedBuilder` | yes | yes | yes | ciphertext |

All builders reject empty data.

## Immutable values

Immutable values are identified by the SHA-256 hash of their data. They do not contain a private key, public key, signature, nonce, or recipient.

```rust
use boson::ImmutableBuilder;

let value = ImmutableBuilder::new(b"hello").build()?;
assert!(!value.is_mutable());
assert!(!value.is_signed());
assert!(!value.is_encrypted());
```

## Signed mutable values

Signed values are identified by the owner's public key ID. They include a nonce and signature.

```rust
use boson::{SignedBuilder, signature::KeyPair};

let keypair = KeyPair::random();
let mut builder = SignedBuilder::new(b"hello");
let value = builder
    .with_keypair(&keypair)
    .with_sequence_number(1)
    .build()?;

assert!(value.is_mutable());
assert!(value.is_signed());
assert!(value.is_valid());
```

If no key pair or nonce is supplied, the builder generates them automatically.

## Encrypted mutable values

Encrypted values are signed by the sender and encrypted to a recipient `Id`. The recipient ID is converted to an X25519 public key internally.

```rust
use boson::{EncryptedBuilder, Id, signature::KeyPair};

let sender = KeyPair::random();
let recipient = Id::from(KeyPair::random().public_key());
let mut builder = EncryptedBuilder::new(b"secret", &recipient);
let value = builder.with_keypair(&sender).build()?;

assert!(value.is_encrypted());
assert!(value.is_signed());
assert!(value.is_valid());
```

The stored `data()` for encrypted values is ciphertext in the `cryptobox` output format.

## Identity and validity

`Value::id()` returns:

- SHA-256(data) for immutable values.
- SHA-256(public-key-id-bytes) for mutable values.

`is_valid()` checks:

- data is non-empty.
- immutable values are accepted as-is.
- mutable values have public key, signature, and nonce.
- mutable signatures verify against the serialized signature digest.

## Ownership

Values built locally may carry the private key used to sign them:

```rust
if value.has_private_key() {
    let owner_key = value.private_key();
}
```

Deserialized values are packed without private keys.

## Serialization

`Value` serializes through compact wire fields:

| Wire field | Meaning |
| --- | --- |
| `k` | optional public key ID |
| `rec` | optional recipient ID |
| `n` | optional nonce |
| `s` | optional signature |
| `v` | value bytes, plaintext or ciphertext |
| `seq` | sequence number |

Deserialization rejects empty data, encrypted values without a public key, mutable values missing nonce/signature, and values whose signatures fail verification.

