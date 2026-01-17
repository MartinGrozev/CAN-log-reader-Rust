//! Log file format parsers (BLF, MF4)
//!
//! This module contains parsers for different CAN log file formats.
//! Each parser implements an iterator pattern over CanFrame objects.

use crate::types::{CanFrame, Result};
use std::path::Path;

pub mod blf;
pub mod mf4;
mod blf_extended;  // Extended BLF type support (100, 101)
pub mod blf_hybrid;    // Hybrid BLF parser with type 100/101 support
mod mf4_ffi;  // FFI bindings for mdflib (private module)

// Re-export parser types
pub use blf::{BlfParser, BlfFrameIterator};
pub use blf_hybrid::{HybridBlfParser, HybridBlfIterator};
pub use mf4::{Mf4Parser, Mf4FrameIterator};

/// Common trait for all log file parsers
///
/// This trait provides a unified interface for parsing different log file formats.
/// Each parser returns an iterator over CanFrame objects.
pub trait LogFileParser: Iterator<Item = Result<CanFrame>> + Sized {
    /// Parse a log file and return an iterator over CAN frames
    fn parse(path: &Path) -> Result<Self>;
}
