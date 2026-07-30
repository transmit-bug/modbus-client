//! End-to-end tests at the agreed primary seam: the public WebSocket JSON-RPC
//! contract, driven against a real Axum server (in-process) and real
//! transports.
//!
//! - Stage 1: connection lifecycle (create / list / close) over WS, with a
//!   dummy TCP acceptor so `open()` succeeds without a Modbus slave.
//! - Stage 2: `read.holdingRegisters` flows through the driver against a real
//!   `tokio-modbus` mock slave.

use futures_util::{SinkExt, StreamExt};
use modbus_engine::{ConnectionManager, SharedConnectionManager};
use modbus_server::app;
use serde_json::{json, Value};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::{TcpListener, TcpStream};
use tokio_modbus::prelude::*;
use tokio_modbus::server::tcp::{accept_tcp_connection, Server};
use tokio_modbus::server::Service;
use tokio_tungstenite::tungstenite::Message;

type Client = tokio_tungstenite::WebSocketStream<
    tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
>;

/// Start the Axum server on an ephemeral port; return its WS url.
async fn start_server() -> (SharedConnectionManager, String) {
    let manager: SharedConnectionManager = Arc::new(ConnectionManager::new());
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let m = manager.clone();
    tokio::spawn(async move {
        let _ = axum::serve(listener, app(m)).await;
    });
    (manager, format!("ws://{addr}/ws"))
}

/// A TCP acceptor that holds connections open but speaks no protocol — enough
/// for `open()` to succeed in lifecycle tests.
async fn dummy_tcp_acceptor() -> u16 {
    let l = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = l.local_addr().unwrap().port();
    tokio::spawn(async move {
        loop {
            if l.accept().await.is_err() {
                break;
            }
        }
    });
    port
}

/// Send a JSON-RPC request and await the next response.
async fn call(ws: &mut Client, id: u64, method: &str, params: Value) -> Value {
    let req = json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params });
    ws.send(Message::Text(req.to_string().into())).await.unwrap();
    loop {
        match ws.next().await.unwrap().unwrap() {
            Message::Text(t) => return serde_json::from_str(t.as_str()).unwrap(),
            _ => continue,
        }
    }
}

// --- Mock Modbus TCP slave ------------------------------------------------

#[derive(Clone)]
struct MockSlave {
    holding: Arc<Vec<u16>>,
}

impl Service for MockSlave {
    type Request = Request<'static>;
    type Response = Response;
    type Exception = ExceptionCode;
    type Future = std::future::Ready<Result<Response, ExceptionCode>>;

    fn call(&self, req: Request<'static>) -> Self::Future {
        let resp = match req {
            Request::ReadHoldingRegisters(addr, qty) => {
                let start = addr as usize;
                let qty = qty as usize;
                let values: Vec<u16> = (start..start + qty)
                    .map(|i| self.holding.get(i).copied().unwrap_or(0))
                    .collect();
                Ok(Response::ReadHoldingRegisters(values))
            }
            _ => Err(ExceptionCode::IllegalFunction),
        };
        std::future::ready(resp)
    }
}

/// Start a `tokio-modbus` TCP mock slave with the given holding registers;
/// returns the bound port.
async fn start_mock_slave(holding: Vec<u16>) -> u16 {
    let holding = Arc::new(holding);
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let server = Server::new(listener);
    tokio::spawn(async move {
        let on_connected = move |stream: TcpStream, _addr: SocketAddr| {
            let holding = holding.clone();
            async move {
                accept_tcp_connection(stream, _addr, move |_addr| {
                    Ok(Some(MockSlave {
                        holding: holding.clone(),
                    }))
                })
            }
        };
        server
            .serve(&on_connected, |_e: std::io::Error| {})
            .await
            .ok();
    });
    port
}

// --- Stage 1: lifecycle ---------------------------------------------------

#[tokio::test]
async fn connection_lifecycle_over_websocket() {
    let (_manager, url) = start_server().await;
    let port = dummy_tcp_acceptor().await;
    let (mut ws, _resp) = tokio_tungstenite::connect_async(&url).await.unwrap();

    let created = call(
        &mut ws,
        1,
        "connection.create",
        json!({ "name": "demo", "transport": { "type": "tcp", "host": "127.0.0.1", "port": port } }),
    )
    .await;
    assert_eq!(created["jsonrpc"], "2.0");
    assert_eq!(created["id"], 1);
    let id = created["result"].as_str().unwrap().to_string();
    assert!(id.starts_with('c'), "assigned id should look like c1, got {id}");

    let listed = call(&mut ws, 2, "connection.list", json!({})).await;
    let arr = listed["result"].as_array().unwrap();
    assert_eq!(arr.len(), 1);
    assert_eq!(arr[0]["name"], "demo");

    let closed = call(&mut ws, 3, "connection.close", json!({ "connection": id })).await;
    assert_eq!(closed["result"], Value::Null);

    let listed = call(&mut ws, 4, "connection.list", json!({})).await;
    assert_eq!(listed["result"].as_array().unwrap().len(), 0);
}

#[tokio::test]
async fn unknown_method_returns_method_not_found() {
    let (_manager, url) = start_server().await;
    let (mut ws, _resp) = tokio_tungstenite::connect_async(&url).await.unwrap();

    let resp = call(&mut ws, 1, "bogus.method", json!({})).await;
    assert_eq!(resp["id"], 1);
    assert_eq!(resp["error"]["code"], -32601);
}

// --- Stage 2: read through the driver against a mock slave -----------------

#[tokio::test]
async fn read_holding_registers_against_mock_slave() {
    let (_manager, url) = start_server().await;
    let slave_port = start_mock_slave(vec![0x1111, 0x2222, 0x3333, 0x4444]).await;
    let (mut ws, _resp) = tokio_tungstenite::connect_async(&url).await.unwrap();

    let created = call(
        &mut ws,
        1,
        "connection.create",
        json!({ "name": "slave", "transport": { "type": "tcp", "host": "127.0.0.1", "port": slave_port } }),
    )
    .await;
    let conn = created["result"].as_str().unwrap().to_string();

    let read =
        call(&mut ws, 2, "read.holdingRegisters", json!({ "connection": conn, "slave": 1, "address": 0, "quantity": 4 }))
            .await;
    assert_eq!(read["jsonrpc"], "2.0");
    assert_eq!(read["id"], 2);
    assert_eq!(read["result"], json!([0x1111, 0x2222, 0x3333, 0x4444]));
}

#[tokio::test]
async fn read_respects_slave_id_per_request() {
    // Same transport (one mock slave) addressed with different slave ids — the
    // connection-is-a-transport model lets slave id vary per request.
    let (_manager, url) = start_server().await;
    let slave_port = start_mock_slave(vec![10, 20, 30]).await;
    let (mut ws, _resp) = tokio_tungstenite::connect_async(&url).await.unwrap();

    let created = call(
        &mut ws,
        1,
        "connection.create",
        json!({ "name": "bus", "transport": { "type": "tcp", "host": "127.0.0.1", "port": slave_port } }),
    )
    .await;
    let conn = created["result"].as_str().unwrap().to_string();

    let a = call(&mut ws, 2, "read.holdingRegisters", json!({ "connection": &conn, "slave": 1, "address": 0, "quantity": 3 })).await;
    let b = call(&mut ws, 3, "read.holdingRegisters", json!({ "connection": &conn, "slave": 247, "address": 0, "quantity": 3 })).await;
    assert_eq!(a["result"], json!([10, 20, 30]));
    assert_eq!(b["result"], json!([10, 20, 30]));
}
