//! MF4 (Measurement Data Format 4) file parser
//!
//! Parses ASAM MDF4 files for automotive measurement data using mdflib.
//! MF4 is a standardized format that can contain CAN bus data along with
//! other automotive measurement data.
//!
//! This implementation uses FFI bindings to the mdflib C++ library.

use crate::types::{CanFrame, DecoderError, Result};
use std::ffi::CString;
use std::path::Path;

use super::mf4_ffi::*;

/// MF4 file parser using mdflib
pub struct Mf4Parser;

impl Mf4Parser {
    /// Parse an MF4 file and return an iterator over CAN frames
    ///
    /// Uses the mdflib C++ library via FFI to parse MDF4 files.
    pub fn parse(path: &Path) -> Result<Mf4FrameIterator> {
        log::info!("Parsing MF4 file: {:?}", path);

        if !path.exists() {
            return Err(DecoderError::LogParseError(format!(
                "MF4 file not found: {:?}",
                path
            )));
        }

        // Convert path to C string
        let path_str = path.to_str().ok_or_else(|| {
            DecoderError::LogParseError(format!("Invalid UTF-8 in path: {:?}", path))
        })?;

        let c_path = CString::new(path_str).map_err(|e| {
            DecoderError::LogParseError(format!("Failed to convert path to C string: {}", e))
        })?;

        // Open the MDF file
        let mut error = MdfError::Ok;
        let reader = unsafe { mdf_open(c_path.as_ptr(), &mut error) };

        if reader.is_null() || error != MdfError::Ok {
            let err_msg = get_last_error();
            return Err(DecoderError::LogParseError(format!(
                "Failed to open MF4 file: {} ({:?})",
                err_msg, error
            )));
        }

        log::info!("MF4 file opened successfully");

        // Create CAN iterator
        let mut iter_error = MdfError::Ok;
        let iterator = unsafe { mdf_create_can_iterator(reader, &mut iter_error) };

        if iterator.is_null() || iter_error != MdfError::Ok {
            let err_msg = get_last_error();
            unsafe { mdf_close(reader) };
            return Err(DecoderError::LogParseError(format!(
                "Failed to create CAN iterator: {} ({:?})",
                err_msg, iter_error
            )));
        }

        log::info!("CAN iterator created successfully");

        Ok(Mf4FrameIterator {
            reader,
            iterator,
            finished: false,
        })
    }
}

/// Iterator over CAN frames from an MF4 file
pub struct Mf4FrameIterator {
    reader: MdfReaderHandle,
    iterator: MdfIteratorHandle,
    finished: bool,
}

impl Iterator for Mf4FrameIterator {
    type Item = Result<CanFrame>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.finished {
            return None;
        }

        let mut mdf_frame = MdfCanFrame {
            timestamp_ns: 0,
            channel: 0,
            can_id: 0,
            data: [0; 64],
            data_length: 0,
            is_extended: 0,
            is_fd: 0,
            is_error_frame: 0,
            is_remote_frame: 0,
        };

        let result = unsafe { mdf_iterator_next(self.iterator, &mut mdf_frame) };

        match result {
            MdfError::Ok => {
                // Convert MdfCanFrame to CanFrame
                let mut data = Vec::with_capacity(mdf_frame.data_length as usize);
                data.extend_from_slice(&mdf_frame.data[..mdf_frame.data_length as usize]);

                Some(Ok(CanFrame {
                    timestamp_ns: mdf_frame.timestamp_ns,
                    channel: mdf_frame.channel,
                    can_id: mdf_frame.can_id,
                    data,
                    is_extended: mdf_frame.is_extended != 0,
                    is_fd: mdf_frame.is_fd != 0,
                    is_error_frame: mdf_frame.is_error_frame != 0,
                    is_remote_frame: mdf_frame.is_remote_frame != 0,
                }))
            }
            MdfError::EndOfData => {
                self.finished = true;
                None
            }
            _ => {
                self.finished = true;
                let err_msg = get_last_error();
                Some(Err(DecoderError::LogParseError(format!(
                    "Error reading CAN frame: {} ({:?})",
                    err_msg, result
                ))))
            }
        }
    }
}

impl Drop for Mf4FrameIterator {
    fn drop(&mut self) {
        unsafe {
            if !self.iterator.is_null() {
                mdf_iterator_free(self.iterator);
            }
            if !self.reader.is_null() {
                mdf_close(self.reader);
            }
        }
        log::debug!("MF4 parser resources released");
    }
}

// Ensure the iterator is Send and Sync if the handles are
unsafe impl Send for Mf4FrameIterator {}
unsafe impl Sync for Mf4FrameIterator {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mf4_file_not_found() {
        let result = Mf4Parser::parse(Path::new("nonexistent.mf4"));
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_mf4() {
        // Test with the actual example files if available
        let workspace_root = std::env::var("CARGO_MANIFEST_DIR")
            .map(|p| std::path::PathBuf::from(p).parent().unwrap().to_path_buf())
            .unwrap_or_else(|_| std::path::PathBuf::from(".."));

        let test_files = vec![
            workspace_root.join("arxml/test_batch.mf4"),
            workspace_root.join("arxml/test_batch_cut_0.mf4"),
            workspace_root.join("arxml/test_batch_cut_1.mf4"),
            workspace_root.join("arxml/test_metadata.mf4"),
        ];

        for test_path in test_files {
            if test_path.exists() {
                println!("\n=== Testing MF4: {:?} ===", test_path);
                match Mf4Parser::parse(&test_path) {
                    Ok(iterator) => {
                        println!("✓ MF4 file opened successfully");
                        let frame_count: usize = iterator
                            .map(|r| match r {
                                Ok(_) => 1,
                                Err(e) => {
                                    println!("  Frame error: {}", e);
                                    0
                                }
                            })
                            .sum();
                        println!("  Frames read: {}", frame_count);
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
