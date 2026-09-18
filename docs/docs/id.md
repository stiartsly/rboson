# `src/core/id.rs`

`Id` is the canonical 32-byte identifier type used across Boson for nodes, peers, users, devices, values, and DIDs.

## Shape

| Constant | Value | Meaning |
| --- | ---: | --- |
| `Id::BYTES` | 32 | Serialized byte length. |
| `Id::BITS` | 256 | Bit length used by DHT distance/routing logic. |
| `DID_PREFIX` | `did:boson:` | Prefix used by `to_did_string()`. |

`Id` is `Copy`, hashable, orderable, and serializable.

## Construction

```rust
use boson::Id;

let random = Id::random();
let zero = Id::zero();
let min = Id::MIN_ID;
let max = Id::MAX_ID;
```

Parsing supports both base58 and `0x`-prefixed hex:

```rust
let id = Id::try_from("5MjQxzgK7awC4He1WX4b5d1cLbY14dm8hvHNkMBU8ELK")?;
let same = Id::try_from("0x...")?;
```

Use `Id::try_from_bytes()` or `TryFrom<&[u8]>` for raw byte input. The input must be exactly 32 bytes.

## Display and encoding

`Display` prints base58.

```rust
let base58 = id.to_base58();
let hex = id.to_hexstr();
let did = id.to_did_string();
let short = id.to_abbr_str();
```

Human-readable serde formats serialize as base58 strings. Non-human-readable serde formats serialize as raw bytes.

## Relationship to keys

An `Id` can be derived directly from an Ed25519 signature public key.

```rust
use boson::{signature::KeyPair, Id};

let keypair = KeyPair::random();
let id = Id::from(keypair.public_key());
```

`Id` can be converted back to an Ed25519 public key or an X25519 encryption public key:

```rust
let signing_public_key = id.to_signature_key();
let encryption_public_key = id.to_encryption_key();
```

These conversions assume the ID bytes represent a valid Ed25519 public key.

## DHT distance helpers

`Id::distance()` computes XOR distance, which is the Kademlia-style metric used by the DHT.

```rust
let distance = local_id.distance(&remote_id);
```

`Id::distance_between(a, b)` is an equivalent associated helper.

Internal routing helpers such as bit comparison and prefix copying live in this module and are used by routing-table code.

