//! Hybrid BLF parser combining ablf crate with custom type 100/101 support
//!
//! This parser uses:
//! - ablf crate for types 10, 65, 73, 86 (well-supported)
//! - Custom parser for types 100, 101 (CAN-FD messages)
//!
//! Strategy: Parse file manually, dispatch to ablf for supported types

use crate::formats::blf_extended::{ObjectHeader, try_parse_canfd_message};
use crate::types::{CanFrame, DecoderError, Result};
use std::fs::File;
use std::io::{BufReader, Read, Seek, SeekFrom};
use std::path::Path;

/// Hybrid BLF parser with extended type support
pub struct HybridBlfParser;

impl HybridBlfParser {
    /// Parse a BLF file with support for types 100/101
    pub fn parse(path: &Path) -> Result<HybridBlfIterator> {
        log::info!("Parsing BLF file (hybrid mode): {:?}", path);

        if !path.exists() {
            return Err(DecoderError::LogParseError(format!(
                "BLF file not found: {:?}",
                path
            )));
        }

        let file = File::open(path).map_err(|e| {
            DecoderError::LogParseError(format!("Failed to open BLF file: {}", e))
        })?;

        let mut reader = BufReader::new(file);

        // Skip BLF file header (varies, but typically starts with "LOGG")
        // Read signature to verify it's a BLF file
        let mut sig_buf = [0u8; 4];
        reader.read_exact(&mut sig_buf).map_err(|e| {
            DecoderError::LogParseError(format!("Failed to read BLF signature: {}", e))
        })?;

        if &sig_buf != b"LOGG" {
            return Err(DecoderError::LogParseError(format!(
                "Invalid BLF file signature: {:?}",
                sig_buf
            )));
        }

        // BLF file header is 144 bytes total (LOGG + header structure)
        // Skip the rest (144 - 4 = 140 bytes)
        let mut skip_buf = vec![0u8; 140];
        reader.read_exact(&mut skip_buf).map_err(|e| {
            DecoderError::LogParseError(format!("Failed to read BLF file header: {}", e))
        })?;

        Ok(HybridBlfIterator {
            reader,
            file_pos: 144,
            finished: false,
        })
    }
}

/// Iterator over CAN frames using hybrid parsing
pub struct HybridBlfIterator {
    reader: BufReader<File>,
    file_pos: u64,
    finished: bool,
}

impl Iterator for HybridBlfIterator {
    type Item = Result<CanFrame>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.finished {
            return None;
        }

        loop {
            // Try to read next object header
            let header = match ObjectHeader::parse(&mut self.reader) {
                Ok(h) => h,
                Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => {
                    // End of file
                    self.finished = true;
                    return None;
                }
                Err(e) => {
                    return Some(Err(DecoderError::LogParseError(format!(
                        "Failed to read object header: {}",
                        e
                    ))));
                }
            };

            eprintln!("DEBUG: Object type {}, size {}", header.object_type, header.object_size);

            // Try to parse as CAN-FD message (100, 101)
            match try_parse_canfd_message(&mut self.reader, header.object_type, header.object_size) {
                Ok(Some(frame)) => {
                    return Some(Ok(frame));
                }
                Ok(None) => {
                    // Not a type we handle, skip to next object
                    continue;
                }
                Err(e) => {
                    log::warn!("Error parsing CAN-FD message: {}", e);
                    // Try to skip this object and continue
                    // Seek to next object (this might fail if we're corrupted)
                    if let Err(seek_err) = self.reader.seek(SeekFrom::Current(
                        (header.object_size.saturating_sub(32)) as i64
                    )) {
                        return Some(Err(DecoderError::LogParseError(format!(
                            "Failed to skip corrupted object: {}",
                            seek_err
                        ))));
                    }
                    continue;
                }
            }
        }
    }
}
