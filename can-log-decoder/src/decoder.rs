//! Main decoder API
//!
//! This module provides the primary interface for the decoder library.
//! The Decoder struct is the entry point for loading signal definitions and
//! decoding log files.

use crate::config::DecoderConfig;
use crate::container_decoder::ContainerDecoder;
use crate::signals::SignalDatabase;
use crate::types::{CanFrame, DecodedEvent, Result};
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
        _config: DecoderConfig,
    ) -> Result<Box<dyn Iterator<Item = Result<DecodedEvent>> + '_>> {
        log::info!("Decoding log file: {:?}", path);

        // Determine file type from extension
        let extension = path.extension()
            .and_then(|s| s.to_str())
            .map(|s| s.to_lowercase());

        match extension.as_deref() {
            Some("blf") => {
                log::debug!("Detected BLF file format");
                let frame_iter = crate::formats::BlfParser::parse(path)?;
                Ok(Box::new(DecodingIterator::new(frame_iter, &self.signal_db)))
            }
            Some("mf4") | Some("mdf") => {
                log::debug!("Detected MF4 file format");
                let frame_iter = crate::formats::Mf4Parser::parse(path)?;
                Ok(Box::new(DecodingIterator::new(frame_iter, &self.signal_db)))
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

/// Iterator that decodes CAN frames into decoded events
///
/// This iterator wraps a frame iterator and processes each frame:
/// 1. Check if CAN ID is a container → decode container PDU
/// 2. Check if CAN ID is a message → decode message signals
/// 3. Otherwise → emit raw frame event
struct DecodingIterator<'a, I>
where
    I: Iterator<Item = Result<CanFrame>>,
{
    frame_iter: I,
    signal_db: &'a SignalDatabase,
    pending_events: Vec<DecodedEvent>,
}

impl<'a, I> DecodingIterator<'a, I>
where
    I: Iterator<Item = Result<CanFrame>>,
{
    fn new(frame_iter: I, signal_db: &'a SignalDatabase) -> Self {
        Self {
            frame_iter,
            signal_db,
            pending_events: Vec::new(),
        }
    }

    /// Process a single CAN frame and generate decoded event(s)
    fn process_frame(&mut self, frame: CanFrame) -> Result<Option<DecodedEvent>> {
        let can_id = frame.can_id;

        // Check if this is a container PDU
        if let Some(container_def) = self.signal_db.get_container(can_id) {
            log::debug!("Decoding container PDU: {} (ID: 0x{:X})", container_def.name, can_id);

            // Decode container - this returns a Vec of events
            let container_events = ContainerDecoder::decode_container(&frame, container_def, self.signal_db)?;

            // Split: first event to return, rest go to pending queue
            let mut events_iter = container_events.into_iter();
            let first_event = events_iter.next();

            // Store remaining events for later emission
            self.pending_events.extend(events_iter);

            // Return the first event
            Ok(first_event)
        }
        // Check if this is a regular message
        else if let Some(message_def) = self.signal_db.get_message(can_id) {
            log::debug!("Decoding message: {} (ID 0x{:X})", message_def.name, can_id);

            // Decode message signals using MessageDecoder
            if let Some(decoded_event) = crate::message_decoder::MessageDecoder::decode_message(&frame, message_def) {
                Ok(Some(decoded_event))
            } else {
                // Decoding failed, emit as raw frame
                log::warn!("Failed to decode message 0x{:X}, emitting as raw frame", can_id);
                Ok(Some(DecodedEvent::RawFrame {
                    timestamp: frame.timestamp(),
                    channel: frame.channel,
                    can_id: frame.can_id,
                    data: frame.data,
                    is_fd: frame.is_fd,
                }))
            }
        }
        // Unknown CAN ID - emit as raw frame
        else {
            log::trace!("Unknown CAN ID: 0x{:X}, emitting as raw frame", can_id);
            Ok(Some(DecodedEvent::RawFrame {
                timestamp: frame.timestamp(),
                channel: frame.channel,
                can_id: frame.can_id,
                data: frame.data,
                is_fd: frame.is_fd,
            }))
        }
    }
}

impl<'a, I> Iterator for DecodingIterator<'a, I>
where
    I: Iterator<Item = Result<CanFrame>>,
{
    type Item = Result<DecodedEvent>;

    fn next(&mut self) -> Option<Self::Item> {
        // First, return any pending events from container decoding
        if let Some(event) = self.pending_events.pop() {
            return Some(Ok(event));
        }

        // Get next frame from underlying iterator
        match self.frame_iter.next()? {
            Ok(frame) => {
                match self.process_frame(frame) {
                    Ok(Some(event)) => Some(Ok(event)),
                    Ok(None) => self.next(), // No event generated, get next frame
                    Err(e) => Some(Err(e)),
                }
            }
            Err(e) => Some(Err(e)),
        }
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
