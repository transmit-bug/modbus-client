//! WebSocket transport for JSON-RPC 2.0 and the method dispatcher.
//!
//! One WS connection carries both request/response (id-correlated) RPC and
//! server→client push notifications. The skeleton dispatches a handful of
//! methods against the [`ConnectionManager`]; subscriptions/push land later.

use crate::{protocol::CloseRequest, protocol::ReadHoldingRegistersRequest, rpc};
use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State,
    },
    response::Response,
    routing::get,
    Router,
};
use futures_util::{SinkExt, StreamExt};
use modbus_core::{ConnectionConfig, ConnectionId};
use modbus_engine::{ConnectionInfo, EngineError, SharedConnectionManager};
use serde_json::{json, Value};

/// Shared server state handed to every handler.
#[derive(Clone)]
pub struct AppState {
    pub manager: SharedConnectionManager,
}

/// Build the HTTP/WS router for a given connection manager.
pub fn app(manager: SharedConnectionManager) -> Router {
    Router::new()
        .route("/ws", get(ws_handler))
        .with_state(AppState { manager })
}

async fn ws_handler(ws: WebSocketUpgrade, State(state): State<AppState>) -> Response {
    ws.on_upgrade(move |socket| run_socket(socket, state))
}

/// Drive one WebSocket connection: read text frames, dispatch each as a
/// JSON-RPC call, and write the response (if any). Notifications (no `id`)
/// receive no response.
async fn run_socket(socket: WebSocket, state: AppState) {
    let (mut sender, mut receiver) = socket.split();
    while let Some(msg) = receiver.next().await {
        let Ok(msg) = msg else { return };
        if let Message::Text(text) = msg {
            if let Some(resp) = dispatch(&state, text.as_str()).await {
                if sender.send(Message::Text(resp.to_string().into())).await.is_err() {
                    return;
                }
            }
        }
    }
}

type RpcResult = Result<Value, (i32, String)>;

fn invalid_params(e: impl std::fmt::Display) -> (i32, String) {
    (rpc::code::INVALID_PARAMS, e.to_string())
}

fn engine_err_to_rpc(e: EngineError) -> (i32, String) {
    match e {
        EngineError::NotFound(_) => (rpc::code::INVALID_PARAMS, e.to_string()),
        EngineError::Transport(_) => (rpc::code::INTERNAL_ERROR, e.to_string()),
    }
}

async fn handle_create(state: &AppState, params: Value) -> RpcResult {
    let cfg: ConnectionConfig = serde_json::from_value(params).map_err(invalid_params)?;
    let id: ConnectionId = state
        .manager
        .open(cfg)
        .await
        .map_err(engine_err_to_rpc)?;
    Ok(json!(id))
}

async fn handle_list(state: &AppState) -> RpcResult {
    let list: Vec<ConnectionInfo> = state.manager.list().await;
    Ok(json!(list))
}

async fn handle_close(state: &AppState, params: Value) -> RpcResult {
    let req: CloseRequest = serde_json::from_value(params).map_err(invalid_params)?;
    state
        .manager
        .close(&req.connection)
        .await
        .map_err(engine_err_to_rpc)?;
    Ok(Value::Null)
}

async fn handle_read_holding(state: &AppState, params: Value) -> RpcResult {
    let req: ReadHoldingRegistersRequest = serde_json::from_value(params).map_err(invalid_params)?;
    let values = state
        .manager
        .read_holding_registers(&req.connection, req.slave, req.address, req.quantity)
        .await
        .map_err(engine_err_to_rpc)?;
    Ok(json!(values))
}

/// Parse and dispatch a single JSON-RPC message. Returns the response value
/// to send back, or `None` for notifications (and errors on notifications).
async fn dispatch(state: &AppState, raw: &str) -> Option<Value> {
    let parsed = match rpc::parse(raw) {
        Ok(p) => p,
        Err((id, code, msg)) => return Some(rpc::error(id, code, msg, None)),
    };
    let id = match parsed.id {
        Some(id) => id,
        None => return None, // notification: no response
    };

    let result: RpcResult = match parsed.method.as_str() {
        "connection.create" => handle_create(state, parsed.params).await,
        "connection.list" => handle_list(state).await,
        "connection.close" => handle_close(state, parsed.params).await,
        "read.holdingRegisters" => handle_read_holding(state, parsed.params).await,
        _ => return Some(rpc::error(Some(id), rpc::code::METHOD_NOT_FOUND, "method not found", None)),
    };

    Some(match result {
        Ok(value) => rpc::success(Some(id), value),
        Err((code, msg)) => rpc::error(Some(id), code, msg, None),
    })
}
