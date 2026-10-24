# Crypto developer documents

This directory contains developer notes for the cryptographic building blocks in `core/crypto/src`.
`boson-api` re-exports them, so they are also reachable as `boson_api::signature` and
`boson_api::cryptobox`.

- [signature](./signature.md): Ed25519 signing keys, signatures, and streaming signatures.
- [cryptobox](./cryptobox.md): X25519 public-key encryption helpers and nonce handling.
