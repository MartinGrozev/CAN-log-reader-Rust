# CAN Log Decoder Library

A stateless, reusable Rust library for decoding CAN log files (BLF, MF4) with signal definitions from DBC and ARXML files.

## Design Philosophy

This library is intentionally **minimal and focused**:

### What it DOES:
- ✅ Parse log files (BLF, MF4)
- ✅ Decode CAN messages using DBC/ARXML definitions
- ✅ Handle multiplexed signals
- ✅ Reconstruct CAN-TP (ISO-TP) multi-frame messages
- ✅ Unpack AUTOSAR container PDUs
- ✅ Emit a stream of decoded events

### What it DOES NOT do:
- ❌ Track signal value changes (old→new)
- ❌ Evaluate events or conditions
- ❌ Execute callbacks
- ❌ Generate reports
- ❌ Apply business logic

**Why?** This separation allows the decoder to be reused in different contexts (CLI tools, GUI applications, web services, embedded systems, etc.) without carrying unnecessary baggage.

## API Example

```rust
use can_log_decoder::{Decoder, DecoderConfig};
use std::path::Path;

// Create decoder and load signal definitions
let mut decoder = Decoder::new();
decoder.add_dbc(Path::new("powertrain.dbc"))?;
decoder.add_dbc(Path::new("diagnostics.dbc"))?;

// Configure decoder
let config = DecoderConfig::new()
    .with_signal_decoding(true)
    .add_cantp_pair(0x7E0, 0x7E8)
    .with_channel_filter(vec![0, 1]);

// Decode log file - returns iterator
let events = decoder.decode_file(Path::new("trace.blf"), config)?;

for event in events {
    match event? {
        DecodedEvent::Message { timestamp, can_id, signals, .. } => {
            println!("Message 0x{:X} at {:?}", can_id, timestamp);
            for signal in signals {
                println!("  {} = {}", signal.name, signal.value);
            }
        }
        DecodedEvent::CanTpMessage { payload, .. } => {
            println!("CAN-TP message: {} bytes", payload.len());
        }
        _ => {}
    }
}
```

## Core Types

### `DecodedEvent`
The primary output of the decoder:
- `Message` - A decoded CAN message with all signals
- `CanTpMessage` - A reconstructed CAN-TP message
- `ContainerPdu` - An unpacked AUTOSAR container PDU
- `RawFrame` - Optional raw CAN frame

### `SignalValue`
Signal values with type safety:
- `Integer(i64)` - Signed integers
- `Float(f64)` - Floating-point values
- `Boolean(bool)` - Boolean values

Includes conversion methods (`as_f64()`, `as_i64()`, `as_bool()`)

### `DecoderConfig`
Minimal configuration for the decoder:
- Signal decoding on/off
- CAN-TP pairs
- Container PDU IDs
- Optional channel/message filters

## Architecture

### Module Structure
```
can-log-decoder/
├── types.rs        - Core types (DecodedEvent, SignalValue, etc.)
├── config.rs       - Decoder configuration
├── decoder.rs      - Main API
├── formats/        - BLF/MF4 parsers (Phase 3)
├── signals/        - DBC/ARXML parsers (Phase 2)
├── cantp/          - CAN-TP reconstruction (Phase 5)
└── container/      - Container PDU support (Phase 6)
```

### Iterator-Based Design
The decoder emits events lazily via iterators, allowing:
- **Memory efficiency** - Process large files without loading everything
- **Early termination** - Stop processing when you find what you need
- **Composability** - Chain with standard Rust iterator methods

## Status

**Phase 1: Complete** ✅
- Core types defined
- Public API designed
- Module structure in place
- Comprehensive documentation
- Unit tests

**Phase 2-6: In Progress**
- DBC parser
- ARXML parser
- BLF/MF4 parsers
- CAN-TP reconstruction
- Container PDU support

## License

MIT License (see workspace LICENSE file)

## Contributing

This library is part of the CAN Log Reader project. See the main project README for contribution guidelines.
