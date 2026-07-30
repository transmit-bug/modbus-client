//! Typed JSON-RPC request payloads, shared with the frontend via `ts-rs`.
//!
//! `connection.create` takes a [`modbus_core::ConnectionConfig`] directly and
//! returns a [`modbus_core::ConnectionId`]; those live in `modbus-core` and are
//! exported from there. The payloads here are the ones that need their own
//! request shape.

use modbus_core::{ConnectionId, Quantity, RegisterAddress, SlaveId};
use serde::Deserialize;

#[derive(Debug, Deserialize, ts_rs::TS)]
#[ts(export)]
pub struct CloseRequest {
    pub connection: ConnectionId,
}

#[derive(Debug, Deserialize, ts_rs::TS)]
#[ts(export)]
pub struct ReadHoldingRegistersRequest {
    pub connection: ConnectionId,
    pub slave: SlaveId,
    pub address: RegisterAddress,
    pub quantity: Quantity,
}
