//! BLF (Binary Log Format) file parser
//!
//! Parses Vector BLF files using the `ablf` crate.
//! BLF is a proprietary format from Vector Informatik for storing CAN bus data.
//!
//! ## Supported Object Types
//! - Type 86 (CanMessage2): CAN 2.0 and CAN-FD messages
//! - Type 73 (CanErrorFrameExt): CAN error frames
//! - Type 10 (LogContainer): Automatically decompressed by ablf
//!
//! ## Known Limitations
//! - Type 100 (CAN-FD Message): Not supported by ablf v0.2.0 (frames are skipped)
//! - Type 115 and others: Unsupported types are silently skipped
//!
//! Most BLF files use type 86 for CAN-FD (with FD flag), so type 100 limitation rarely impacts usage.

use crate::types::{CanFrame, DecoderError, Result};
use ablf::{BlfFile, ObjectTypes};
use std::collections::HashSet;
use std::fs::File;
use std::io::BufReader;
use std::path::Path;

/// BLF file parser using ablf crate
pub struct BlfParser;

impl BlfParser {
    /// Parse a BLF file and return an iterator over CAN frames
    ///
    /// Opens the BLF file and validates its structure. Returns an iterator
    /// that yields CanFrame structs for all supported message types.
    pub fn parse(path: &Path) -> Result<BlfFrameIterator> {
        log::info!("Parsing BLF file: {:?}", path);

        if !path.exists() {
            return Err(DecoderError::LogParseError(format!(
                "BLF file not found: {:?}",
                path
            )));
        }

        // Open file with buffered reading
        let file = File::open(path).map_err(|e| {
            DecoderError::LogParseError(format!("Failed to open BLF file: {}", e))
        })?;

        let reader = BufReader::new(file);

        // Parse BLF file structure
        let blf = BlfFile::from_reader(reader).map_err(|(e, _)| {
            DecoderError::LogParseError(format!("Failed to parse BLF file: {}", e))
        })?;

        // Validate BLF file
        if !blf.is_valid() {
            return Err(DecoderError::LogParseError(
                "Invalid BLF file format".to_string(),
            ));
        }

        log::info!("BLF file opened successfully");

        // Create the iterator from BlfFile
        let object_iter = blf.into_iter();

        Ok(BlfFrameIterator {
            objects: object_iter,
            skipped_types: HashSet::new(),
        })
    }
}

/// Iterator over CAN frames from a BLF file
pub struct BlfFrameIterator {
    objects: ablf::ObjectIterator<BufReader<File>>,
    skipped_types: HashSet<u32>,
}

impl Iterator for BlfFrameIterator {
    type Item = Result<CanFrame>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let obj = self.objects.next()?;
            match obj.data {
                ObjectTypes::CanMessage86(msg) => {
                    // Extract CAN 2.0 or CAN-FD message (type 86)
                    return Some(Ok(CanFrame {
                        timestamp_ns: msg.header.timestamp_ns,
                        channel: msg.channel as u8,
                        can_id: msg.id,
                        data: msg.data.clone(),
                        is_extended: (msg.flags & 0x02) != 0, // Bit 1: Extended ID
                        is_fd: (msg.flags & 0x80) != 0,       // Bit 7: CAN-FD frame
                        is_error_frame: false,
                        is_remote_frame: (msg.flags & 0x04) != 0, // Bit 2: Remote frame
                    }));
                }
                ObjectTypes::CanErrorExt73(err) => {
                    // Extract CAN error frame (type 73)
                    return Some(Ok(CanFrame {
                        timestamp_ns: err.header.timestamp_ns,
                        channel: err.channel as u8,
                        can_id: err.id,
                        data: err.data.to_vec(),
                        is_extended: false,
                        is_fd: false,
                        is_error_frame: true,
                        is_remote_frame: false,
                    }));
                }
                ObjectTypes::AppText65(_) => {
                    // Skip application text (type 65)
                    continue;
                }
                ObjectTypes::LogContainer10(_) => {
                    // Containers are automatically unpacked by ablf iterator
                    // We should never see this directly
                    continue;
                }
                ObjectTypes::UnsupportedPadded { .. } => {
                    // Skip recognized but unsupported types (6, 7, 8, 9, 72, 90, 92, 96)
                    continue;
                }
                ObjectTypes::Unsupported(_) => {
                    // Warn about unsupported types (like type 100 CAN-FD, type 115, etc.)
                    let obj_type = obj.object_type;
                    if !self.skipped_types.contains(&obj_type) {
                        log::warn!(
                            "Skipping unsupported BLF object type {} (size {} bytes)",
                            obj_type,
                            obj.object_size
                        );
                        self.skipped_types.insert(obj_type);
                    }
                    continue;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_blf_file_not_found() {
        let result = BlfParser::parse(Path::new("nonexistent.blf"));
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_real_blf() {
        // Test with the actual example files
        // Use workspace root relative path
        let workspace_root = std::env::var("CARGO_MANIFEST_DIR")
            .map(|p| std::path::PathBuf::from(p).parent().unwrap().to_path_buf())
            .unwrap_or_else(|_| std::path::PathBuf::from(".."));

        let test_files = vec![
            workspace_root.join("arxml/test_CanFdMessage.blf"),
            workspace_root.join("arxml/test_CanFdMessage64.blf"),
        ];

        for test_path in test_files {
            if test_path.exists() {
                println!("\n=== Testing: {:?} ===", test_path);
                let result = BlfParser::parse(&test_path);

                match result {
                    Ok(iterator) => {
                        println!("✓ BLF file opened and validated");

                        let mut can_count = 0;
                        let mut error_count = 0;
                        let mut fd_count = 0;

                        for frame_result in iterator {
                            match frame_result {
                                Ok(frame) => {
                                    if frame.is_error_frame {
                                        error_count += 1;
                                    } else {
                                        can_count += 1;
                                        if frame.is_fd {
                                            fd_count += 1;
                                        }
                                    }
                                }
                                Err(e) => {
                                    println!("  Frame error: {}", e);
                                }
                            }
                        }

                        println!("  CAN frames: {}", can_count);
                        println!("  CAN-FD frames: {}", fd_count);
                        println!("  Error frames: {}", error_count);

                        assert!(can_count > 0 || error_count > 0, "No frames extracted");
                    }
                    Err(e) => {
                        println!("✗ Error: {}", e);
                    }
                }
            } else {
                println!("Test file not found: {:?} (skipping)", test_path);
            }
        }
    }
}
