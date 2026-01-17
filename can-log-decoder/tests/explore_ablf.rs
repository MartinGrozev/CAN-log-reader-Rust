// Quick exploration of ablf crate API
use std::fs::File;
use std::io::BufReader;

#[test]
fn explore_ablf_api() {
    // Try to open a BLF file using ablf crate
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent().unwrap()
        .join("arxml/test_CanFdMessage.blf");

    if !path.exists() {
        println!("Test file not found: {:?}", path);
        return;
    }

    // Attempt 3: Use BufReader
    let file = File::open(&path).unwrap();
    let reader = BufReader::new(file);
    let blf = ablf::BlfFile::from_reader(reader).unwrap();

    // Try to iterate - iterator returns Object directly
    let mut count = 0;
    for obj in blf {
        println!("\nObject {}: Type={}, Size={}", count, obj.object_type, obj.object_size);
        println!("  Data: {:?}", obj.data);

        // Just print everything for now to learn structure
        // Later we'll filter for CAN messages

        count += 1;
        if count > 20 {
            break; // Limit output
        }
    }

    println!("\n=== Summary ===");
    println!("Total objects: {}", count);
}
