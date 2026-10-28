# Boson CLI: keys, Director registration, and Active Proxy

Minimal end-to-end guide for `boson-cli` from this repository. Run all commands from the repository root.

## Prerequisites

- Rust 1.92 (pinned by `rust-toolchain.toml`).
- A Director URL.
- A Boson Active Proxy service peer ID, host, and port.
- A local upstream service. The examples use an HTTP server on `127.0.0.1:8888`.

```bash
cargo build -p boson-cli
```

`--insecure` is only needed when the Director uses a self-signed certificate. Remove it for a CA-signed certificate.

## 1. Generate a user key and a device key

Generate both keys in one command:

```bash
cargo run -p boson-cli -- util keygen --user --device
```

The output contains:

- **User ID** and the user private key.
- **Device ID** and the device private key.

Save the values as shell variables (use the actual values from the output):

```bash
export USER_ID='<User ID>'
export USER_PRIVATE_KEY='<user private key>'
export DEVICE_PRIVATE_KEY='<device private key>'
```

Keep both private keys secret. They can be supplied as `0x`-prefixed hex or Base58.

If the keys already exist, skip this step and reuse them.

## 2. Register the user and add the device to the Director

Set the Director URL:

```bash
export BOSON_DIRECTOR_URL='https://<director-host>:9000'
# or: export BOSON_DIRECTOR_URL="$VANILLA_DIRECTOR_URL"
```

### 2.1 Register the user

```bash
cargo run -p boson-cli -- user register "$USER_PRIVATE_KEY" \
  --name 'proxy-user' \
  --email 'proxy-user@example.com' \
  --insecure
```

Skip this command if the user is already registered. If the account has a passphrase, add `--passphrase '<passphrase>'`.

### 2.2 Add the device

```bash
cargo run -p boson-cli -- device add "$DEVICE_PRIVATE_KEY" \
  -u "$USER_PRIVATE_KEY" \
  --device-name 'proxy' \
  --app 'BosonApp' \
  --insecure
```

If the account has a passphrase, add `--passphrase '<passphrase>'`.

### 2.3 Verify

```bash
cargo run -p boson-cli -- user show "$USER_ID" \
  -u "$USER_PRIVATE_KEY" \
  --devices \
  --insecure
```

The device ID from step 1 should appear in the device list.

## 3. Use the Active Proxy

### 3.1 Configure the proxy

Edit `tools/cli/proxy.yaml`:

```yaml
service:
  peerId: <active-proxy-service-peer-id>
  host: <active-proxy-service-host>
  port: 9090

client:
  userId: ${BOSON_PROXY_USER_ID}
  devicePrivateKey: ${BOSON_PROXY_DEVICE_KEY}

upstream:
  host: 127.0.0.1
  port: 8888
  scheme: http://

nameAccess: false
announcePeer: false

logLevel: info
logFile: /tmp/boson-proxy.log
logConsole: true
```

Export the configuration values:

```bash
export BOSON_PROXY_USER_ID="$USER_ID"
export BOSON_PROXY_DEVICE_KEY="$DEVICE_PRIVATE_KEY"
```

Alternatively, replace the `${...}` references with the literal values.

Notes for the current implementation:

- `service.host` is required; `service.port` defaults to `9090`.
- Keep `upstream.scheme: http://` for the sample HTTP upstream. The current upstream resolver does not accept the default `tcp://`.
- Keep `announcePeer: false`. The CLI does not install an announce-peer handler.
- `nameAccess` depends on the service and subscription; leave it `false` initially.

### 3.2 Start a test upstream

In another terminal:

```bash
python3 -m http.server 8888 --bind 127.0.0.1
```

### 3.3 Start the proxy

```bash
cargo run -p boson-cli -- proxy start --config tools/cli/proxy.yaml
```

The process stays in the foreground until `Ctrl+C`. A successful connection logs a line like:

```text
ActiveProxy mapping ready: http://<public-host>:<port> -> http://127.0.0.1:8888
```

Use the URL from that line to access the local upstream:

```bash
curl 'http://<public-host>:<port>/'
```

`9090` is the proxy service control endpoint, not the public mapping port. Use the mapping printed in the log.

### 3.4 Run in the background (optional)

```bash
cargo build --release -p boson-cli

nohup ./target/release/boson-cli proxy start \
  --config "$PWD/tools/cli/proxy.yaml" \
  > /tmp/boson-proxy.out 2>&1 &

echo $! > /tmp/boson-proxy.pid
```

Stop it gracefully with `SIGINT`:

```bash
kill -INT "$(cat /tmp/boson-proxy.pid)"
```

## Troubleshooting

| Error | Fix |
|---|---|
| `environment variable ... is not set` | Export the variable used in the YAML, or replace the placeholder. |
| `service.host is required` | Set `service.host` in `tools/cli/proxy.yaml`. |
| `Bad upstream address: tcp://...` | Set `upstream.scheme: http://`. |
| `Invalid Director request: ... invalid JSON` | Rebuild the current tree; the device request now sends `nonce` as Base64URL and includes the required `userSig`. |
| `Passphrase required` | Add `--passphrase` to `user register` / `device add`, and set `passphrase`-protected operations accordingly. |
| Connection/auth failure | Check the Director URL, Active Proxy service peer ID/host/port, and that the device is registered to the user. |
