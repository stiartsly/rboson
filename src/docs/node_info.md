# `src/core/node_info.rs`

`NodeInfo` describes a DHT node's stable identity and reachable socket addresses.

## Fields and invariants

Conceptually, a `NodeInfo` contains:

- `id`: the node's 32-byte Boson `Id`.
- optional IPv4 `SocketAddr`.
- optional IPv6 `SocketAddr`.
- a default/preferred `Network` family.

At least one address must be present. Address family and port are validated:

- IPv4 entries must be IPv4 addresses.
- IPv6 entries must be IPv6 addresses.
- port `0` is rejected.

## Construction

Use `NodeInfo::new()` for one address.

```rust
use boson::{Id, NodeInfo};

let id = Id::random();
let node = NodeInfo::new(id, "127.0.0.1:39001".parse()?);
```

Use `NodeInfo::with_addresses()` when both IPv4 and IPv6 may be available.

```rust
let node = NodeInfo::with_addresses(
    id,
    Some("127.0.0.1:39001".parse()?),
    Some("[::1]:39001".parse()?),
)?;
```

If both addresses are present, IPv4 becomes the preferred/default family.

## Accessors

Common accessors:

```rust
let id = node.id();
let default_addr = node.address();
let default_ip = node.ip();
let default_port = node.port();
```

Family-specific accessors:

```rust
use boson::Network;

let addr4 = node.address4();
let addr6 = node.address6();
let port4 = node.port4();
let port6 = node.port6();
let ipv4_addr = node.address_for(Network::IPv4);
```

Use `has_address4()`, `has_address6()`, and `has_multi_addresses()` when selecting transport families.

## Narrowing to one family

`narrow_down(family)` returns a `NodeInfo` containing only the requested family address. It panics if that family is not available, so check with `has_address()` first when the input is uncertain.

```rust
if node.has_address(Network::IPv4) {
    let ipv4_only = node.narrow_down(Network::IPv4);
}
```

## Matching and equality

`PartialEq` is strict: IDs and both address slots must match.

`matches()` is looser and returns true if either the node ID or any available socket address conflicts. Use it for collision/conflict detection in routing logic.

## Serialization

`NodeInfo` serializes as a compact tuple:

- `id, ip, port` for one address.
- `id, ip4, port4, ip6, port6` for dual-stack nodes.

Human-readable serializers encode IP addresses as strings. Binary serializers encode IP addresses as raw octets.

