//! AUTOSAR Container PDU decoder
//!
//! This module implements unpacking of AUTOSAR Container I-PDUs, which can contain
//! multiple smaller PDUs within a single CAN frame.
//!
//! ## Container Types
//!
//! - **Static Container**: Fixed layout with PDUs always at the same positions
//! - **Dynamic Container**: Variable layout with header indicating which PDUs are present
//!   - SHORT-HEADER: 4 bytes (32 bits) with PDU ID + length for each contained PDU
//!   - LONG-HEADER: 8 bytes (64 bits) with extended information
//! - **Queued Container**: Multiple instances of the same PDU type

use crate::message_decoder::MessageDecoder;
use crate::signals::database::{ContainerDefinition, ContainerLayout, SignalDatabase};
use crate::types::{CanFrame, ContainerType, DecodedEvent, Result, DecoderError};

/// Container PDU decoder
pub struct ContainerDecoder;

impl ContainerDecoder {
    /// Decode a Container I-PDU from a CAN frame
    ///
    /// # Arguments
    ///
    /// * `frame` - The CAN frame containing the container
    /// * `container_def` - Container definition from ARXML
    /// * `signal_db` - Signal database for decoding contained PDUs
    ///
    /// # Returns
    ///
    /// Vector of decoded events (one per contained PDU, plus the container event itself)
    pub fn decode_container(
        frame: &CanFrame,
        container_def: &ContainerDefinition,
        signal_db: &SignalDatabase,
    ) -> Result<Vec<DecodedEvent>> {
        let mut events = Vec::new();

        match &container_def.layout {
            ContainerLayout::Static { pdus } => {
                // Static container: PDUs are always at fixed positions
                events.extend(Self::decode_static_container(frame, container_def, pdus, signal_db)?);
            }
            ContainerLayout::Dynamic { header_size, pdus } => {
                // Dynamic container: Header indicates which PDUs are present
                events.extend(Self::decode_dynamic_container(
                    frame,
                    container_def,
                    *header_size,
                    pdus,
                    signal_db,
                )?);
            }
            ContainerLayout::Queued { pdu_id, pdu_size } => {
                // Queued container: Multiple instances of same PDU
                events.extend(Self::decode_queued_container(
                    frame,
                    container_def,
                    *pdu_id,
                    *pdu_size,
                    signal_db,
                )?);
            }
        }

        Ok(events)
    }

    /// Decode a Static Container PDU
    ///
    /// Static containers have a fixed layout where PDUs are always at the same byte positions.
    /// All PDUs are always present in every frame.
    fn decode_static_container(
        frame: &CanFrame,
        container_def: &ContainerDefinition,
        pdus: &[crate::signals::database::ContainedPduInfo],
        signal_db: &SignalDatabase,
    ) -> Result<Vec<DecodedEvent>> {
        let mut contained_pdus = Vec::new();
        let mut decoded_events = Vec::new();
        let mut warning_count = 0;
        const MAX_WARNINGS: usize = 5; // Limit warnings to prevent spam

        for pdu_info in pdus {
            // Validate position and size
            let end_pos = pdu_info.position + pdu_info.size;
            if end_pos > frame.data.len() {
                warning_count += 1;
                if warning_count <= MAX_WARNINGS {
                    log::warn!(
                        "PDU {} at position {} with size {} exceeds frame data length {} (warning {}/{})",
                        pdu_info.name,
                        pdu_info.position,
                        pdu_info.size,
                        frame.data.len(),
                        warning_count,
                        MAX_WARNINGS
                    );
                } else if warning_count == MAX_WARNINGS + 1 {
                    log::warn!("... suppressing further position warnings for this container");
                }
                continue;
            }

            // Extract PDU data
            let pdu_data = frame.data[pdu_info.position..end_pos].to_vec();

            // Add to contained PDUs list
            contained_pdus.push(crate::types::ContainedPdu {
                pdu_id: pdu_info.pdu_id,
                name: pdu_info.name.clone(),
                data: pdu_data.clone(),
            });

            // Try to decode signals from this PDU
            if let Some(message_def) = signal_db.get_message_by_name(&pdu_info.name) {
                if let Some(decoded_message) = MessageDecoder::decode_pdu_data(
                    &pdu_data,
                    message_def,
                    frame.timestamp(),
                ) {
                    log::debug!(
                        "Decoded {} signals from contained PDU: {}",
                        match &decoded_message {
                            DecodedEvent::Message { signals, .. } => signals.len(),
                            _ => 0,
                        },
                        pdu_info.name
                    );
                    decoded_events.push(decoded_message);
                }
            } else {
                log::debug!(
                    "No signal definition found for contained PDU: {}",
                    pdu_info.name
                );
            }
        }

        // Create container PDU event
        let mut events = vec![DecodedEvent::ContainerPdu {
            timestamp: frame.timestamp(),
            container_id: container_def.id,
            container_name: container_def.name.clone(),
            container_type: container_def.container_type,
            contained_pdus,
        }];

        // Add all decoded message events from contained PDUs
        events.extend(decoded_events);

        Ok(events)
    }

