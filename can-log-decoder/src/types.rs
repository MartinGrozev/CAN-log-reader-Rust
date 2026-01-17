//! Core types for the CAN log decoder library
//!
//! This module defines all the fundamental types that the decoder emits when processing
//! log files. The decoder is stateless and only outputs decoded events - it does not
//! track signal changes or evaluate conditions.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fmt;

/// Timestamp type used throughout the decoder
pub type Timestamp = DateTime<Utc>;

/// Result type for decoder operations
pub type Result<T> = std::result::Result<T, DecoderError>;

/// Raw CAN frame from a log file (BLF, MF4, etc.)
///
/// This represents a single CAN frame as read from the log file,
/// before any signal decoding or message interpretation.
#[derive(Debug, Clone, PartialEq)]
pub struct CanFrame {
    /// Timestamp in nanoseconds since epoch
    pub timestamp_ns: u64,
    /// CAN channel number (e.g., 0, 1, 2...)
    pub channel: u8,
    /// CAN message ID (11-bit or 29-bit)
    pub can_id: u32,
    /// Frame data bytes (0-8 bytes for classic CAN, up to 64 for CAN-FD)
    pub data: Vec<u8>,
    /// True if this is an extended (29-bit) CAN ID
    pub is_extended: bool,
    /// True if this is a CAN-FD frame
    pub is_fd: bool,
    /// True if this is an error frame
    pub is_error_frame: bool,
    /// True if this is a remote frame
    pub is_remote_frame: bool,
}

impl CanFrame {
    /// Convert timestamp from nanoseconds to DateTime<Utc>
    pub fn timestamp(&self) -> Timestamp {
        let secs = (self.timestamp_ns / 1_000_000_000) as i64;
        let nsecs = (self.timestamp_ns % 1_000_000_000) as u32;
        DateTime::from_timestamp(secs, nsecs).unwrap_or_else(|| Utc::now())
    }

    /// Get the data length code (DLC) - number of data bytes
    pub fn dlc(&self) -> usize {
        self.data.len()
    }
}

/// Errors that can occur during decoding
#[derive(Debug, thiserror::Error)]
pub enum DecoderError {
    #[error("Failed to parse log file: {0}")]
    LogParseError(String),

    #[error("Failed to parse DBC file: {0}")]
    DbcParseError(String),

    #[error("Failed to parse ARXML file: {0}")]
    ArxmlParseError(String),

    #[error("Signal not found: {0}")]
    SignalNotFound(String),

    #[error("Message not found: CAN ID 0x{0:X}")]
    MessageNotFound(u32),

    #[error("Invalid signal definition: {0}")]
    InvalidSignalDefinition(String),

    #[error("Invalid data: {0}")]
    InvalidData(String),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Unknown error: {0}")]
    Unknown(String),
}

/// Main decoded event type - the primary output of the decoder
#[derive(Debug, Clone, PartialEq)]
pub enum DecodedEvent {
    /// A decoded CAN message with all its signals
    Message {
        /// Absolute timestamp from the log file
        timestamp: Timestamp,
        /// CAN channel number (e.g., 0, 1, 2...)
        channel: u8,
        /// CAN message ID
        can_id: u32,
        /// Message name from DBC/ARXML (if available)
        message_name: Option<String>,
        /// Sender ECU name from DBC/ARXML (if available)
        sender: Option<String>,
        /// All decoded signals in this message
        signals: Vec<DecodedSignal>,
        /// True if this message contains multiplexed signals
        is_multiplexed: bool,
        /// Active multiplexer value (if message is multiplexed)
        multiplexer_value: Option<u64>,
    },

    /// A reconstructed CAN-TP (ISO-TP) message with complete payload
    CanTpMessage {
        /// Timestamp of the first frame in the sequence
        timestamp: Timestamp,
        /// CAN channel number
        channel: u8,
        /// Source address (request ID)
        source_addr: u32,
        /// Target address (response ID)
        target_addr: u32,
        /// Complete reassembled payload bytes
        payload: Vec<u8>,
        /// Total length of the payload
        payload_length: usize,
    },

    /// An AUTOSAR container PDU with raw contained PDUs (before signal decoding)
    ContainerPdu {
        /// Absolute timestamp from the log file
        timestamp: Timestamp,
        /// Container PDU CAN ID
        container_id: u32,
        /// Container name from ARXML
        container_name: String,
        /// Type of container (Static/Dynamic/Queued)
        container_type: ContainerType,
        /// Raw contained PDUs (before signal decoding)
        contained_pdus: Vec<ContainedPdu>,
    },

    /// A raw CAN frame (optionally emitted if requested in config)
    RawFrame {
        /// Absolute timestamp from the log file
        timestamp: Timestamp,
        /// CAN channel number
        channel: u8,
        /// CAN message ID
        can_id: u32,
        /// Raw data bytes
        data: Vec<u8>,
        /// True if this is a CAN-FD frame
        is_fd: bool,
    },
}

/// AUTOSAR container PDU types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ContainerType {
    /// Fixed layout container - always contains same PDUs at same positions
    Static,
    /// Variable layout container - header bytes indicate which PDUs are present
    Dynamic,
    /// Multiple instances of the same PDU type queued together
    Queued,
}

impl fmt::Display for ContainerType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ContainerType::Static => write!(f, "Static"),
            ContainerType::Dynamic => write!(f, "Dynamic"),
            ContainerType::Queued => write!(f, "Queued"),
        }
    }
}

