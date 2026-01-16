# CAN Log Reader (Rust)

A high-performance CAN log reader/decoder written in Rust. Decodes offline BLF/MF4 files using signal definitions from DBC and ARXML files, reconstructs CAN-TP messages, and generates detailed analysis reports.

**Status:** 🚧 Active Development - Phases 1-4 Complete

## Features

### ✅ Implemented (Ready to Use)
- **Phase 1:** Core architecture and types
- **Phase 2:** Signal database parsers (DBC + ARXML with AUTOSAR 4.x support)
  - Full DBC parsing with multiplexed signals
  - Complete ARXML parsing using `autosar-data` crate
  - Physical value conversion (factor, offset, units)
  - Optimized PDU-to-CAN-ID lookup (O(1) HashMap)
  - SYSTEM-SIGNAL-REF parsing for engineering values
- **Phase 3:** Log file format parsers (BLF/MF4 stubs ready)
- **Phase 4:** Message decoding engine
  - Bit extraction (little-endian & big-endian)
  - Physical value conversion
  - Multiplexed signal decoding
  - Sign extension for signed values

### 🚧 Coming Soon
- **Phase 5:** CAN-TP (ISO-TP) multi-frame reconstruction
- **Phase 6:** AUTOSAR Container PDU support
- **Phase 7-12:** Event tracking, expressions, callbacks, reports
- **Phase 13:** Multi-file parallel processing

## Quick Start

### Prerequisites
- Windows (tested on Win10/Win11)
- No Rust installation needed - use pre-built binary!

### Installation

1. **Clone the repository:**
   ```bash
   git clone https://github.com/MartinGrozev/CAN-log-reader-Rust.git
   cd CAN-log-reader-Rust
   ```

2. **Use the pre-built binary:**
   ```bash
   # Binary is located in: target/release/can-log-cli.exe
   ```

3. **Or compile from source** (requires Rust toolchain + vcpkg):
   ```bash
   cargo build --release
   ```

### Basic Usage

#### Test signal database loading:
```bash
can-log-cli.exe --dbc signals.dbc
can-log-cli.exe --arxml system.arxml
```

#### Decode a log file (when BLF parser is integrated):
```bash
can-log-cli.exe --log trace.blf --dbc powertrain.dbc
can-log-cli.exe --log trace.blf --arxml system.arxml --output decoded.txt
```

#### Use multiple signal files:
```bash
can-log-cli.exe --log trace.blf --dbc powertrain.dbc --dbc diagnostics.dbc --arxml system.arxml
```

### Command Line Options

```
can-log-cli.exe [OPTIONS]

Options:
  -l, --log <FILE>          BLF/MF4 log file to decode
      --dbc <FILE>          DBC file(s) - can be repeated
      --arxml <FILE>        ARXML file(s) - can be repeated
  -o, --output <FILE>       Output file (default: stdout)
  -c, --config <FILE>       Config file for advanced features
      --max-frames <COUNT>  Limit frames (for testing)
  -v, --verbose             Increase verbosity (-v, -vv, -vvv)
  -q, --quiet               Suppress all output except errors
  -h, --help                Show help
  -V, --version             Show version
```

## Example Output

```
═══════════════════════════════════════════════
  CAN Log Decoder - Simple Mode
═══════════════════════════════════════════════

Loading DBC: "powertrain.dbc" ... ✓
Loading ARXML: "system.arxml" ... ✓

📊 Signal Database:
  Messages: 145
  Signals:  782
  Containers: 3

✓ Signal database loaded successfully!
  Add --log <file.blf> to decode CAN frames
```

## Architecture

### Project Structure
```
CAN-log-reader-Rust/
├── can-log-decoder/     # Core decoder library (stateless)
│   ├── src/
│   │   ├── signals/     # DBC/ARXML parsers
│   │   ├── formats/     # BLF/MF4 parsers
│   │   ├── message_decoder.rs  # Signal extraction engine
│   │   └── types.rs     # Core types
│   └── Cargo.toml
├── can-log-cli/         # CLI application
│   ├── src/
│   │   └── main.rs      # Command-line interface
│   └── Cargo.toml
├── can-log-api/         # C FFI header (future)
└── target/release/
    └── can-log-cli.exe  # Pre-built binary ⭐
```

### Key Design Principles
- **Library/Application Separation:** Reusable decoder core
- **Stateless Decoding:** Pure signal extraction, no state tracking
- **Configurability:** Everything driven by signal definitions
- **Performance:** Optimized algorithms (O(1) lookups, parallel processing)
- **Extensibility:** C callbacks for custom logic (coming soon)

## Technical Details

### Supported Formats

#### Input Formats
- **DBC:** Vector CAN Database files
  - Multiplexed signals ✅
  - Value tables ✅
  - Signed/unsigned signals ✅
- **ARXML:** AUTOSAR XML (all 4.x versions)
  - I-SIGNAL-I-PDU ✅
  - MULTIPLEXED-I-PDU ✅
  - CONTAINER-I-PDU ✅
  - SYSTEM-SIGNAL-REF with COMPU-METHOD ✅
