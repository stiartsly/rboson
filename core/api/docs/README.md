# Core developer documents

This directory contains developer notes for the public building blocks in `core/api/src`.

- [id](./id.md): 32-byte Boson identifiers, encodings, DID strings, and distance helpers.
- [NodeInfo](./node_info.md): DHT node identity and IPv4/IPv6 socket metadata.
- [PeerInfo](./peer_info.md): signed service peer advertisements.
- [Value](./value.md): immutable, signed, and encrypted DHT values.

The cryptographic building blocks (`signature`, `cryptobox`) live in `boson-crypto`; `boson-api`
re-exports them. Their notes are in [core/crypto/docs](../../crypto/docs/README.md).
