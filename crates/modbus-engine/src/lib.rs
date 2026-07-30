//! Connection manager, subscription engine, ring buffer, and tag codec.
//!
//! The walking skeleton ships only [`ConnectionManager`] — enough to open a
//! transport, read through it, and close it. Subscription polling, reconnect
//! supervision, the ring buffer, and the tag codec land in later increments.

use modbus_core::{
    ConnectionConfig, ConnectionId, ModbusTransport, Quantity, RegisterAddress, SlaveId,
    TransportError,
};
use modbus_driver_tokio::TokioModbusDriver;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tokio::sync::Mutex;

/// Errors raised by the engine layer.
#[derive(Debug, thiserror::Error)]
pub enum EngineError {
    #[error("connection not found: {0}")]
    NotFound(String),
    #[error("transport error: {0}")]
    Transport(#[from] TransportError),
}

/// A connection's runtime state.
struct Handle {
    name: String,
    config: ConnectionConfig,
    /// One in flight at a time: the mutex serializes requests against this
    /// transport. The full design (per-connection mpsc command actor with
    /// write prioritization) replaces this in a later increment.
    transport: Mutex<Box<dyn ModbusTransport>>,
}

/// A public snapshot of a connection.
#[derive(Debug, Clone)]
pub struct ConnectionInfo {
    pub id: ConnectionId,
    pub name: String,
    pub config: ConnectionConfig,
}

/// Registry of live Modbus connections, each a transport that may address
/// many slaves.
pub struct ConnectionManager {
    connections: Mutex<HashMap<ConnectionId, Arc<Handle>>>,
    next_id: AtomicU64,
}

impl ConnectionManager {
    pub fn new() -> Self {
        Self {
            connections: Mutex::new(HashMap::new()),
            next_id: AtomicU64::new(1),
        }
    }

    /// Open a transport from `config` and register it. Returns the assigned
    /// (or provided) id.
    pub async fn open(&self, mut config: ConnectionConfig) -> Result<ConnectionId, EngineError> {
        let id = config.id.clone().unwrap_or_else(|| {
            let n = self.next_id.fetch_add(1, Ordering::Relaxed);
            ConnectionId::new(format!("c{n}"))
        });
        config.id = Some(id.clone());

        let driver = TokioModbusDriver::connect(&config.transport).await?;
        let handle = Arc::new(Handle {
            name: config.name.clone(),
            config,
            transport: Mutex::new(Box::new(driver)),
        });

        self.connections.lock().await.insert(id.clone(), handle);
        tracing::info!(connection = %id.0, "connection opened");
        Ok(id)
    }

    /// Close and drop a registered connection.
    pub async fn close(&self, id: &ConnectionId) -> Result<(), EngineError> {
        match self.connections.lock().await.remove(id) {
            Some(_) => {
                tracing::info!(connection = %id.0, "connection closed");
                Ok(())
            }
            None => Err(EngineError::NotFound(id.0.clone())),
        }
    }

    /// List all registered connections.
    pub async fn list(&self) -> Vec<ConnectionInfo> {
        self.connections
            .lock()
            .await
            .iter()
            .map(|(id, h)| ConnectionInfo {
                id: id.clone(),
                name: h.name.clone(),
                config: h.config.clone(),
            })
            .collect()
    }

    /// Read holding registers from a connection's transport.
    pub async fn read_holding_registers(
        &self,
        id: &ConnectionId,
        slave: SlaveId,
        addr: RegisterAddress,
        qty: Quantity,
    ) -> Result<Vec<u16>, EngineError> {
        self.with_transport(id, |t| {
            Box::pin(async move { t.read_holding_registers(slave, addr, qty).await })
        })
        .await
    }

    /// Borrow a connection's transport under its lock and run `f` against it.
    /// The registry lock is released before the transport is touched, so
    /// connections stay concurrent.
    async fn with_transport<F, R>(&self, id: &ConnectionId, f: F) -> Result<R, EngineError>
    where
        F: for<'a> FnOnce(
            &'a mut Box<dyn ModbusTransport>,
        ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<R, TransportError>> + Send + 'a>>,
    {
        let handle: Arc<Handle> = {
            let guard = self.connections.lock().await;
            guard
                .get(id)
                .cloned()
                .ok_or_else(|| EngineError::NotFound(id.0.clone()))?
        };
        let mut transport = handle.transport.lock().await;
        f(&mut transport).await.map_err(EngineError::from)
    }
}

impl Default for ConnectionManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Type alias used by the server crate to share the manager across handlers.
pub type SharedConnectionManager = Arc<ConnectionManager>;
