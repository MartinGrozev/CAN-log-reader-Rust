//! Signal database and DBC/ARXML parsers
//!
//! This module contains parsers for signal definition files (DBC, ARXML)
//! and the unified signal database.

pub mod dbc;
pub mod arxml;
pub mod database;

// Re-export key types for convenience
pub use database::{
    ByteOrder, ContainerDefinition, ContainerLayout, ContainedPduInfo,
    MessageDefinition, MultiplexerInfo, SignalDatabase, SignalDefinition,
    ValueType, DatabaseStats,
};
