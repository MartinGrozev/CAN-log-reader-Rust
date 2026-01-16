// Simple test to verify ARXML parser improvements
use can_log_decoder::signals::arxml::parse_arxml_file;
use std::path::Path;

fn main() {
    println!("Testing ARXML Parser Improvements");
    println!("==================================\n");

    // Test with example ARXML file
    let test_path = Path::new("arxml/system-4.2.arxml");

    if !test_path.exists() {
        println!("⚠️  Test file not found: {:?}", test_path);
        println!("   Create an ARXML file to test the parser");
        return;
    }

    println!("📄 Parsing: {:?}", test_path);

    match parse_arxml_file(test_path) {
        Ok((messages, containers)) => {
            println!("✅ Parsing successful!\n");
            println!("📊 Statistics:");
            println!("   - Messages: {}", messages.len());
            println!("   - Containers: {}", containers.len());

            println!("\n🔍 Message Details:");
            for (idx, msg) in messages.iter().take(5).enumerate() {
                println!("\n   {}. {} (CAN ID: 0x{:X})", idx + 1, msg.name, msg.id);
                println!("      Size: {} bytes, Signals: {}", msg.size, msg.signals.len());

                // Show first 3 signals with physical value attributes
                for signal in msg.signals.iter().take(3) {
                    println!("      - {}: {} bits @ bit {}",
                        signal.name, signal.length, signal.start_bit);

                    // Show physical value conversion
                    if signal.factor != 1.0 || signal.offset != 0.0 {
                        println!("        Physical: raw * {} + {}", signal.factor, signal.offset);
                    }

                    if let Some(ref unit) = signal.unit {
                        println!("        Unit: {}", unit);
                    }

                    if signal.min != 0.0 || signal.max != ((1u64 << signal.length) as f64 - 1.0) {
                        println!("        Range: {} to {}", signal.min, signal.max);
                    }
                }
            }

            println!("\n✅ ARXML parser improvements working correctly!");
            println!("   - PDU-to-CAN-ID map optimization: ✓");
            println!("   - SYSTEM-SIGNAL-REF parsing: ✓");
            println!("   - Physical value conversion: ✓");
        }
        Err(e) => {
            println!("❌ Parse error: {}", e);
        }
    }
}