/// A PDU contained within an AUTOSAR container (raw data before signal decoding)
#[derive(Debug, Clone, PartialEq)]
pub struct ContainedPdu {
    /// PDU identifier
    pub pdu_id: u32,
    /// PDU name from ARXML (if available)
    pub name: String,
    /// Raw PDU data bytes
    pub data: Vec<u8>,
}

/// A message contained within an AUTOSAR container PDU (after signal decoding)
#[derive(Debug, Clone, PartialEq)]
pub struct ContainedMessage {
    /// PDU identifier
    pub pdu_id: u32,
    /// PDU name from ARXML (if available)
    pub pdu_name: Option<String>,
    /// All decoded signals in this contained PDU
    pub signals: Vec<DecodedSignal>,
    /// True if this PDU contains multiplexed signals
    pub is_multiplexed: bool,
    /// Active multiplexer value (if PDU is multiplexed)
    pub multiplexer_value: Option<u64>,
}

/// A decoded signal with its current value
#[derive(Debug, Clone, PartialEq)]
pub struct DecodedSignal {
    /// Signal name from DBC/ARXML
    pub name: String,
    /// Current decoded value (no history tracked by decoder)
    pub value: SignalValue,
    /// Engineering unit (e.g., "km/h", "°C", "V")
    pub unit: Option<String>,
    /// Value description from value tables (e.g., "0=Off, 1=On")
    pub value_description: Option<String>,
    /// Raw value before scaling (useful for debugging)
    pub raw_value: i64,
}

/// Signal value types supported by the decoder
#[derive(Debug, Clone, PartialEq)]
pub enum SignalValue {
    /// Signed integer value
    Integer(i64),
    /// Floating-point value (after scaling/offset)
    Float(f64),
    /// Boolean value (0/1 or from value table)
    Boolean(bool),
}

impl fmt::Display for SignalValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SignalValue::Integer(v) => write!(f, "{}", v),
            SignalValue::Float(v) => write!(f, "{:.3}", v),
            SignalValue::Boolean(v) => write!(f, "{}", if *v { "true" } else { "false" }),
        }
    }
}

impl SignalValue {
    /// Convert signal value to f64 for expression evaluation
    pub fn as_f64(&self) -> f64 {
        match self {
            SignalValue::Integer(v) => *v as f64,
            SignalValue::Float(v) => *v,
            SignalValue::Boolean(v) => if *v { 1.0 } else { 0.0 },
        }
    }

    /// Convert signal value to i64 if possible
    pub fn as_i64(&self) -> Option<i64> {
        match self {
            SignalValue::Integer(v) => Some(*v),
            SignalValue::Float(v) => Some(*v as i64),
            SignalValue::Boolean(v) => Some(if *v { 1 } else { 0 }),
        }
    }

    /// Check if this is a boolean value
    pub fn as_bool(&self) -> bool {
        match self {
            SignalValue::Boolean(v) => *v,
            SignalValue::Integer(v) => *v != 0,
            SignalValue::Float(v) => *v != 0.0,
        }
    }
}

impl DecodedEvent {
    /// Get the timestamp of this event
    pub fn timestamp(&self) -> Timestamp {
        match self {
            DecodedEvent::Message { timestamp, .. } => *timestamp,
            DecodedEvent::CanTpMessage { timestamp, .. } => *timestamp,
            DecodedEvent::ContainerPdu { timestamp, .. } => *timestamp,
            DecodedEvent::RawFrame { timestamp, .. } => *timestamp,
        }
    }

    /// Get the CAN channel of this event (if applicable)
    pub fn channel(&self) -> Option<u8> {
        match self {
            DecodedEvent::Message { channel, .. } => Some(*channel),
            DecodedEvent::CanTpMessage { channel, .. } => Some(*channel),
            DecodedEvent::ContainerPdu { .. } => None, // Channel not stored in container
            DecodedEvent::RawFrame { channel, .. } => Some(*channel),
        }
    }

    /// Get the CAN ID of this event (if applicable)
    pub fn can_id(&self) -> Option<u32> {
        match self {
            DecodedEvent::Message { can_id, .. } => Some(*can_id),
            DecodedEvent::ContainerPdu { container_id, .. } => Some(*container_id),
            DecodedEvent::RawFrame { can_id, .. } => Some(*can_id),
            DecodedEvent::CanTpMessage { .. } => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_signal_value_conversions() {
        let int_val = SignalValue::Integer(42);
        assert_eq!(int_val.as_f64(), 42.0);
        assert_eq!(int_val.as_i64(), Some(42));
        assert!(int_val.as_bool());

        let float_val = SignalValue::Float(3.14);
        assert_eq!(float_val.as_f64(), 3.14);
        assert_eq!(float_val.as_i64(), Some(3));

        let bool_val = SignalValue::Boolean(true);
        assert_eq!(bool_val.as_f64(), 1.0);
        assert!(bool_val.as_bool());
    }

    #[test]
    fn test_signal_value_display() {
        assert_eq!(format!("{}", SignalValue::Integer(42)), "42");
        assert_eq!(format!("{}", SignalValue::Float(3.14159)), "3.142");
        assert_eq!(format!("{}", SignalValue::Boolean(true)), "true");
    }
}
