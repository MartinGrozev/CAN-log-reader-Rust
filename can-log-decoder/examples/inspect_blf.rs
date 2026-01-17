//! Inspect raw BLF file to see what object types it contains

use ablf::BlfFile;
use std::collections::HashMap;
use std::fs::File;
use std::io::BufReader;
use std::path::Path;

fn main() {
    let test_files = vec![
        Path::new("../arxml/test_CanFdMessage.blf"),
        Path::new("../arxml/test_CanFdMessage64.blf"),
    ];

    for test_path in test_files {
        println!("\n═══════════════════════════════════════");
        println!("Inspecting: {:?}", test_path);
        println!("═══════════════════════════════════════");

        if !test_path.exists() {
            println!("⚠ File not found (skipping)");
            continue;
        }

        let file = File::open(test_path).unwrap();
        let reader = BufReader::new(file);
        let blf = match BlfFile::from_reader(reader) {
            Ok(b) => b,
            Err((e, _)) => {
                println!("✗ Error: {}", e);
                continue;
            }
        };

        if !blf.is_valid() {
            println!("✗ Invalid BLF file");
            continue;
        }

        println!("✓ Valid BLF file\n");

        let mut type_counts: HashMap<u32, usize> = HashMap::new();
        let mut total = 0;

        for obj in blf {
            *type_counts.entry(obj.object_type).or_insert(0) += 1;
            total += 1;
        }

        println!("Object Type Statistics:");
        println!("─────────────────────────");
        let mut types: Vec<_> = type_counts.iter().collect();
        types.sort_by_key(|(t, _)| **t);

        for (obj_type, count) in types {
            let type_name = match *obj_type {
                10 => "LogContainer",
                65 => "AppText",
                73 => "CanErrorFrameExt",
                86 => "CanMessage2 (CAN/CAN-FD)",
                100 => "CAN-FD Message (⚠ not supported by ablf)",
                115 => "Reserved/Unknown",
                _ => "Other",
            };
            println!("  Type {:3}: {:4} objects  ({})", obj_type, count, type_name);
        }

        println!("\nTotal objects: {}", total);
    }
}
