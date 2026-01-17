//! Test BLF parser with real files

use can_log_decoder::formats::blf::BlfParser;
use std::path::Path;

fn main() {

    let test_files = vec![
        Path::new("../arxml/test_CanFdMessage.blf"),
        Path::new("../arxml/test_CanFdMessage64.blf"),
    ];

    for test_path in test_files {
        println!("\n═══════════════════════════════════════");
        println!("Testing: {:?}", test_path);
        println!("═══════════════════════════════════════");

        if !test_path.exists() {
            println!("⚠ File not found (skipping)");
            continue;
        }

        match BlfParser::parse(test_path) {
            Ok(mut iterator) => {
                println!("✓ BLF file opened and validated\n");

                let mut can_count = 0;
                let mut error_count = 0;
                let mut fd_count = 0;
                let mut extended_count = 0;
                let mut total_objects = 0;

                // Peek at raw objects first
                println!("Inspecting raw BLF objects...");
                for obj in &mut iterator.objects {
                    total_objects += 1;
                    println!("  Object type={}, size={}", obj.object_type, obj.object_size);
                    if total_objects >= 10 {
                        break;
                    }
                }
                println!("(Inspected {} objects)\n", total_objects);

                // Reset iterator by re-parsing
                iterator = BlfParser::parse(test_path).unwrap();

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

                                // Print first few frames
                                if i < 5 {
                                    println!("Frame {}: ID=0x{:03X} ({}), DLC={}, Data={:02X?}",
                                        i,
                                        frame.can_id,
                                        if frame.is_extended { "ext" } else { "std" },
                                        frame.data.len(),
                                        &frame.data
                                    );
                                }
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
                    println!("\n✓ SUCCESS!");
                }
            }
            Err(e) => {
                println!("✗ Error opening file: {}", e);
            }
        }
    }
}
