//! Main decoder API
//!
//! This module provides the primary interface for the decoder library.
//! The Decoder struct is the entry point for loading signal definitions and
//! decoding log files.

use crate::config::DecoderConfig;
use crate::signals::SignalDatabase;
use crate::types::{DecodedEvent, Result};
use std::path::Path;

/// The main decoder struct - entry point for all decoding operations
pub struct Decoder {
    /// Internal signal database (loaded from DBC/ARXML files)
    signal_db: SignalDatabase,
}

impl Decoder {
    /// Create a new decoder instance
    pub fn new() -> Self {
        Self {
            signal_db: SignalDatabase::new(),
        }
    }

    /// Load a DBC file and add its definitions to the signal database
    ///
    /// # Arguments
    /// * `path` - Path to the DBC file
    ///
    /// # Returns
    /// * `Result<()>` - Ok if loaded successfully, Err if parsing failed
    ///
    /// # Example
    /// ```no_run
    /// use can_log_decoder::Decoder;
    /// use std::path::Path;
    ///
    /// let mut decoder = Decoder::new();
    /// decoder.add_dbc(Path::new("powertrain.dbc")).unwrap();
    /// ```
    pub fn add_dbc(&mut self, path: &Path) -> Result<()> {
        log::info!("Loading DBC file: {:?}", path);

        // Parse DBC file
        let messages = crate::signals::dbc::parse_dbc_file(path)?;

        // Add all messages to the database
        for message in messages {
            self.signal_db.add_message(message);
        }

        log::info!("DBC file loaded successfully: {:?}", path);
        Ok(())
    }

    /// Load an ARXML file and add its definitions to the signal database
    ///
    /// # Arguments
    /// * `path` - Path to the ARXML file
    ///
    /// # Returns
    /// * `Result<()>` - Ok if loaded successfully, Err if parsing failed
    ///
    /// # Example
    /// ```no_run
    /// use can_log_decoder::Decoder;
    /// use std::path::Path;
    ///
    /// let mut decoder = Decoder::new();
    /// decoder.add_arxml(Path::new("system.arxml")).unwrap();
    /// ```
    pub fn add_arxml(&mut self, path: &Path) -> Result<()> {
        log::info!("Loading ARXML file: {:?}", path);

        // Parse ARXML file
        let (messages, containers) = crate::signals::arxml::parse_arxml_file(path)?;

        // Add all messages to the database
        for message in messages {
            self.signal_db.add_message(message);
        }

        // Add all containers to the database
        for container in containers {
            self.signal_db.add_container(container);
        }

        log::info!("ARXML file loaded successfully: {:?}", path);
        Ok(())
    }

    /// Decode a log file and return an iterator of decoded events
    ///
    /// This is the main decoding function. It returns an iterator that lazily decodes
    /// the log file, emitting DecodedEvent items as it processes frames.
    ///
    /// # Arguments
    /// * `path` - Path to the log file (BLF or MF4)
    /// * `config` - Decoder configuration
    ///
    /// # Returns
    /// * `Result<impl Iterator<Item = Result<DecodedEvent>>>` - Iterator of decoded events
    ///
    /// # Example
    /// ```no_run
    /// use can_log_decoder::{Decoder, DecoderConfig};
    /// use std::path::Path;
    ///
    /// let decoder = Decoder::new();
    /// let config = DecoderConfig::new();
    /// let events = decoder.decode_file(Path::new("trace.blf"), config).unwrap();
    ///
    /// for event in events {
    ///     match event {
    ///         Ok(decoded) => println!("Decoded event: {:?}", decoded),
    ///         Err(e) => eprintln!("Error: {}", e),
    ///     }
    /// }
    /// ```
    pub fn decode_file(
        &self,
        path: &Path,
        config: DecoderConfig,
    ) -> Result<Box<dyn Iterator<Item = Result<DecodedEvent>> + '_>> {
        log::info!("Decoding log file: {:?}", path);

        // Determine file type from extension
        let extension = path.extension()
            .and_then(|s| s.to_str())
            .map(|s| s.to_lowercase());

        match extension.as_deref() {
            Some("blf") => {
                log::debug!("Detected BLF file format");
                // TODO: Implement BLF parser in Phase 3
                Ok(Box::new(std::iter::empty()))
            }
            Some("mf4") | Some("mdf") => {
                log::debug!("Detected MF4 file format");
                // TODO: Implement MF4 parser in Phase 3
                Ok(Box::new(std::iter::empty()))
            }
            _ => {
                Err(crate::types::DecoderError::LogParseError(
                    format!("Unsupported file format: {:?}", extension)
                ))
            }
        }
    }

    /// Get statistics about the loaded signal database
    pub fn database_stats(&self) -> DatabaseStats {
        self.signal_db.stats()
    }
}

impl Default for Decoder {
    fn default() -> Self {
        Self::new()
    }
}

// Re-export DatabaseStats for public API
pub use crate::signals::DatabaseStats;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_decoder_creation() {
        let decoder = Decoder::new();
        let stats = decoder.database_stats();
        assert_eq!(stats.num_messages, 0);
        assert_eq!(stats.num_signals, 0);
    }

    #[test]
    fn test_unsupported_file_format() {
        let decoder = Decoder::new();
        let config = DecoderConfig::new();
        let result = decoder.decode_file(Path::new("test.txt"), config);
        assert!(result.is_err());
    }
}
