//! Unified signal database
//!
//! Combines signal definitions from multiple DBC and ARXML files into a
//! single queryable database.

use std::collections::HashMap;

/// A complete CAN message definition
#[derive(Debug, Clone)]
pub struct MessageDefinition {
    /// CAN message ID
    pub id: u32,
    /// Message name
    pub name: String,
    /// Message size in bytes
    pub size: usize,
    /// Sender ECU name (optional)
    pub sender: Option<String>,
    /// All signals in this message
    pub signals: Vec<SignalDefinition>,
    /// True if this message has multiplexed signals
    pub is_multiplexed: bool,
    /// Multiplexer signal name (if multiplexed)
    pub multiplexer_signal: Option<String>,
    /// Source file (DBC/ARXML filename)
    pub source: String,
}

/// A CAN signal definition
#[derive(Debug, Clone)]
pub struct SignalDefinition {
    /// Signal name
    pub name: String,
    /// Start bit in the CAN frame
    pub start_bit: u16,
    /// Length in bits
    pub length: u16,
    /// Byte order (true = big-endian, false = little-endian)
    pub byte_order: ByteOrder,
    /// Value type (signed/unsigned)
    pub value_type: ValueType,
    /// Scale factor to convert raw value to physical value
    pub factor: f64,
    /// Offset to add after scaling
    pub offset: f64,
    /// Minimum physical value
    pub min: f64,
    /// Maximum physical value
    pub max: f64,
    /// Engineering unit (e.g., "km/h", "°C", "V")
    pub unit: Option<String>,
    /// Value table for enum-like values (raw_value -> description)
    pub value_table: Option<HashMap<i64, String>>,
    /// Multiplexer info (None if not multiplexed)
    pub multiplexer_info: Option<MultiplexerInfo>,
}

/// Byte order for signal extraction
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ByteOrder {
    /// Little-endian (Intel format)
    LittleEndian,
    /// Big-endian (Motorola format)
    BigEndian,
}

/// Value type for signal interpretation
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValueType {
    /// Signed integer
    Signed,
    /// Unsigned integer
    Unsigned,
}

/// Multiplexer information for multiplexed signals
#[derive(Debug, Clone)]
pub struct MultiplexerInfo {
    /// Name of the multiplexer signal that controls this signal
    pub multiplexer_signal: String,
    /// Multiplexer value(s) for which this signal is active
    pub multiplexer_values: Vec<u64>,
}

/// AUTOSAR Container PDU definition
#[derive(Debug, Clone)]
pub struct ContainerDefinition {
    /// Container CAN ID
    pub id: u32,
    /// Container name
    pub name: String,
    /// Container type (Static/Dynamic/Queued)
    pub container_type: crate::types::ContainerType,
    /// Layout information (PDU positions, headers, etc.)
    pub layout: ContainerLayout,
    /// Source ARXML file
    pub source: String,
}

/// Container layout information
#[derive(Debug, Clone)]
pub enum ContainerLayout {
    /// Fixed layout - PDUs always at same positions
    Static {
        pdus: Vec<ContainedPduInfo>,
    },
    /// Variable layout with header
    Dynamic {
        header_size: usize,
        pdus: Vec<ContainedPduInfo>,
    },
    /// Queued instances of same PDU
    Queued {
        pdu_id: u32,
        pdu_size: usize,
    },
}

/// Information about a PDU contained within a container
#[derive(Debug, Clone)]
pub struct ContainedPduInfo {
    /// PDU identifier
    pub pdu_id: u32,
    /// PDU name
    pub name: String,
    /// Position in container (byte offset)
    pub position: usize,
    /// PDU size in bytes
    pub size: usize,
}

/// The unified signal database
pub struct SignalDatabase {
    /// All message definitions by CAN ID
    /// Key: CAN ID, Value: List of messages with that ID (can be multiple from different DBCs)
    messages: HashMap<u32, Vec<MessageDefinition>>,

    /// Container PDU definitions by container ID
    containers: HashMap<u32, ContainerDefinition>,

    /// Signal name lookup for quick access
    /// Key: Signal name, Value: List of (CAN ID, signal index) tuples
    signal_lookup: HashMap<String, Vec<(u32, usize)>>,

    /// Message name lookup for contained PDUs
    /// Key: Message name, Value: (CAN ID, message index in messages vector)
    message_lookup: HashMap<String, (u32, usize)>,
}

impl SignalDatabase {
    /// Create a new empty signal database
    pub fn new() -> Self {
        Self {
            messages: HashMap::new(),
            containers: HashMap::new(),
            signal_lookup: HashMap::new(),
            message_lookup: HashMap::new(),
        }
    }

