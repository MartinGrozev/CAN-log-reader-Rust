//! Comprehensive BLF file analyzer
//!
//! This tool analyzes BLF files to help understand their structure and determine
//! what parsing strategy is needed. Run this on your real BLF files to see:
//! - Object type distribution
//! - Whether files use compression (LogContainer type 10)
//! - What CAN message types are present
//! - File structure details

use ablf::BlfFile;
use std::collections::HashMap;
use std::env;
use std::fs::File;
use std::io::BufReader;
use std::path::Path;

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        println!("Usage: {} <blf_file_path>", args[0]);
        println!("\nExample:");
        println!("  {} path/to/your/logfile.blf", args[0]);
        println!("\nThis will analyze the BLF file structure and show:");
        println!("  - Object type statistics");
        println!("  - Compression status");
        println!("  - Recommended parsing strategy");
        return;
    }

    let file_path = Path::new(&args[1]);

    println!("\n╔══════════════════════════════════════════════════════════════╗");
    println!("║          BLF FILE STRUCTURE ANALYZER                         ║");
    println!("╚══════════════════════════════════════════════════════════════╝");
    println!("\nFile: {:?}", file_path);
    println!("────────────────────────────────────────────────────────────────\n");

    if !file_path.exists() {
        println!("❌ ERROR: File not found!");
        return;
    }

    // Get file size
    let file_size = match std::fs::metadata(file_path) {
        Ok(meta) => meta.len(),
        Err(e) => {
            println!("❌ ERROR: Cannot read file metadata: {}", e);
            return;
        }
    };

    println!("📊 File Size: {:.2} KB ({} bytes)", file_size as f64 / 1024.0, file_size);

    // Open and parse BLF file
    let file = match File::open(file_path) {
        Ok(f) => f,
        Err(e) => {
            println!("❌ ERROR: Cannot open file: {}", e);
            return;
        }
    };

    let reader = BufReader::new(file);
    let blf = match BlfFile::from_reader(reader) {
        Ok(b) => b,
        Err((e, _)) => {
            println!("❌ ERROR: Cannot parse BLF file: {}", e);
            println!("\nThis file may be:");
            println!("  - Corrupted");
            println!("  - Not a valid BLF file");
            println!("  - Using an unsupported BLF version");
            return;
        }
    };

    if !blf.is_valid() {
        println!("⚠️  WARNING: BLF file validation failed");
        println!("   Continuing analysis anyway...\n");
    } else {
        println!("✅ Valid BLF file format\n");
    }

    // Analyze objects
    let mut type_counts: HashMap<u32, usize> = HashMap::new();
    let mut type_sizes: HashMap<u32, usize> = HashMap::new();
    let mut total_objects = 0;
    let mut has_compression = false;
    let mut has_can_messages = false;
    let mut has_canfd_100 = false;
    let mut has_canfd_101 = false;

    println!("🔍 Analyzing objects...\n");

    for obj in blf {
        *type_counts.entry(obj.object_type).or_insert(0) += 1;
        *type_sizes.entry(obj.object_type).or_insert(0) += obj.object_size as usize;
        total_objects += 1;

        // Check for specific types
        match obj.object_type {
            10 => has_compression = true,
            86 => has_can_messages = true,
            100 => has_canfd_100 = true,
            101 => has_canfd_101 = true,
            _ => {}
        }

        // Show first few objects in detail
        if total_objects <= 5 {
            println!("  Object #{}: Type {}, Size {} bytes",
                total_objects, obj.object_type, obj.object_size);
        }
    }

    if total_objects > 5 {
        println!("  ... ({} more objects)\n", total_objects - 5);
    } else {
        println!();
    }

    // Sort types by count
    let mut types: Vec<_> = type_counts.iter().collect();
    types.sort_by_key(|(_, count)| std::cmp::Reverse(**count));

    println!("═══════════════════════════════════════════════════════════════");
    println!("                  OBJECT TYPE STATISTICS");
    println!("═══════════════════════════════════════════════════════════════\n");

    println!("{:<15} {:<12} {:<15} {}", "Type", "Count", "Total Size", "Description");
    println!("─────────────────────────────────────────────────────────────");

    for (obj_type, count) in types {
        let total_size = type_sizes.get(obj_type).unwrap_or(&0);
        let type_name = match *obj_type {
            10 => "LogContainer (⚠️  COMPRESSED)",
            65 => "AppText",
            73 => "CanErrorFrameExt",
            86 => "CanMessage2 (✅ SUPPORTED)",
            100 => "CAN_FD_MESSAGE (⚠️  NEEDS WORK)",
            101 => "CAN_FD_MESSAGE_64 (⚠️  NEEDS WORK)",
            115 => "Reserved/Unknown",
            _ => "Other",
        };

        println!("{:<15} {:<12} {:<15} {}",
            format!("{}", obj_type),
            count,
            format!("{:.1} KB", *total_size as f64 / 1024.0),
            type_name
        );
    }

    println!("\nTotal Objects: {}", total_objects);

    // Analysis and recommendations
    println!("\n═══════════════════════════════════════════════════════════════");
    println!("                     ANALYSIS RESULTS");
    println!("═══════════════════════════════════════════════════════════════\n");

    if has_compression {
        println!("🗜️  COMPRESSION DETECTED");
        println!("   This file uses LogContainer (type 10) compression.");
        println!("   CAN messages are stored inside compressed containers.");
        println!("   The ablf crate will automatically decompress these.\n");
    }

    if has_can_messages {
        println!("✅ CAN MESSAGES FOUND (Type 86)");
        println!("   This file contains standard CAN/CAN-FD messages.");
        println!("   These are FULLY SUPPORTED by the current parser.\n");
    }

    if has_canfd_100 || has_canfd_101 {
        println!("⚠️  CAN-FD MESSAGES DETECTED");
        if has_canfd_100 {
            println!("   - Type 100 (CAN_FD_MESSAGE) found");
        }
        if has_canfd_101 {
            println!("   - Type 101 (CAN_FD_MESSAGE_64) found");
        }
        println!("   These types require additional parser support.");
        println!("   See recommendations below.\n");
    }

    if !has_can_messages && !has_canfd_100 && !has_canfd_101 {
        println!("⚠️  NO CAN MESSAGE OBJECTS DETECTED");
        println!("   This file may contain:");
        println!("   - Only diagnostic/metadata");
        println!("   - CAN messages in unsupported formats");
        println!("   - Compressed data that needs decompression\n");
    }

    // Recommendations
    println!("═══════════════════════════════════════════════════════════════");
    println!("                     RECOMMENDATIONS");
    println!("═══════════════════════════════════════════════════════════════\n");

    if has_can_messages && !has_canfd_100 && !has_canfd_101 {
        println!("✅ READY TO USE");
        println!("   Use: BlfParser (standard parser)");
        println!("   This file is fully supported with type 86 messages.\n");
    } else if has_canfd_100 || has_canfd_101 {
        println!("⚠️  PARTIAL SUPPORT");
        println!("   Current Status:");
        println!("   - Type 86 messages: ✅ Fully supported");
        println!("   - Type 100/101 messages: ❌ Not yet supported");
        println!("\n   Options:");
        println!("   1. Export logs with type 86 format (CANoe settings)");
        println!("   2. Wait for type 100/101 parser implementation");
        println!("   3. Use python-can as intermediate converter\n");
    }

    if has_compression && (has_canfd_100 || has_canfd_101) {
        println!("📝 TECHNICAL NOTE:");
        println!("   Type 100/101 messages are inside compressed containers.");
        println!("   Parser needs to:");
        println!("   1. Decompress LogContainer (type 10) - ablf handles this");
        println!("   2. Parse inner type 100/101 objects - needs implementation\n");
    }

    println!("═══════════════════════════════════════════════════════════════\n");
    println!("For questions or issues, please create a GitHub issue with");
    println!("the output of this analysis.\n");
}
