//! `tokio-modbus` implementation of [`modbus_core::ModbusTransport`].
//!
//! TCP is implemented fully (it is what the walking-skeleton e2e exercises).
//! RTU-over-TCP and RTU-over-serial land in the next increment — they return a
//! `TransportError::Protocol` until then, since they cannot be exercised
//! end-to-end without serial hardware.

use modbus_core::{
    ModbusException, ModbusTransport, Quantity, RegisterAddress, SlaveId, TransportConfig,
    TransportError,
};
use std::net::ToSocketAddrs;
use tokio_modbus::prelude::*;

/// A live `tokio-modbus` client behind the [`ModbusTransport`] trait.
pub struct TokioModbusDriver {
    ctx: client::Context,
}

impl TokioModbusDriver {
    /// Open a transport per `config`. Only TCP is wired in the skeleton.
    pub async fn connect(config: &TransportConfig) -> Result<Self, TransportError> {
        let ctx = match config {
            TransportConfig::Tcp { host, port } => {
                let mut addrs = (host.as_str(), *port)
                    .to_socket_addrs()
                    .map_err(TransportError::Io)?;
                let addr = addrs
                    .next()
                    .ok_or_else(|| TransportError::Protocol(format!("unresolved {host}:{port}")))?;
                tcp::connect(addr).await.map_err(TransportError::Io)?
            }
            TransportConfig::RtuOverTcp { host, port } => {
                return Err(TransportError::Protocol(format!(
                    "rtu-over-tcp ({host}:{port}) not implemented in skeleton"
                )));
            }
            TransportConfig::RtuOverSerial { path, .. } => {
                return Err(TransportError::Protocol(format!(
                    "rtu-over-serial ({path}) not implemented in skeleton"
                )));
            }
        };
        Ok(Self { ctx })
    }
}

/// Map the outer transport/protocol error layer to [`TransportError`].
fn map_err(e: tokio_modbus::Error) -> TransportError {
    match e {
        tokio_modbus::Error::Transport(io) => TransportError::Io(io),
        tokio_modbus::Error::Protocol(p) => TransportError::Protocol(p.to_string()),
    }
}

/// Map the inner Modbus exception layer to [`TransportError`].
fn map_exception(exc: tokio_modbus::ExceptionCode) -> TransportError {
    TransportError::Exception(ModbusException::from(u8::from(exc)))
}

#[async_trait::async_trait]
impl ModbusTransport for TokioModbusDriver {
    async fn read_coils(
        &mut self,
        slave: SlaveId,
        addr: RegisterAddress,
        qty: Quantity,
    ) -> Result<Vec<bool>, TransportError> {
        self.ctx.set_slave(Slave(slave.0));
        self.ctx
            .read_coils(addr.0, qty.0)
            .await
            .map_err(map_err)?
            .map_err(map_exception)
    }

    async fn read_discrete_inputs(
        &mut self,
        slave: SlaveId,
        addr: RegisterAddress,
        qty: Quantity,
    ) -> Result<Vec<bool>, TransportError> {
        self.ctx.set_slave(Slave(slave.0));
        self.ctx
            .read_discrete_inputs(addr.0, qty.0)
            .await
            .map_err(map_err)?
            .map_err(map_exception)
    }

    async fn read_input_registers(
        &mut self,
        slave: SlaveId,
        addr: RegisterAddress,
        qty: Quantity,
    ) -> Result<Vec<u16>, TransportError> {
        self.ctx.set_slave(Slave(slave.0));
        self.ctx
            .read_input_registers(addr.0, qty.0)
            .await
            .map_err(map_err)?
            .map_err(map_exception)
    }

    async fn read_holding_registers(
        &mut self,
        slave: SlaveId,
        addr: RegisterAddress,
        qty: Quantity,
    ) -> Result<Vec<u16>, TransportError> {
        self.ctx.set_slave(Slave(slave.0));
        self.ctx
            .read_holding_registers(addr.0, qty.0)
            .await
            .map_err(map_err)?
            .map_err(map_exception)
    }

    async fn write_single_coil(
        &mut self,
        slave: SlaveId,
        addr: RegisterAddress,
        value: bool,
    ) -> Result<(), TransportError> {
        self.ctx.set_slave(Slave(slave.0));
        self.ctx
            .write_single_coil(addr.0, value)
            .await
            .map_err(map_err)?
            .map_err(map_exception)
    }

    async fn write_multiple_coils(
        &mut self,
        slave: SlaveId,
        addr: RegisterAddress,
        values: &[bool],
    ) -> Result<(), TransportError> {
        self.ctx.set_slave(Slave(slave.0));
        self.ctx
            .write_multiple_coils(addr.0, values)
            .await
            .map_err(map_err)?
            .map_err(map_exception)
    }

    async fn write_single_register(
        &mut self,
        slave: SlaveId,
        addr: RegisterAddress,
        value: u16,
    ) -> Result<(), TransportError> {
        self.ctx.set_slave(Slave(slave.0));
        self.ctx
            .write_single_register(addr.0, value)
            .await
            .map_err(map_err)?
            .map_err(map_exception)
    }

    async fn write_multiple_registers(
        &mut self,
        slave: SlaveId,
        addr: RegisterAddress,
        values: &[u16],
    ) -> Result<(), TransportError> {
        self.ctx.set_slave(Slave(slave.0));
        self.ctx
            .write_multiple_registers(addr.0, values)
            .await
            .map_err(map_err)?
            .map_err(map_exception)
    }
}
