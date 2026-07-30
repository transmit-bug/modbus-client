//! Axum HTTP + WebSocket server with a JSON-RPC 2.0 dispatcher.

pub mod rpc;
pub mod ws;

pub use ws::{app, AppState};
