//! Decoder configuration types
//!
//! This module defines the minimal configuration needed by the decoder library.
//! The decoder is intentionally simple - complex business logic (events, callbacks, etc.)
//! is handled by the application layer.

use serde::{Deserialize, Serialize};

/// Configuration for the decoder library
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DecoderConfig {
    /// Whether to decode signals (false = only emit raw frames)
    #[serde(default = "default_true")]
    pub decode_signals: bool,

    /// CAN-TP address pairs to reconstruct (source, target)
    #[serde(default)]
    pub cantp_pairs: Vec<CanTpPair>,

    /// Container PDU IDs to unpack and decode
    #[serde(default)]
    pub container_ids: Vec<u32>,

    /// Optional: only decode messages from these CAN channels
    #[serde(default)]
    pub channel_filter: Option<Vec<u8>>,

    /// Optional: only decode these specific CAN message IDs
    #[serde(default)]
    pub message_filter: Option<Vec<u32>>,

    /// Whether to emit raw frames in addition to decoded messages
    #[serde(default)]
    pub emit_raw_frames: bool,

    /// Enable CAN-TP auto-detection (scan for patterns)
    #[serde(default)]
    pub cantp_auto_detect: bool,

    /// CAN-TP timeout in milliseconds (default: 1000ms)
    #[serde(default = "default_cantp_timeout")]
    pub cantp_timeout_ms: u64,

    /// Maximum flow control wait frames to handle (default: 10)
    #[serde(default = "default_max_wait_frames")]
    pub cantp_max_wait_frames: usize,
}

fn default_true() -> bool {
    true
}

fn default_cantp_timeout() -> u64 {
    1000
}

fn default_max_wait_frames() -> usize {
    10
}

/// CAN-TP address pair configuration
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct CanTpPair {
    /// Source address (request)
    pub source: u32,
    /// Target address (response)
    pub target: u32,
    /// Optional name for documentation
    pub name: Option<String>,
}

impl CanTpPair {
    /// Create a new CAN-TP pair
    pub fn new(source: u32, target: u32) -> Self {
        Self {
            source,
            target,
            name: None,
        }
    }

    /// Create a new CAN-TP pair with a name
    pub fn with_name(source: u32, target: u32, name: impl Into<String>) -> Self {
        Self {
            source,
            target,
            name: Some(name.into()),
        }
    }
}

impl DecoderConfig {
    /// Create a new decoder configuration with default settings
    pub fn new() -> Self {
        Self::default()
    }

    /// Builder method: enable or disable signal decoding
    pub fn with_signal_decoding(mut self, enabled: bool) -> Self {
        self.decode_signals = enabled;
        self
    }

    /// Builder method: add a CAN-TP pair
    pub fn add_cantp_pair(mut self, source: u32, target: u32) -> Self {
        self.cantp_pairs.push(CanTpPair::new(source, target));
        self
    }

    /// Builder method: add a CAN-TP pair with a name
    pub fn add_named_cantp_pair(mut self, source: u32, target: u32, name: impl Into<String>) -> Self {
        self.cantp_pairs.push(CanTpPair::with_name(source, target, name));
        self
    }

    /// Builder method: add a container PDU ID
    pub fn add_container_id(mut self, container_id: u32) -> Self {
        self.container_ids.push(container_id);
        self
    }

    /// Builder method: set channel filter
    pub fn with_channel_filter(mut self, channels: Vec<u8>) -> Self {
        self.channel_filter = Some(channels);
        self
    }

    /// Builder method: set message filter
    pub fn with_message_filter(mut self, messages: Vec<u32>) -> Self {
        self.message_filter = Some(messages);
        self
    }

    /// Builder method: enable raw frame emission
    pub fn with_raw_frames(mut self, enabled: bool) -> Self {
        self.emit_raw_frames = enabled;
        self
    }

    /// Builder method: enable CAN-TP auto-detection
    pub fn with_cantp_auto_detect(mut self, enabled: bool) -> Self {
        self.cantp_auto_detect = enabled;
        self
    }

    /// Check if a channel should be processed
    pub fn should_process_channel(&self, channel: u8) -> bool {
        match &self.channel_filter {
            Some(channels) => channels.contains(&channel),
            None => true,
        }
    }

    /// Check if a message ID should be processed
    pub fn should_process_message(&self, can_id: u32) -> bool {
        match &self.message_filter {
            Some(messages) => messages.contains(&can_id),
            None => true,
        }
    }

    /// Check if a frame should be processed based on filters
    pub fn should_process_frame(&self, channel: u8, can_id: u32) -> bool {
        self.should_process_channel(channel) && self.should_process_message(can_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_decoder_config_builder() {
        let config = DecoderConfig::new()
            .with_signal_decoding(true)
            .add_cantp_pair(0x7E0, 0x7E8)
            .add_named_cantp_pair(0x7E1, 0x7E9, "TCU_Diagnostics")
            .add_container_id(0x100)
            .with_channel_filter(vec![0, 1])
            .with_cantp_auto_detect(true);

        assert!(config.decode_signals);
        assert_eq!(config.cantp_pairs.len(), 2);
        assert_eq!(config.container_ids, vec![0x100]);
        assert_eq!(config.channel_filter, Some(vec![0, 1]));
        assert!(config.cantp_auto_detect);
    }

    #[test]
    fn test_filter_logic() {
        let config = DecoderConfig::new()
            .with_channel_filter(vec![0, 1])
            .with_message_filter(vec![0x123, 0x456]);

        assert!(config.should_process_frame(0, 0x123));
        assert!(config.should_process_frame(1, 0x456));
        assert!(!config.should_process_frame(2, 0x123)); // Wrong channel
        assert!(!config.should_process_frame(0, 0x789)); // Wrong message
    }

    #[test]
    fn test_no_filters() {
        let config = DecoderConfig::new();

        // Without filters, everything should pass
        assert!(config.should_process_frame(0, 0x123));
        assert!(config.should_process_frame(99, 0xFFFFFFFF));
    }
}
