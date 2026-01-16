//! Message Decoding Engine
//!
//! Extracts signal values from raw CAN frames based on signal definitions
//! from the signal database. Handles bit extraction, endianness, multiplexing,
//! and physical value conversion.

use crate::signals::database::{ByteOrder, MessageDefinition, SignalDefinition, ValueType};
use crate::types::{CanFrame, DecodedEvent, DecodedSignal, SignalValue, Timestamp};
use std::collections::HashMap;

/// Message decoder - extracts signals from CAN frames
pub struct MessageDecoder;

impl MessageDecoder {
    /// Decode a CAN frame into a DecodedEvent::Message
    ///
    /// # Arguments
    /// * `frame` - Raw CAN frame
    /// * `message_def` - Message definition from signal database
    ///
    /// # Returns
    /// * `Some(DecodedEvent::Message)` if decoding succeeded
    /// * `None` if no signals could be decoded
    pub fn decode_message(frame: &CanFrame, message_def: &MessageDefinition) -> Option<DecodedEvent> {
        let mut decoded_signals = Vec::new();
        let mut multiplexer_value: Option<u64> = None;

        // For multiplexed messages, first extract the multiplexer signal value
        if message_def.is_multiplexed {
            if let Some(ref mux_signal_name) = message_def.multiplexer_signal {
                // Find the multiplexer signal
                if let Some(mux_signal) = message_def.signals.iter().find(|s| s.name == *mux_signal_name) {
                    // Extract multiplexer value
                    if let Some(value) = Self::extract_signal_value(&frame.data, mux_signal) {
                        multiplexer_value = Some(value as u64);
                    }
                }
            }
        }

        // Decode all signals (non-multiplexed and applicable multiplexed ones)
        for signal in &message_def.signals {
            // Check if signal should be decoded based on multiplexer
            if let Some(ref mux_info) = signal.multiplexer_info {
                // This signal is multiplexed - check if it should be active
                if let Some(current_mux_value) = multiplexer_value {
                    if !mux_info.multiplexer_values.contains(&current_mux_value) {
                        // Skip this signal - multiplexer value doesn't match
                        continue;
                    }
                } else {
                    // No multiplexer value extracted - skip multiplexed signals
                    continue;
                }
            }

            // Extract signal value
            if let Some(decoded) = Self::decode_signal(&frame.data, signal) {
                decoded_signals.push(decoded);
            }
        }

        // Only emit event if we decoded at least one signal
        if decoded_signals.is_empty() {
            return None;
        }

        Some(DecodedEvent::Message {
            timestamp: frame.timestamp(),
            channel: frame.channel,
            can_id: frame.can_id,
            message_name: Some(message_def.name.clone()),
            sender: message_def.sender.clone(),
            signals: decoded_signals,
            is_multiplexed: message_def.is_multiplexed,
            multiplexer_value,
        })
    }

    /// Decode a single signal from CAN frame data
    fn decode_signal(data: &[u8], signal: &SignalDefinition) -> Option<DecodedSignal> {
        // Extract raw value from CAN frame data
        let raw_value = Self::extract_signal_value(data, signal)?;

        // Apply physical value conversion (factor and offset)
        let physical_value = signal.offset + signal.factor * (raw_value as f64);

        // Determine value type and create appropriate SignalValue
        let value = if signal.factor == 1.0 && signal.offset == 0.0 && signal.length == 1 {
            // Boolean signal (single bit, no scaling)
            SignalValue::Boolean(raw_value != 0)
        } else if signal.factor != 1.0 || signal.offset != 0.0 {
            // Scaled signal - use float
            SignalValue::Float(physical_value)
        } else {
            // Integer signal (no scaling)
            SignalValue::Integer(raw_value)
        };

        // Look up value description from value table
        let value_description = signal
            .value_table
            .as_ref()
            .and_then(|table| table.get(&raw_value))
            .cloned();

        Some(DecodedSignal {
            name: signal.name.clone(),
            value,
            unit: signal.unit.clone(),
            value_description,
            raw_value,
        })
    }

