//! CAN Log Decoder Library
//!
//! A stateless, reusable library for decoding CAN log files (BLF, MF4) with signal
//! definitions from DBC and ARXML files.
//!
//! # Architecture
//!
//! This library is intentionally minimal and focused on decoding:
//! - Parses log files and emits a stream of decoded events
//! - Supports DBC and ARXML signal definitions
//! - Handles multiplexed signals and AUTOSAR container PDUs
//! - Reconstructs CAN-TP (ISO-TP) multi-frame messages
//!
//! The library does NOT:
//! - Track signal value changes (old→new)
//! - Evaluate events or conditions
//! - Execute callbacks
//! - Generate reports
//!
//! All higher-level functionality is in the application layer (can-log-cli).
//!
//! # Example Usage
//!
//! ```no_run
//! use can_log_decoder::{Decoder, DecoderConfig};
//! use std::path::Path;
//!
//! // Create decoder and load signal definitions
//! let mut decoder = Decoder::new();
//! decoder.add_dbc(Path::new("powertrain.dbc")).unwrap();
//! decoder.add_dbc(Path::new("diagnostics.dbc")).unwrap();
//!
//! // Configure decoder
//! let config = DecoderConfig::new()
//!     .with_signal_decoding(true)
//!     .add_cantp_pair(0x7E0, 0x7E8)
//!     .with_channel_filter(vec![0, 1]);
//!
//! // Decode log file
//! let events = decoder.decode_file(Path::new("trace.blf"), config).unwrap();
//!
//! for event in events {
//!     match event {
//!         Ok(decoded) => {
//!             // Process decoded event
//!             println!("Event at {:?}", decoded.timestamp());
//!         }
//!         Err(e) => eprintln!("Decode error: {}", e),
//!     }
//! }
//! ```

// Public modules
pub mod config;
pub mod decoder;
pub mod types;

// Re-export main types for convenience
pub use config::{CanTpPair, DecoderConfig};
pub use decoder::{DatabaseStats, Decoder};
pub use types::{
    ContainedMessage, ContainerType, DecodedEvent, DecodedSignal,
    DecoderError, Result, SignalValue, Timestamp,
};

// Internal modules (not exposed in public API)
mod formats;
mod signals;
mod message_decoder;
mod cantp;
mod container;

/// Library version
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_library_basics() {
        // Smoke test: ensure we can create a decoder
        let decoder = Decoder::new();
        let stats = decoder.database_stats();
        assert_eq!(stats.num_messages, 0);
    }
}
