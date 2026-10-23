# `src/core/peer_info.rs`

`PeerInfo` is a signed service advertisement. It tells the DHT and other nodes where a higher-level service can be reached and proves ownership with an Ed25519 signature.

## Builder

Create peers through `PeerInfo::builder(endpoint)`.

```rust
use boson::{PeerInfo, signature::KeyPair};

let keypair = KeyPair::random();
let peer = PeerInfo::builder("https://service.example:443")
    .with_key(keypair)
    .with_sequence_number(1)
    .build()?;
```

The builder supports:

- `with_key()` or `with_private_key()` to provide stable peer identity.
- `with_sequence_number()` for monotonic updates.
- `with_fingerprint()` for service-specific versioning or grouping.
- `with_extra()` for opaque service metadata.
- `with_node()` to add a node-level authentication signature.

If no peer key is supplied, a random key is generated and embedded in the returned `PeerInfo`.

## Identity and signatures

The peer ID is derived from the peer signing public key:

```rust
let peer_id = peer.id();
```

The primary peer signature covers:

- peer public key ID.
- sequence number.
- optional authenticating node ID and node signature.
- fingerprint.
- endpoint.
- optional extra bytes.

`is_valid()` verifies signature shape and signature correctness.

## Node-authenticated peers

When `with_node()` is used, `PeerInfo` also stores:

- `nodeid`: the DHT node ID that authenticated the peer.
- `node_sig`: a signature by that node over the peer ID, node ID, fingerprint, and sequence.

Use:

```rust
if peer.is_authenticated() {
    let node_id = peer.nodeid();
    let node_signature = peer.node_signature();
}
```

## Updating

`PeerInfo::update()` creates a new signed advertisement with an incremented sequence number. It requires the private key to be present, so it only works on owned peer infos.

```rust
let updated = peer.update("https://service.example:8443", None, None)?;
assert_eq!(updated.sequence_number(), peer.sequence_number() + 1);
```

If the peer was node-authenticated, the update must be authenticated by the same node.

## Public vs owned representation

Use `without_private_key()` before storing or sharing a peer advertisement publicly.

```rust
let public_peer = peer.without_private_key();
assert!(!public_peer.has_private_key());
assert!(public_peer.is_valid());
```

## Serialization

`PeerInfo` serializes through a compact internal representation:

| Wire field | Meaning |
| --- | --- |
| `id` | peer ID / public key bytes |
| `seq` | sequence number |
| `o` | optional owner node ID |
| `os` | optional owner node signature |
| `sig` | peer signature |
| `f` | fingerprint |
| `e` | endpoint |
| `ex` | optional extra bytes |

Deserialization rejects missing endpoints, invalid signature lengths, mismatched node authentication fields, and failed signature verification.

