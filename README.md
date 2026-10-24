# Boson Network Rust Implementation

This repository contains a Rust implementation of core Boson Network components and several application entry points for running and interacting with those components.

## 1. Boson Network introduction

Boson Network is a decentralized networking stack built around peer identity, secure communication, distributed discovery, and service publication.

At a high level, this project provides:

- Typed 256-bit identities for nodes, peers, users, devices, and values.
- Cryptographic primitives for signing, identity derivation, and encrypted communication.
- A Kademlia-style DHT for node discovery, peer discovery, and value storage.
- DID-related primitives and resolution support.
- Director client support for interacting with a Boson super node.
- Messaging and application-level service integration built on top of the core network components.

## 2. Layout

| Folder | Crate | What it is |
|---|---|---|
| `boson/` | `boson` | Facade: re-exports the crates below, node and client crates behind features |
| `core/crypto/` | `boson-crypto` | Signatures, key agreement, crypto box, hashing, key derivation |
| `core/api/` | `boson-api` | Id, Value, PeerInfo, NodeInfo, the `Node` trait, CWT |
| `core/identifier/` | `boson-identifier` | DID documents, credentials, cards, vouches, registry |
| `core/dht/` | `boson-dht` | Regular DHT node |
| `core/http/` | `boson-http` | HTTPS transport shared by the clients |
| `clients/higgs/` | `boson-higgs` | `Node` over a super node's web gateway |
| `clients/director/` | `boson-director-client` | Director client |
| `clients/ion-store/` | `boson-ion-store-client` | IonStore client |
| `clients/active-proxy/` | `boson-active-proxy-client` | Active proxy client |
| `clients/messaging/` | `boson-messaging-client` | Messaging client |
| `tools/common/` | `boson-tools-common` | Shared code of the tools |
| `tools/cli/` | `boson-cli` | Command-line client |
| `tools/director-cli/` | `boson-director-cli` | Director administration |
| `tools/node/` | `boson-node` | Regular node daemon |
| `interop/` | | Test vectors and integration tests against the Java implementation |

The `boson` facade also keeps the public API of the earlier single `rboson` crate (`boson::Id`,
`boson::dht::Node`, `boson::did`, ...), so code written against it builds unchanged.

Work in progress in this layout:

- Cryptography still lives in `boson-api`; `boson-crypto` gets it once it is split out.
- `Node` is still the concrete DHT node in `boson-dht`. A `Node` abstraction in `boson-api`, like
  the Java `Node` interface, comes later; then the client crates depend on it instead of on a node
  implementation, and the application chooses `boson-dht` or `boson-higgs`.
- `boson-crypto`, `boson-http`, `boson-higgs` and `boson-ion-store-client` are still empty.

## 3. Compiling the project

```bash
cargo build
cargo clippy --all-targets --all-features
```

Run the tests:

```bash
cargo test                   # unit tests of every crate
cargo test --all-features    # also the API tests in boson/tests/apitests
```

The API tests use the whole stack through the `boson` facade, so they need all of its features.
They start DHT nodes on fixed local ports.

`boson-dht` enables its `devp` feature by default: developer mode, which lets the node use private
(bogon) addresses.

## 4. Tools

The tools ship with the Rust SDK and double as sample code for Rust developers. They have the same
commands as the Java tools of the Boson Java SDK. Commands without a Rust implementation yet say so
(`<tool> <command>: not implemented yet`, exit code 2).

| Tool | Implemented | Not implemented yet |
|---|---|---|
| `boson-cli` | `user register`, `user show`, `device add`, `node status`, `proxy start`, `util keygen` | the rest of `user`, `device`, `ionstore`, `dht`, `turn`, `node`, `config`, `identity`, `util` |
| `boson-node` | `run`, `shell` | `setup`, `cache`, `id`, `config` |
| `boson-director-cli` | | all of `user`, `device`, `plan`, `feature`, `subscription`, `blacklist`, `federation`, `node`, `config`, `identity` |

Every command has `--help`, for example `cargo run -p boson-cli -- user --help`.

### boson-node

Run a DHT node. It keeps its key and database in the data directory, and stops after 10 minutes:

```bash
cargo run -p boson-node -- run \
  -d .sample_node \
  -p 39088 \
  -b NODEID@IP:PORT
```

Explore the DHT from an interactive shell:

```bash
cargo run -p boson-node -- shell --config tools/node/shell/node.yaml
```

Set `privateKey` in `tools/node/shell/node.yaml` first; it ships with a placeholder. Inside the
shell, `help` lists the commands: `id`, `find node|value|peer`, `store value`,
`announce peer`, `routing` and `log`; `exit` leaves it. `tools/node/shell/` also holds the
configurations of three more shell nodes (`announce_peer`, `announce_value`, `query`), each with
its own port, key and data directory, to run several shells on one machine.

### boson-cli

The account commands talk to the Director of a super node, given by `--director-url` or
`$BOSON_DIRECTOR_URL`:

```bash
export BOSON_DIRECTOR_URL=https://127.0.0.1:9000

cargo run -p boson-cli -- node status
cargo run -p boson-cli -- user register USER_PRIVATE_KEY --name Alice
cargo run -p boson-cli -- user show -u USER_PRIVATE_KEY --devices
cargo run -p boson-cli -- device add DEVICE_PRIVATE_KEY -u USER_PRIVATE_KEY --device-name laptop
```

Generate a user identity or a device key, offline:

```bash
cargo run -p boson-cli -- util keygen --user
cargo run -p boson-cli -- util keygen --device
```

Expose a local service through the super node's Active Proxy, until Ctrl+C:

```bash
cargo run -p boson-cli -- proxy start --config tools/cli/proxy.yaml
```

### boson-director-cli

The administration tool of a super node. None of its commands has a Rust implementation yet.

## 5. Examples

### Messaging chat

`clients/messaging/examples/chat` is an interactive terminal chat over the messaging client: friend
requests, channels and invite tickets, text messages, and every client event as it happens. Run two
instances, for example as Alice and Bob, against a messaging service. Their configurations take the
service and the keys from environment variables:

```bash
export MESSAGING_SERVICE_PEER_ID=SERVICE_PEER_ID
export MESSAGING_SERVICE_ENDPOINT=mqtts://HOST:PORT

ALICE_USER_PRIVATE_KEY=... ALICE_DEVICE_PRIVATE_KEY=... \
  cargo run -p boson-messaging-client --example chat -- -c clients/messaging/examples/chat/alice.yaml
BOB_USER_PRIVATE_KEY=... BOB_DEVICE_PRIVATE_KEY=... \
  cargo run -p boson-messaging-client --example chat -- -c clients/messaging/examples/chat/bob.yaml
```

`boson-cli util keygen --user` and `--device` generate the keys.

Inside the chat, `help` lists the commands; `exit` leaves it.

## 6. License

This project is open source software licensed under the MIT License.

You may use, copy, modify, merge, publish, distribute, sublicense, and/or sell copies of the software, subject to the terms and conditions of the MIT License.
