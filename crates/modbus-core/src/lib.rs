//! Domain types and traits for the Modbus client.
//!
//! Zero dependencies on any Modbus driver or web framework. Defines the
//! vocabulary the other crates share: identifiers, register addressing,
//! connection/transport configuration, error mapping, and the
//! [`ModbusTransport`] trait a driver implements.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Identifiers & addressing
// ---------------------------------------------------------------------------

/// Opaque identifier for a managed connection (which represents one
/// transport — a TCP socket or a serial port — that may address many slaves).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ConnectionId(pub String);

impl ConnectionId {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }
}

/// A Modbus slave (unit) identifier. `0` is broadcast for writes.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize,
)]
pub struct SlaveId(pub u8);

/// A 0-based Modbus register/coil address.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize,
)]
pub struct RegisterAddress(pub u16);

/// Number of registers/coils to read or write.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize,
)]
pub struct Quantity(pub u16);

// ---------------------------------------------------------------------------
// Transport & connection configuration
// ---------------------------------------------------------------------------

/// Serial parity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Parity {
    None,
    Even,
    Odd,
}

/// How a connection reaches its device(s). A connection is one transport;
/// the slave id is chosen per request, so one transport can address many
/// devices (TCP gateway or RS-485 bus).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TransportConfig {
    /// Modbus TCP.
    Tcp { host: String, port: u16 },
    /// Modbus RTU framing carried over a TCP socket (serial gateway).
    RtuOverTcp { host: String, port: u16 },
    /// Modbus RTU over a serial line (RS-232 / RS-485).
    RtuOverSerial {
        path: String,
        baud_rate: u32,
        data_bits: u8,
        parity: Parity,
        stop_bits: u8,
    },
}

/// User-facing definition of a managed connection.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConnectionConfig {
    /// `None` => the server assigns an id on create.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<ConnectionId>,
    pub name: String,
    pub transport: TransportConfig,
}

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

/// A Modbus protocol exception returned by a slave.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModbusException {
    IllegalFunction,
    IllegalDataAddress,
    IllegalDataValue,
    SlaveDeviceFailure,
    Acknowledge,
    SlaveDeviceBusy,
    MemoryParityError,
    GatewayPathUnavailable,
    GatewayTargetNoResponse,
    Other(u8),
}

impl ModbusException {
    pub fn code(self) -> u8 {
        match self {
            Self::IllegalFunction => 1,
            Self::IllegalDataAddress => 2,
            Self::IllegalDataValue => 3,
            Self::SlaveDeviceFailure => 4,
            Self::Acknowledge => 5,
            Self::SlaveDeviceBusy => 6,
            Self::MemoryParityError => 8,
            Self::GatewayPathUnavailable => 10,
            Self::GatewayTargetNoResponse => 11,
            Self::Other(c) => c,
        }
    }
}

impl From<u8> for ModbusException {
    fn from(code: u8) -> Self {
        match code {
            1 => Self::IllegalFunction,
            2 => Self::IllegalDataAddress,
            3 => Self::IllegalDataValue,
            4 => Self::SlaveDeviceFailure,
            5 => Self::Acknowledge,
            6 => Self::SlaveDeviceBusy,
            8 => Self::MemoryParityError,
            10 => Self::GatewayPathUnavailable,
            11 => Self::GatewayTargetNoResponse,
            other => Self::Other(other),
        }
    }
}

/// Errors a transport can surface.
#[derive(Debug, thiserror::Error)]
pub enum TransportError {
    #[error("i/o error: {0}")]
    Io(#[from] std::io::Error),
    #[error("modbus exception: {0:?}")]
    Exception(ModbusException),
    #[error("modbus protocol error: {0}")]
    Protocol(String),
    #[error("connection closed")]
    Closed,
}

// ---------------------------------------------------------------------------
// The transport trait a driver implements
// ---------------------------------------------------------------------------

/// Abstract Modbus transport. A driver (e.g. `tokio-modbus`) implements this;
/// the engine talks to it through the trait so the driver is swappable and
/// testable in isolation.
#[async_trait]
pub trait ModbusTransport: Send {
    /// Read coils (function code 0x01).
    async fn read_coils(
        &mut self,
        slave: SlaveId,
        addr: RegisterAddress,
        qty: Quantity,
    ) -> Result<Vec<bool>, TransportError>;

    /// Read discrete inputs (function code 0x02).
    async fn read_discrete_inputs(
        &mut self,
        slave: SlaveId,
        addr: RegisterAddress,
        qty: Quantity,
    ) -> Result<Vec<bool>, TransportError>;

    /// Read input registers (function code 0x04).
    async fn read_input_registers(
        &mut self,
        slave: SlaveId,
        addr: RegisterAddress,
        qty: Quantity,
    ) -> Result<Vec<u16>, TransportError>;

    /// Read holding registers (function code 0x03).
    async fn read_holding_registers(
        &mut self,
        slave: SlaveId,
        addr: RegisterAddress,
        qty: Quantity,
    ) -> Result<Vec<u16>, TransportError>;

    /// Write a single coil (function code 0x05).
    async fn write_single_coil(
        &mut self,
        slave: SlaveId,
        addr: RegisterAddress,
        value: bool,
    ) -> Result<(), TransportError>;

    /// Write multiple coils (function code 0x0F).
    async fn write_multiple_coils(
        &mut self,
        slave: SlaveId,
        addr: RegisterAddress,
        values: &[bool],
    ) -> Result<(), TransportError>;

    /// Write a single register (function code 0x06).
    async fn write_single_register(
        &mut self,
        slave: SlaveId,
        addr: RegisterAddress,
        value: u16,
    ) -> Result<(), TransportError>;

    /// Write multiple registers (function code 0x10).
    async fn write_multiple_registers(
        &mut self,
        slave: SlaveId,
        addr: RegisterAddress,
        values: &[u16],
    ) -> Result<(), TransportError>;
}
