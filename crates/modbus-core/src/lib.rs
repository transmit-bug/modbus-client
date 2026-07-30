//! Domain types and traits for the Modbus client.
//!
//! This crate has zero dependencies on any Modbus driver or web framework.
//! It defines the vocabulary the other crates share: identifiers, register
//! addressing, connection/transport configuration, and the `ModbusTransport`
//! trait that a driver implements.
