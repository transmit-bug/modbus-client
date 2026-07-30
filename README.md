# modbus-client

A single-user, local Modbus client: a Rust backend (Axum + tokio) that embeds a
React frontend and serves both the UI and a JSON-RPC-over-WebSocket API from
one binary on `localhost`.

**Status:** walking skeleton. Every layer is wired and the full read path is
proven end-to-end (create a connection → read holding registers → values
back) against a mock Modbus slave. Subscriptions, the tag codec, the ring
buffer, RTU transports, and reconnection land in later increments — see
[PRD #1](https://github.com/transmit-bug/modbus-client/issues/1).

## Architecture

A cargo workspace of focused crates; each layer depends only on the ones
below it.

```
crates/
├── modbus-core          domain types + async ModbusTransport trait (no driver/web deps)
├── modbus-driver-tokio  tokio-modbus impl of the trait (TCP wired; RTU next)
├── modbus-engine        ConnectionManager (registry, one-in-flight-per-transport)
├── modbus-server        Axum HTTP + WebSocket + JSON-RPC 2.0 dispatcher
└── modbus-app           binary: wires layers, embeds the frontend, serves localhost
frontend/                React + Vite + TypeScript
```

Data flow for a read:

```
browser ──WS(JSON-RPC)──▶ server ──▶ engine ──▶ driver(tokio-modbus) ──▶ device
                                          │
                            one request in flight per transport (Mutex)
```

A connection is a **transport** (one TCP socket / serial port); the slave id is
chosen per request, so one gateway socket or RS-485 bus serves many devices.

The Rust↔TypeScript RPC contract is generated from Rust `serde` structs by
[`ts-rs`](https://github.com/Aleph-Alpha/ts-rs) into `frontend/src/types/`, so
the frontend cannot drift from the backend.

## Build & run

Prerequisites: Rust (stable), Node 18+.

```bash
# 1. generate the TypeScript types from Rust (runs on cargo test)
cargo test --workspace

# 2. build the frontend
cd frontend && npm install && npm run build && cd ..

# 3. run the binary — serves UI + WS on http://127.0.0.1:8080
cargo run -p modbus-app
```

### Dev mode (frontend HMR)

Two terminals:

```bash
# backend
cargo run -p modbus-app
# frontend (Vite on :5173, proxies /ws to :8080)
cd frontend && npm run dev
```

Open http://localhost:5173.

## Testing

```bash
cargo test --workspace
```

The primary seam is end-to-end: the public WebSocket JSON-RPC contract driven
against a real in-process Axum server and a `tokio-modbus` mock slave
(`crates/modbus-server/tests/e2e.rs`). Narrow unit seams cover the JSON-RPC
framing/error mapper (`rpc.rs`) and will cover the tag codec once it lands.

## License

MIT OR Apache-2.0.