    /// Decode a Dynamic Container PDU
    ///
    /// Dynamic containers use a header to indicate which PDUs are present.
    ///
    /// ## SHORT-HEADER Format (4 bytes per PDU):
    /// ```text
    /// Byte 0-1: PDU ID (16 bits, big-endian)
    /// Byte 2:   PDU length (8 bits)
    /// Byte 3:   Reserved/CRC
    /// ```
    ///
    /// ## LONG-HEADER Format (8 bytes per PDU):
    /// ```text
    /// Byte 0-3: PDU ID (32 bits, big-endian)
    /// Byte 4-7: PDU length + metadata
    /// ```
    fn decode_dynamic_container(
        frame: &CanFrame,
        container_def: &ContainerDefinition,
        header_size: usize,
        pdus: &[crate::signals::database::ContainedPduInfo],
        signal_db: &SignalDatabase,
    ) -> Result<Vec<DecodedEvent>> {
        if frame.data.len() < header_size {
            return Err(DecoderError::InvalidData(format!(
                "Frame too small for dynamic container header: {} < {}",
                frame.data.len(),
                header_size
            )));
        }

        let mut contained_pdus = Vec::new();
        let mut decoded_events = Vec::new();
        let mut offset = 0;

        // Parse headers until we run out of data or hit a zero header
        while offset + header_size <= frame.data.len() {
            let header_bytes = &frame.data[offset..offset + header_size];

            // Check for end marker (all zeros)
            if header_bytes.iter().all(|&b| b == 0) {
                break;
            }

            let (pdu_id, pdu_length) = if header_size == 4 {
                // SHORT-HEADER: 2 bytes ID + 1 byte length + 1 byte reserved
                let id = u16::from_be_bytes([header_bytes[0], header_bytes[1]]) as u32;
                let len = header_bytes[2] as usize;
                (id, len)
            } else if header_size == 8 {
                // LONG-HEADER: 4 bytes ID + 4 bytes length/metadata
                let id = u32::from_be_bytes([
                    header_bytes[0],
                    header_bytes[1],
                    header_bytes[2],
                    header_bytes[3],
                ]);
                let len = u32::from_be_bytes([
                    header_bytes[4],
                    header_bytes[5],
                    header_bytes[6],
                    header_bytes[7],
                ]) as usize;
                (id, len)
            } else {
                return Err(DecoderError::InvalidData(format!(
                    "Unsupported header size: {}",
                    header_size
                )));
            };

            // Move past header
            offset += header_size;

            // Validate PDU length
            if offset + pdu_length > frame.data.len() {
                log::warn!(
                    "PDU with ID {} has length {} that exceeds remaining frame data",
                    pdu_id,
                    pdu_length
                );
                break;
            }

            // Extract PDU data
            let pdu_data = frame.data[offset..offset + pdu_length].to_vec();
            offset += pdu_length;

            // Look up PDU info to get the name
            let pdu_info = pdus.iter().find(|p| p.pdu_id == pdu_id);
            let pdu_name = pdu_info
                .map(|p| p.name.clone())
                .unwrap_or_else(|| format!("PDU_{}", pdu_id));

            // Add to contained PDUs list
            contained_pdus.push(crate::types::ContainedPdu {
                pdu_id,
                name: pdu_name.clone(),
                data: pdu_data.clone(),
            });

            // Try to decode signals from this PDU
            if let Some(message_def) = signal_db.get_message_by_name(&pdu_name) {
                if let Some(decoded_message) = MessageDecoder::decode_pdu_data(
                    &pdu_data,
                    message_def,
                    frame.timestamp(),
                ) {
                    log::debug!(
                        "Decoded signals from dynamic contained PDU: {}",
                        pdu_name
                    );
                    decoded_events.push(decoded_message);
                }
            }
        }

        // Create container PDU event
        let mut events = vec![DecodedEvent::ContainerPdu {
            timestamp: frame.timestamp(),
            container_id: container_def.id,
            container_name: container_def.name.clone(),
            container_type: container_def.container_type,
            contained_pdus,
        }];

        // Add all decoded message events from contained PDUs
        events.extend(decoded_events);

        Ok(events)
    }