    /// Add a message definition to the database
    pub fn add_message(&mut self, message: MessageDefinition) {
        let can_id = message.id;

        // Build signal lookup indices
        for (sig_idx, signal) in message.signals.iter().enumerate() {
            self.signal_lookup
                .entry(signal.name.clone())
                .or_insert_with(Vec::new)
                .push((can_id, sig_idx));
        }

        // Get the index where this message will be added
        let msg_idx = self.messages
            .get(&can_id)
            .map(|v| v.len())
            .unwrap_or(0);

        // Add message name lookup (for contained PDU decoding)
        self.message_lookup.insert(message.name.clone(), (can_id, msg_idx));

        // Add message to database
        self.messages
            .entry(can_id)
            .or_insert_with(Vec::new)
            .push(message);
    }

    /// Add a container definition to the database
    pub fn add_container(&mut self, container: ContainerDefinition) {
        self.containers.insert(container.id, container);
    }

    /// Get all message definitions for a given CAN ID
    pub fn get_messages(&self, can_id: u32) -> Option<&Vec<MessageDefinition>> {
        self.messages.get(&can_id)
    }

    /// Get a specific message definition (first one found for given CAN ID)
    pub fn get_message(&self, can_id: u32) -> Option<&MessageDefinition> {
        self.messages.get(&can_id).and_then(|msgs| msgs.first())
    }

    /// Get container definition by ID
    pub fn get_container(&self, container_id: u32) -> Option<&ContainerDefinition> {
        self.containers.get(&container_id)
    }

    /// Get message definition by name (for contained PDU decoding)
    pub fn get_message_by_name(&self, message_name: &str) -> Option<&MessageDefinition> {
        self.message_lookup
            .get(message_name)
            .and_then(|(can_id, msg_idx)| {
                self.messages.get(can_id).and_then(|msgs| msgs.get(*msg_idx))
            })
    }

    /// Find all messages containing a specific signal name
    pub fn find_signal(&self, signal_name: &str) -> Vec<(u32, &SignalDefinition)> {
        self.signal_lookup
            .get(signal_name)
            .map(|locations| {
                locations
                    .iter()
                    .filter_map(|(can_id, sig_idx)| {
                        self.get_message(*can_id)
                            .and_then(|msg| msg.signals.get(*sig_idx))
                            .map(|sig| (*can_id, sig))
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Get database statistics
    pub fn stats(&self) -> DatabaseStats {
        let num_messages: usize = self.messages.values().map(|v| v.len()).sum();
        let num_signals: usize = self.messages.values()
            .flat_map(|msgs| msgs.iter())
            .map(|msg| msg.signals.len())
            .sum();
        let num_containers = self.containers.len();

        DatabaseStats {
            num_messages,
            num_signals,
            num_containers,
        }
    }

    /// Get all unique CAN IDs in the database
    pub fn get_all_can_ids(&self) -> Vec<u32> {
        let mut ids: Vec<u32> = self.messages.keys().copied().collect();
        ids.sort_unstable();
        ids
    }
}

/// Database statistics
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DatabaseStats {
    /// Total number of message definitions
    pub num_messages: usize,
    /// Total number of signal definitions
    pub num_signals: usize,
    /// Total number of container PDUs
    pub num_containers: usize,
}

impl Default for SignalDatabase {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_database() {
        let db = SignalDatabase::new();
        let stats = db.stats();
        assert_eq!(stats.num_messages, 0);
        assert_eq!(stats.num_signals, 0);
        assert_eq!(stats.num_containers, 0);
    }

    #[test]
    fn test_add_message() {
        let mut db = SignalDatabase::new();

        let signal = SignalDefinition {
            name: "EngineSpeed".to_string(),
            start_bit: 0,
            length: 16,
            byte_order: ByteOrder::LittleEndian,
            value_type: ValueType::Unsigned,
            factor: 1.0,
            offset: 0.0,
            min: 0.0,
            max: 8000.0,
            unit: Some("rpm".to_string()),
            value_table: None,
            multiplexer_info: None,
        };

        let message = MessageDefinition {
            id: 0x123,
            name: "EngineData".to_string(),
            size: 8,
            sender: Some("ECU1".to_string()),
            signals: vec![signal],
            is_multiplexed: false,
            multiplexer_signal: None,
            source: "test.dbc".to_string(),
        };

        db.add_message(message);

        let stats = db.stats();
        assert_eq!(stats.num_messages, 1);
        assert_eq!(stats.num_signals, 1);

        // Test retrieval
        let msg = db.get_message(0x123).unwrap();
        assert_eq!(msg.name, "EngineData");
        assert_eq!(msg.signals[0].name, "EngineSpeed");

        // Test signal lookup
        let found = db.find_signal("EngineSpeed");
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].0, 0x123);
    }
}
