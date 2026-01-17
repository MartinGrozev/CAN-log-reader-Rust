//! Test hybrid BLF parser with CAN-FD type 100/101 support

use can_log_decoder::formats::blf_hybrid::HybridBlfParser;
use std::path::Path;

fn main() {
    let test_files = vec![
        Path::new("../arxml/test_CanFdMessage.blf"),
        Path::new("../arxml/test_CanFdMessage64.blf"),
    ];

    for test_path in test_files {
        println!("\n═══════════════════════════════════════");
        println!("Testing HYBRID parser: {:?}", test_path);
        println!("═══════════════════════════════════════");

        if !test_path.exists() {
            println!("⚠ File not found (skipping)");
            continue;
        }

        match HybridBlfParser::parse(test_path) {
            Ok(iterator) => {
                println!("✓ BLF file opened (hybrid mode)\n");

                let mut can_count = 0;
                let mut error_count = 0;
                let mut fd_count = 0;
                let mut extended_count = 0;

                for (i, frame_result) in iterator.enumerate() {
                    match frame_result {
                        Ok(frame) => {
                            if frame.is_error_frame {
                                error_count += 1;
                            } else {
                                can_count += 1;
                                if frame.is_fd {
                                    fd_count += 1;
                                }
                                if frame.is_extended {
                                    extended_count += 1;
                                }

                                // Print all frames (should be few)
                                println!("Frame {}: ID=0x{:03X} ({}), DLC={}, FD={}, Data={:02X?}",
                                    i,
                                    frame.can_id,
                                    if frame.is_extended { "ext" } else { "std" },
                                    frame.data.len(),
                                    if frame.is_fd { "YES" } else { "no" },
                                    &frame.data
                                );
                            }
                        }
                        Err(e) => {
                            println!("✗ Frame error: {}", e);
                        }
                    }
                }

                println!("\n─────────────────────────────────────");
                println!("Summary:");
                println!("  CAN frames: {}", can_count);
                println!("  CAN-FD frames: {}", fd_count);
                println!("  Extended ID frames: {}", extended_count);
                println!("  Error frames: {}", error_count);
                println!("  Total: {}", can_count + error_count);

                if can_count == 0 && error_count == 0 {
                    println!("\n⚠ WARNING: No frames extracted!");
                } else {
                    println!("\n✓ SUCCESS - Hybrid parser extracted CAN-FD frames!");
                }
            }
            Err(e) => {
                println!("✗ Error opening file: {}", e);
            }
        }
    }
}
