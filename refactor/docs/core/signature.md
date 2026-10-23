# `src/core/signature.rs`

The `signature` module wraps libsodium Ed25519 signing primitives and provides the identity key material used throughout Boson.

## Main types

| Type | Size | Purpose |
| --- | ---: | --- |
| `PrivateKey` | 64 bytes | Ed25519 private key used to sign data. |
| `PublicKey` | 32 bytes | Ed25519 public key used to verify signatures. |
| `KeyPair` | private + public | Convenience wrapper for an Ed25519 private/public key pair. |
| `Signature` | 64 bytes output | Streaming signature state and final signature verification/signing helper. |

`PrivateKey`, `PublicKey`, `KeyPair`, and `Signature` clear their internal byte buffers in `Drop`.

## Key creation and parsing

Use `KeyPair::random()` for new persistent identities and `KeyPair::try_from_seed()` for deterministic test vectors or reproducible identities.

```rust
use boson::signature::KeyPair;

let keypair = KeyPair::random();
let public_key = keypair.public_key();
let private_key = keypair.private_key();
```

`PrivateKey` accepts:

- `0x`-prefixed hex strings.
- Base58 strings without a `0x` prefix.
- raw byte slices with exactly `PrivateKey::BYTES` bytes.

```rust
use boson::signature::{KeyPair, PrivateKey};

let private_key = PrivateKey::try_from("0x...")?;
let keypair = KeyPair::from(private_key);
```

## One-shot signing

Use the free functions when the entire payload is already available.

```rust
use boson::signature;

let signature = signature::sign_into(b"hello", keypair.private_key())?;
let verified = signature::verify(b"hello", &signature, keypair.public_key())?;
assert!(verified);
```

`PrivateKey::sign()` and `signature::sign()` require the destination signature buffer to be exactly `Signature::BYTES` bytes.

## Streaming signing

Use `Signature` when data is processed in chunks.

```rust
use boson::signature::Signature;

let mut state = Signature::new();
let signature = state
    .reset()
    .update(b"hello ")
    .update(b"world")
    .sign_into(keypair.private_key())?;
```

Verification uses the same chunking pattern.

```rust
let verified = Signature::new()
    .reset()
    .update(b"hello ")
    .update(b"world")
    .verify(&signature, keypair.public_key())?;
assert!(verified);
```

## Relationship to `Id`

A Boson `Id` can be created from a signature public key. This is the common pattern for deriving stable user, device, peer, and node IDs from Ed25519 identities.

```rust
use boson::Id;

let id = Id::from(keypair.public_key());
```

## Error behavior

The module returns repository-standard `Error` values:

- `ArgumentError` for malformed input sizes or invalid encoded strings.
- `CryptoError` for cryptographic operation failures or invalid signature buffer sizes.