    /// Decode a Queued Container PDU
    ///
    /// Queued containers contain multiple instances of the same PDU type,
    /// packed sequentially without individual headers.
    fn decode_queued_container(
        frame: &CanFrame,
        container_def: &ContainerDefinition,
        pdu_id: u32,
        pdu_size: usize,
        signal_db: &SignalDatabase,
    ) -> Result<Vec<DecodedEvent>> {
        let mut contained_pdus = Vec::new();
        let mut decoded_events = Vec::new();
        let mut offset = 0;
        let mut instance = 0;

        // For queued containers, try to find the PDU name by looking it up in the signal database
        // Since queued containers only store pdu_id, we'll try to resolve the name via message lookup
        let pdu_name_base: Option<String> = None; // Will be looked up per-instance if needed

        // Extract PDUs until we run out of data
        while offset + pdu_size <= frame.data.len() {
            let pdu_data = frame.data[offset..offset + pdu_size].to_vec();

            // Check if PDU is empty (all zeros) - indicates end of queue
            if pdu_data.iter().all(|&b| b == 0) {
                break;
            }

            let pdu_name = format!("PDU_{}_{}", pdu_id, instance);

            // Add to contained PDUs list
            contained_pdus.push(crate::types::ContainedPdu {
                pdu_id,
                name: pdu_name.clone(),
                data: pdu_data.clone(),
            });

            // Try to decode signals from this PDU by looking up the message by CAN ID
            // For queued containers, the pdu_id may map to a CAN message ID
            if let Some(message_def) = signal_db.get_message(pdu_id) {
                if let Some(decoded_message) = MessageDecoder::decode_pdu_data(
                    &pdu_data,
                    message_def,
                    frame.timestamp(),
                ) {
                    log::debug!(
                        "Decoded signals from queued PDU instance {}: ID 0x{:X}",
                        instance,
                        pdu_id
                    );
                    decoded_events.push(decoded_message);
                }
            }

            offset += pdu_size;
            instance += 1;
        }

        // Create container PDU event
        let mut events = vec![DecodedEvent::ContainerPdu {
            timestamp: frame.timestamp(),
            container_id: container_def.id,
            container_name: container_def.name.clone(),
            container_type: container_def.container_type,
            contained_pdus,
        }];

        // Add all decoded message events from contained PDUs
        events.extend(decoded_events);

        Ok(events)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::signals::database::{ContainedPduInfo, SignalDatabase};
    use crate::types::CanFrame;

    fn create_test_frame(data: Vec<u8>) -> CanFrame {
        CanFrame {
            timestamp_ns: 1000000000,
            channel: 0,
            can_id: 0x100,
            data,
            is_extended: false,
            is_fd: false,
            is_error_frame: false,
            is_remote_frame: false,
        }
    }

    fn create_test_signal_db() -> SignalDatabase {
        // Create an empty signal database for tests
        SignalDatabase::new()
    }

    #[test]
    fn test_static_container() {
        let frame = create_test_frame(vec![
            0x11, 0x22, // PDU 1 (2 bytes at offset 0)
            0x33, 0x44, 0x55, // PDU 2 (3 bytes at offset 2)
            0x66, 0x77, 0x88, // PDU 3 (3 bytes at offset 5)
        ]);

        let pdus = vec![
            ContainedPduInfo {
                pdu_id: 1,
                name: "PDU1".to_string(),
                position: 0,
                size: 2,
            },
            ContainedPduInfo {
                pdu_id: 2,
                name: "PDU2".to_string(),
                position: 2,
                size: 3,
            },
            ContainedPduInfo {
                pdu_id: 3,
                name: "PDU3".to_string(),
                position: 5,
                size: 3,
            },
        ];

        let container_def = ContainerDefinition {
            id: 0x100,
            name: "TestContainer".to_string(),
            container_type: ContainerType::Static,
            layout: ContainerLayout::Static {
                pdus: pdus.clone(),
            },
            source: "test".to_string(),
        };

        let signal_db = create_test_signal_db();
        let events = ContainerDecoder::decode_static_container(&frame, &container_def, &pdus, &signal_db)
            .expect("Failed to decode static container");

        assert_eq!(events.len(), 1);
        if let DecodedEvent::ContainerPdu { contained_pdus, .. } = &events[0] {
            assert_eq!(contained_pdus.len(), 3);
            assert_eq!(contained_pdus[0].data, vec![0x11, 0x22]);
            assert_eq!(contained_pdus[1].data, vec![0x33, 0x44, 0x55]);
            assert_eq!(contained_pdus[2].data, vec![0x66, 0x77, 0x88]);
        } else {
            panic!("Expected ContainerPdu event");
        }
    }

    #[test]
    fn test_dynamic_container_short_header() {
        let frame = create_test_frame(vec![
            // Header 1: PDU ID=0x0001, Length=2
            0x00, 0x01, 0x02, 0x00, // PDU 1 data
            0xAA, 0xBB, // Header 2: PDU ID=0x0002, Length=3
            0x00, 0x02, 0x03, 0x00, // PDU 2 data
            0xCC, 0xDD, 0xEE, // End marker
            0x00, 0x00, 0x00, 0x00,
        ]);

        let container_def = ContainerDefinition {
            id: 0x100,
            name: "DynamicContainer".to_string(),
            container_type: ContainerType::Dynamic,
            layout: ContainerLayout::Dynamic {
                header_size: 4,
                pdus: Vec::new(),
            },
            source: "test".to_string(),
        };

        let signal_db = create_test_signal_db();
        let events =
            ContainerDecoder::decode_dynamic_container(&frame, &container_def, 4, &[], &signal_db)
                .expect("Failed to decode dynamic container");

        assert_eq!(events.len(), 1);
        if let DecodedEvent::ContainerPdu { contained_pdus, .. } = &events[0] {
            assert_eq!(contained_pdus.len(), 2);
            assert_eq!(contained_pdus[0].pdu_id, 1);
            assert_eq!(contained_pdus[0].data, vec![0xAA, 0xBB]);
            assert_eq!(contained_pdus[1].pdu_id, 2);
            assert_eq!(contained_pdus[1].data, vec![0xCC, 0xDD, 0xEE]);
        } else {
            panic!("Expected ContainerPdu event");
        }
    }

    #[test]
    fn test_queued_container() {
        let frame = create_test_frame(vec![
            0x11, 0x22, // Instance 0
            0x33, 0x44, // Instance 1
            0x55, 0x66, // Instance 2
            0x00, 0x00, // End marker (all zeros)
        ]);

        let container_def = ContainerDefinition {
            id: 0x100,
            name: "QueuedContainer".to_string(),
            container_type: ContainerType::Queued,
            layout: ContainerLayout::Queued {
                pdu_id: 42,
                pdu_size: 2,
            },
            source: "test".to_string(),
        };

        let signal_db = create_test_signal_db();
        let events = ContainerDecoder::decode_queued_container(&frame, &container_def, 42, 2, &signal_db)
            .expect("Failed to decode queued container");

        assert_eq!(events.len(), 1);
        if let DecodedEvent::ContainerPdu { contained_pdus, .. } = &events[0] {
            assert_eq!(contained_pdus.len(), 3);
            assert_eq!(contained_pdus[0].data, vec![0x11, 0x22]);
            assert_eq!(contained_pdus[1].data, vec![0x33, 0x44]);
            assert_eq!(contained_pdus[2].data, vec![0x55, 0x66]);
        } else {
            panic!("Expected ContainerPdu event");
        }
    }
}
