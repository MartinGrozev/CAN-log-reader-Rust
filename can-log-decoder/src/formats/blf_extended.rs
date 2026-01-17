//! Extended BLF object type support
//!
//! This module adds support for BLF object types that aren't handled by ablf crate v0.2.0,
//! specifically CAN-FD message types 100 and 101.
//!
//! Based on python-can implementation and Vector BLF specification.

use crate::types::CanFrame;
use std::io::{Read, Seek, SeekFrom};

/// BLF Object Header (32 bytes) - common to all object types
#[derive(Debug)]
pub struct ObjectHeader {
    pub signature: [u8; 4],      // "LOBJ" = 0x4A424F4C
    pub header_size: u16,         // 32
    pub header_version: u16,      // 1
    pub object_size: u32,         // Total size including header
    pub object_type: u32,         // Object type ID
}

impl ObjectHeader {
    /// Parse object header from reader
    pub fn parse<R: Read>(reader: &mut R) -> std::io::Result<Self> {
        use std::io::ErrorKind;

        let mut buf = [0u8; 32];
        reader.read_exact(&mut buf)?;

        let signature = [buf[0], buf[1], buf[2], buf[3]];

        // Validate signature
        if &signature != b"LOBJ" {
            return Err(std::io::Error::new(
                ErrorKind::InvalidData,
                format!("Invalid object signature: {:?}", signature),
            ));
        }

        let header_size = u16::from_le_bytes([buf[4], buf[5]]);
        let header_version = u16::from_le_bytes([buf[6], buf[7]]);
        let object_size = u32::from_le_bytes([buf[8], buf[9], buf[10], buf[11]]);
        let object_type = u32::from_le_bytes([buf[12], buf[13], buf[14], buf[15]]);

        Ok(ObjectHeader {
            signature,
            header_size,
            header_version,
            object_size,
            object_type,
        })
    }
}

/// CAN_FD_MESSAGE (type 100) structure
/// Based on python-can struct: "<HBBLLBBB5x64s"
#[derive(Debug)]
pub struct CanFdMessage {
    pub channel: u16,           // CAN channel (1-based in BLF, will be 0-based in CanFrame)
    pub flags: u8,              // Direction, remote frame, etc.
    pub dlc: u8,                // Data length code
    pub can_id: u32,            // CAN arbitration ID
    pub frame_length_ns: u32,   // Frame duration in nanoseconds
    pub bit_count: u8,          // Number of bits
    pub fd_flags: u8,           // CAN-FD specific flags
    pub valid_data_bytes: u8,   // Number of valid bytes in data
    // 5 reserved bytes
    pub data: [u8; 64],         // Frame payload (max 64 bytes for CAN-FD)
    pub timestamp_ns: u64,      // Timestamp from object header
}

impl CanFdMessage {
    /// Parse CAN-FD message (type 100) from reader
    /// Assumes object header has already been read
    pub fn parse<R: Read>(reader: &mut R, timestamp_ns: u64) -> std::io::Result<Self> {
        let mut buf = [0u8; 80];  // channel(2) + flags(1) + dlc(1) + id(4) + frame_len(4) +
                                   // bit_count(1) + fd_flags(1) + valid_bytes(1) + reserved(5) + data(64)
        reader.read_exact(&mut buf)?;

        let channel = u16::from_le_bytes([buf[0], buf[1]]);
        let flags = buf[2];
        let dlc = buf[3];
        let can_id = u32::from_le_bytes([buf[4], buf[5], buf[6], buf[7]]);
        let frame_length_ns = u32::from_le_bytes([buf[8], buf[9], buf[10], buf[11]]);
        let bit_count = buf[12];
        let fd_flags = buf[13];
        let valid_data_bytes = buf[14];
        // buf[15..20] are reserved (5 bytes)

        let mut data = [0u8; 64];
        data.copy_from_slice(&buf[20..84]);

        Ok(CanFdMessage {
            channel,
            flags,
            dlc,
            can_id,
            frame_length_ns,
            bit_count,
            fd_flags,
            valid_data_bytes,
            data,
            timestamp_ns,
        })
    }

    /// Convert to CanFrame
    pub fn to_can_frame(&self) -> CanFrame {
        // Extract valid data bytes
        let data_len = self.valid_data_bytes.min(64) as usize;
        let data = self.data[..data_len].to_vec();

        // Flag bits (from python-can)
        const CAN_MSG_EXT: u32 = 0x80000000;     // Extended ID flag in can_id
        const REMOTE_FLAG: u8 = 0x80;             // Remote frame flag in flags
        const DIR: u8 = 0x01;                     // Direction flag (0=RX, 1=TX)

        CanFrame {
            timestamp_ns: self.timestamp_ns,
            channel: if self.channel > 0 { self.channel as u8 - 1 } else { 0 },  // Convert 1-based to 0-based
            can_id: self.can_id & 0x1FFFFFFF,     // Mask out flag bits
            data,
            is_extended: (self.can_id & CAN_MSG_EXT) != 0,
            is_fd: (self.fd_flags & 0x01) != 0,    // CAN-FD flag
            is_error_frame: false,
            is_remote_frame: (self.flags & REMOTE_FLAG) != 0,
        }
    }
}

/// CAN_FD_MESSAGE_64 (type 101) structure
/// Similar to type 100 but might have slightly different layout
/// For now, we'll treat it the same as type 100
pub type CanFdMessage64 = CanFdMessage;

/// Try to parse types 100/101 manually from a BLF object
pub fn try_parse_canfd_message<R: Read + Seek>(
    reader: &mut R,
    obj_type: u32,
    object_size: u32,
) -> std::io::Result<Option<CanFrame>> {
    // Position should be just after object header (32 bytes)
    // Read timestamp from object header extended data
    let mut ts_buf = [0u8; 8];
    reader.read_exact(&mut ts_buf)?;
    let timestamp_ns = u64::from_le_bytes(ts_buf);

    match obj_type {
        100 => {
            // CAN_FD_MESSAGE
            let msg = CanFdMessage::parse(reader, timestamp_ns)?;
            Ok(Some(msg.to_can_frame()))
        }
        101 => {
            // CAN_FD_MESSAGE_64 (treat same as 100 for now)
            let msg = CanFdMessage64::parse(reader, timestamp_ns)?;
            Ok(Some(msg.to_can_frame()))
        }
        _ => {
            // Not a CAN-FD message we support
            // Skip remaining bytes
            let bytes_read = 32 + 8;  // header + timestamp
            if object_size > bytes_read {
                reader.seek(SeekFrom::Current((object_size - bytes_read) as i64))?;
            }
            Ok(None)
        }
    }
}