    /// Extract raw signal value from CAN frame data
    ///
    /// Handles bit extraction with proper endianness support.
    /// This is the core signal extraction algorithm.
    fn extract_signal_value(data: &[u8], signal: &SignalDefinition) -> Option<i64> {
        let start_bit = signal.start_bit as usize;
        let length = signal.length as usize;

        // Validate signal fits within data
        let required_bytes = ((start_bit + length) + 7) / 8;
        if required_bytes > data.len() {
            log::warn!(
                "Signal '{}' requires {} bytes but frame only has {} bytes",
                signal.name,
                required_bytes,
                data.len()
            );
            return None;
        }

        // Extract raw bits based on byte order
        let raw_value = match signal.byte_order {
            ByteOrder::LittleEndian => Self::extract_little_endian(data, start_bit, length),
            ByteOrder::BigEndian => Self::extract_big_endian(data, start_bit, length),
        };

        // Apply sign extension if needed
        let signed_value = match signal.value_type {
            ValueType::Unsigned => raw_value as i64,
            ValueType::Signed => Self::sign_extend(raw_value, length),
        };

        Some(signed_value)
    }

    /// Extract signal with little-endian (Intel) byte order
    ///
    /// Little-endian format:
    /// - Start bit points to the LSB (least significant bit)
    /// - Bits are numbered from LSB to MSB within each byte
    /// - Byte 0 is the first byte in the CAN frame
    fn extract_little_endian(data: &[u8], start_bit: usize, length: usize) -> u64 {
        let mut result: u64 = 0;

        for i in 0..length {
            let bit_pos = start_bit + i;
            let byte_idx = bit_pos / 8;
            let bit_in_byte = bit_pos % 8;

            if byte_idx < data.len() {
                let bit_value = (data[byte_idx] >> bit_in_byte) & 0x01;
                result |= (bit_value as u64) << i;
            }
        }

        result
    }

    /// Extract signal with big-endian (Motorola) byte order
    ///
    /// Big-endian format in CAN:
    /// - Start bit points to the MSB (most significant bit) of the signal
    /// - Bit numbering: bit 0 = MSB of byte 0, bit 7 = LSB of byte 0
    /// - Signal grows downward (towards higher bit numbers)
    fn extract_big_endian(data: &[u8], start_bit: usize, length: usize) -> u64 {
        let mut result: u64 = 0;

        for i in 0..length {
            // In big-endian, start_bit is MSB, and we count forward
            let bit_pos = start_bit + i;
            let byte_idx = bit_pos / 8;
            let bit_in_byte = 7 - (bit_pos % 8); // Bit 0 = MSB, bit 7 = LSB

            if byte_idx < data.len() {
                let bit_value = (data[byte_idx] >> bit_in_byte) & 0x01;
                result |= (bit_value as u64) << (length - 1 - i);
            }
        }

        result
    }

    /// Sign-extend a value from N bits to 64 bits
    ///
    /// If the value's MSB is 1, fill the upper bits with 1s.
    /// This converts unsigned representation to proper signed value.
    fn sign_extend(value: u64, bit_length: usize) -> i64 {
        if bit_length >= 64 {
            return value as i64;
        }

        let sign_bit = 1u64 << (bit_length - 1);
        if (value & sign_bit) != 0 {
            // Negative value - sign extend
            let mask = !0u64 << bit_length;
            (value | mask) as i64
        } else {
            // Positive value
            value as i64
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_little_endian_simple() {
        // Signal: 8 bits starting at bit 0 (byte 0)
        let data = vec![0xAB, 0xCD, 0xEF, 0x12];
        let value = MessageDecoder::extract_little_endian(&data, 0, 8);
        assert_eq!(value, 0xAB);
    }

    #[test]
    fn test_extract_little_endian_cross_byte() {
        // Signal: 16 bits starting at bit 0 (bytes 0-1)
        let data = vec![0xAB, 0xCD, 0xEF, 0x12];
        let value = MessageDecoder::extract_little_endian(&data, 0, 16);
        assert_eq!(value, 0xCDAB); // Little-endian byte order
    }

    #[test]
    fn test_extract_big_endian_simple() {
        // Signal: 8 bits starting at bit 7 (byte 0)
        let data = vec![0xAB, 0xCD, 0xEF, 0x12];
        let value = MessageDecoder::extract_big_endian(&data, 7, 8);
        assert_eq!(value, 0xAB);
    }

    #[test]
    fn test_sign_extend_positive() {
        // 8-bit value 0x7F (127) should remain positive
        let value = MessageDecoder::sign_extend(0x7F, 8);
        assert_eq!(value, 127);
    }

    #[test]
    fn test_sign_extend_negative() {
        // 8-bit value 0xFF (-1 in two's complement) should become -1
        let value = MessageDecoder::sign_extend(0xFF, 8);
        assert_eq!(value, -1);
    }

    #[test]
    fn test_sign_extend_negative_16bit() {
        // 16-bit value 0x8000 (-32768 in two's complement)
        let value = MessageDecoder::sign_extend(0x8000, 16);
        assert_eq!(value, -32768);
    }
}
