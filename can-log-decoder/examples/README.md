# BLF/MF4 Analysis Examples

This directory contains tools to analyze and test BLF and MF4 log files.

## Quick Start

### Analyze Your BLF File

```bash
cargo run --release --example analyze_blf -- path/to/your/file.blf
```

This will show:
- Object type statistics
- Whether compression is used
- What CAN message types are present
- Recommendations for parsing

### Inspect BLF Structure

```bash
cargo run --release --example inspect_blf
```

Inspects the test BLF files in `arxml/` directory and shows object type distribution.

## Current BLF Support Status

### ✅ Fully Supported
- **Type 86 (CanMessage2)**: Standard CAN and CAN-FD messages
- **Type 73 (CanErrorFrameExt)**: CAN error frames
- **Type 10 (LogContainer)**: Automatic decompression

### ⚠️ Partially Supported
- **Type 100 (CAN_FD_MESSAGE)**: Not yet implemented
- **Type 101 (CAN_FD_MESSAGE_64)**: Not yet implemented

## How to Test with Your Files

1. **Run the analyzer first:**
   ```bash
   cargo run --release --example analyze_blf -- /path/to/your/real.blf
   ```

2. **Check the output:**
   - If you see "Type 86" messages → File is ready to use!
   - If you see "Type 100/101" messages → Needs additional work

3. **Test extraction:**
   ```bash
   # For Type 86 files (should work):
   cargo run --release --example test_blf

   # For Type 100/101 files (experimental):
   cargo run --release --example test_hybrid_blf
   ```

## Example Output

```
╔══════════════════════════════════════════════════════════════╗
║          BLF FILE STRUCTURE ANALYZER                         ║
╚══════════════════════════════════════════════════════════════╝

File: "my_real_log.blf"
────────────────────────────────────────────────────────────────

📊 File Size: 15.3 MB (16,048,576 bytes)
✅ Valid BLF file format

═══════════════════════════════════════════════════════════════
                  OBJECT TYPE STATISTICS
═══════════════════════════════════════════════════════════════

Type            Count        Total Size      Description
─────────────────────────────────────────────────────────────
86              12,456       14.2 MB         CanMessage2 (✅ SUPPORTED)
10              8            1.1 MB          LogContainer (⚠️  COMPRESSED)

Total Objects: 12,464

═══════════════════════════════════════════════════════════════
                     RECOMMENDATIONS
═══════════════════════════════════════════════════════════════

✅ READY TO USE
   Use: BlfParser (standard parser)
   This file is fully supported with type 86 messages.
```

## Reporting Issues

If you encounter problems:
1. Run `analyze_blf` on your file
2. Save the output
3. Create a GitHub issue with the analysis results

## Next Steps

- See [CLAUDE.md](../CLAUDE.md) for development progress
- See [Specification.txt](../Specification.txt) for full project spec
