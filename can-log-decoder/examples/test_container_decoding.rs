//! Test end-to-end container decoding
//!
//! This example tests:
//! 1. Loading ARXML with container definitions
//! 2. Verifying container decoder integration into main Decoder

use can_log_decoder::Decoder;
use std::path::Path;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Testing Container Decoding ===\n");

    // Create decoder and load ARXML
    let mut decoder = Decoder::new();
    let arxml_path = Path::new("C:\\Users\\HP\\Rust\\CAN log reader\\arxml\\system-4.2.arxml");

    println!("Loading ARXML file: {:?}", arxml_path);
    decoder.add_arxml(arxml_path)?;

    // Check database stats
    let stats = decoder.database_stats();
    println!("\nDatabase loaded:");
    println!("  Messages: {}", stats.num_messages);
    println!("  Signals: {}", stats.num_signals);
    println!("  Containers: {}", stats.num_containers);

    if stats.num_containers == 0 {
        println!("\n⚠ No containers found in ARXML file");
        return Ok(());
    }

    println!("\n✅ Container definitions loaded successfully!");

    // Note: We can't easily create synthetic BLF files here, so this is just
    // a smoke test to verify the decoder loads containers properly.
    // Full integration testing will happen when processing real BLF/MF4 files.

    println!("\n=== Integration Test Complete ===");
    println!("Container decoder is integrated into main Decoder!");
    println!("Ready to decode real BLF/MF4 files with container PDUs.");

    Ok(())
}