- **BLF:** Vector Binary Log Format (stub, integration pending)
- **MF4:** ASAM MDF4 files (stub with mdflib FFI)

#### Output Formats
- Console/Text (current)
- HTML reports (Phase 12)
- CSV export (planned)

### Signal Decoding

**Bit Extraction:**
- Little-endian (Intel): LSB-first, grows upward
- Big-endian (Motorola): MSB-first, grows downward
- Cross-byte signals supported
- Handles 1-64 bit signals

**Physical Value Conversion:**
```
physical_value = offset + factor * raw_value
```
Example: `BatterySOC = 0.0 + 0.5 * 150 = 75.0%`

**Multiplexed Signals:**
- Extracts multiplexer signal first
- Filters signals based on active multiplexer value
- Supports multiple multiplexer modes per message

### Performance Optimizations

**ARXML Parser:**
- PDU-to-CAN-ID HashMap: O(1) lookups instead of O(n) DFS
- For 1000 PDUs: ~1M operations → ~1K (1000x faster!)

**Message Decoder:**
- Efficient bit manipulation algorithms
- Minimal allocations
- Parallel processing ready (Phase 13)

## Development Status

### Completed Phases ✅
1. ✅ **Phase 1:** Project setup (Session 1)
2. ✅ **Phase 2:** Signal parsers (Sessions 2-3, optimized in Session 6)
3. ✅ **Phase 3:** Log format parsers - stubs (Session 4-5)
4. ✅ **Phase 4:** Message decoder (Session 6)

### Current Limitations ⚠️
- BLF parser uses stub (needs ablf crate integration)
- MF4 parser uses stub (mdflib C++ FFI ready but not integrated)
- No CAN-TP reconstruction yet (Phase 5)
- No event tracking yet (Phase 10)
- No report generation yet (Phase 12)

**But you can already:**
- ✅ Load and parse DBC files
- ✅ Load and parse ARXML files
- ✅ View signal database statistics
- ✅ Test signal definitions are correct

**Next step:** Integrate BLF parser → decode real CAN frames!

## Testing with Real Data

### Recommended Workflow

1. **On your development PC:**
   - Pull latest code from GitHub
   - Use pre-built `can-log-cli.exe`

2. **Test with a small trace:**
   ```bash
   # Even 1 second of CAN data (50-200 frames) is enough!
   can-log-cli.exe --log short_trace.blf --dbc signals.dbc
   ```

3. **Verify:**
   - ✅ Signals decode correctly
   - ✅ Physical values match expectations (km/h, °C, %)
   - ✅ Multiplexed signals work
   - ✅ Units display properly

### What to Test

**Critical:**
- Signal extraction (are bit positions correct?)
- Physical values (is 0x96 → 75.0% correct?)
- Endianness (big-endian signals decode right?)
- Multiplexing (right signals active per mode?)

**Nice to have:**
- Performance (how fast on large files?)
- Memory usage (reasonable for large traces?)

## Dependencies

**Core:**
- `can-dbc` - DBC parser
- `autosar-data` - ARXML parser (AUTOSAR 4.x support)
- `ablf` - BLF parser (integration pending)

**Build:**
- `cmake` - For mdflib (MF4 support)
- `cc` - C++ compiler integration
- `vcpkg` - ZLIB/EXPAT for mdflib

**CLI:**
- `clap` - Command-line argument parsing
- `anyhow` - Error handling
- `env_logger` - Logging

## Contributing

This is a personal project for automotive CAN analysis. Contributions welcome!

### Reporting Issues

**From company workstation (no sensitive data sharing):**
- ❌ Don't share company CAN data, DBC files, or ARXML files
- ✅ Do share: "Signal X is always zero" or "Big-endian broken"
- ✅ Do share: Error messages, stack traces, CLI output

### Building from Source

**Requirements:**
- Rust 1.70+ (`rustup` toolchain)
- vcpkg (for mdflib dependencies)
- CMake 3.15+
- MSVC build tools (Windows)

**Build steps:**
```bash
# Install vcpkg dependencies
vcpkg install zlib:x64-windows-static expat:x64-windows-static

# Build release
cargo build --release

# Run tests (note: some tests fail due to known runtime issues)
cargo test --lib
```

## License

MIT License - See LICENSE file

## References

- [AUTOSAR Specification](https://www.autosar.org/)
- [Vector DBC Format](https://www.vector.com/)
- [ISO 11898 (CAN)](https://www.iso.org/standard/63648.html)
- [ISO-TP (ISO 15765-2)](https://www.iso.org/standard/66574.html)

## Changelog

### v0.1.0 (2026-01-16) - Phases 1-4 Complete
- ✅ Complete ARXML parser with physical value support
- ✅ Optimized PDU-to-CAN-ID lookup (1000x faster)
- ✅ Full message decoding engine
- ✅ CLI with DBC/ARXML loading
- ✅ BLF/MF4 parser stubs ready

---

**Built with ❤️ in Rust**
