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

## 2. Compiling the project

The project is a Cargo workspace-style Rust crate with the main library exposed from `src/lib.rs`.

The top-level source modules are:

| Module | Description |
| --- | --- |
| `src/core` | Core primitives such as `Id`, signatures, encryption helpers, `NodeInfo`, `PeerInfo`, and `Value`. |
| `src/did` | DID documents, credentials, resolvers, registries, proofs, and related DID utilities. |
| `src/dht` | DHT node implementation, routing, storage, UDP server, lookup tasks, and node runtime. |
| `src/director` | Director options and client APIs for working with a Boson super node. |
| `src/messaging` | Messaging client configuration, contacts, channels, sessions, and message-related APIs. |
| `src/activeproxy` | Active proxy configuration, client, connection, and service integration. |

Build the full project:

```bash
cargo build
```

Check the library modules:

```bash
cargo check --lib
```

Run tests:

```bash
cargo test
```

Build or check a specific application binary:

```bash
cargo check --bin shell
cargo check --bin launcher
cargo check --bin chat
```

The default feature set includes development/inspection support as configured in `Cargo.toml`.

## 3. Main applications

### Shell

The shell application provides an interactive DHT and Director-oriented command shell.

```bash
cargo run --bin shell -- --config apps/shell/node.yaml
```

Useful options include:

```bash
cargo run --bin shell -- \
  --config apps/shell/node.yaml \
  --port 39001 \
  --datadir .dev_shell_data
```

Inside the shell, use `help` to list available commands.

### Launcher

The launcher application starts the Boson node and active proxy service using separate node and active proxy configuration files.

```bash
cargo run --bin launcher -- \
  --config apps/launcher/node.yaml \
  --activeproxy-config apps/launcher/activeproxy.yaml \
  --director-url https://127.0.0.1:9000
```

The node configuration is used to construct node runtime options, while the active proxy configuration provides service, user, device, and upstream settings.

### Chat

The chat application starts a DHT node and an interactive messaging shell. It can be run in multiple terminals with different ports, data directories, user keys, and device keys to simulate multiple users.

```bash
cargo run --bin chat -- \
  --config apps/chat/alice.yaml \
  --node-config apps/chat/node.yaml \
  --port 39011 \
  --datadir .dev_alice_data/node
```

Example for a second instance:

```bash
cargo run --bin chat -- \
  --config apps/chat/bob.yaml \
  --node-config apps/chat/node.yaml \
  --port 39012 \
  --datadir .dev_bob_data/node
```

The chat application also supports direct messaging service arguments:

```bash
cargo run --bin chat -- \
  --peerid PEERID \
  --endpoint ENDPOINT \
  --userid USER_PRIVATE_KEY \
  --dev-key DEVICE_PRIVATE_KEY \
  --node-config apps/chat/node.yaml \
  --port 39011 \
  --datadir .dev_chat_data/node
```

## 4. License

This project is open source software licensed under the MIT License.

You may use, copy, modify, merge, publish, distribute, sublicense, and/or sell copies of the software, subject to the terms and conditions of the MIT License.
