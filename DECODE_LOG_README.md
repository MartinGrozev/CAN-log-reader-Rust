# CAN Log Decoder Tool - User Guide

## Overview

`decode_log.exe` is a standalone tool for decoding CAN log files (BLF/MF4) using signal definitions from DBC and ARXML files.

**Location**: `target/release/examples/decode_log.exe` (3.0 MB)

## What You'll See

The decoder will show you:

1. **Decoded Messages** - CAN messages with signal names and engineering values
2. **Container PDUs** - AUTOSAR containers unpacked into contained PDUs
3. **Signals** - Individual signal values with units (temperature, voltage, current, etc.)
4. **Statistics** - Summary of what was decoded

## Usage

```bash
decode_log.exe <log_file> [OPTIONS]
```

### Required

- `<log_file>` - Path to BLF or MF4 file

### Options

- `--dbc <file.dbc>` - Load DBC file (can specify multiple times)
- `--arxml <file.arxml>` - Load ARXML file (can specify multiple times)
- `--limit <count>` - Limit output to first N events
- `--verbose` or `-v` - Show detailed signal values
- No options - Shows all events without signal details

## Examples

### Basic Usage (No Signal Definitions)
```bash
decode_log.exe trace.blf --limit 100
```
**Output**: Raw CAN frames with IDs and byte counts

### With DBC File
```bash
decode_log.exe trace.blf --dbc powertrain.dbc --limit 50
```
**Output**: Decoded messages with signal names and values

### With ARXML (Container Support)
```bash
decode_log.exe trace.blf --arxml system.arxml --verbose --limit 100
```
**Output**: Container PDUs unpacked + signals decoded

### Multiple Definition Files
```bash
decode_log.exe trace.blf --dbc powertrain.dbc --dbc diagnostics.dbc --arxml system.arxml --limit 200
```
**Output**: Full decoding with all available definitions

### Full Verbose Output
```bash
decode_log.exe trace.blf --dbc powertrain.dbc --arxml system.arxml --verbose > output.txt
```
**Output**: All events with signal details to file

## Sample Output

### Without Verbose Mode
```
=== CAN Log Decoder ===
Log file: "trace.blf"
DBC files: 1 loaded
ARXML files: 1 loaded
Limit: 100 events

=== SIGNAL DATABASE ===
Messages: 25
Signals: 150
Containers: 2

=== DECODING LOG FILE ===

[0.000100s] CH0 0x123 EngineStatus
[0.000250s] CH0 0x456 BatteryData (MUX=0)
[0.000500s] CONTAINER 0x100 MainContainer (Static) - 3 PDUs
[0.000750s] CH0 0x789 VehicleSpeed

=== DECODING SUMMARY ===
Total frames processed: 100
Decoded messages: 85
Container PDUs: 2
Contained PDUs extracted: 6
Total signals decoded: 425
Unique CAN IDs seen: 15
Unique message names: 12

Top 10 Most Frequent Messages:
  EngineStatus: 25 times
  BatteryData: 20 times
  VehicleSpeed: 15 times
```

### With Verbose Mode
```
[0.000100s] CH0 0x123 EngineStatus
    RPM: 2500.00rpm
    Temperature: 85.50°C
    Throttle: 45.20%
    Status: "Running"
    ErrorCode: 0

[0.000500s] CONTAINER 0x100 MainContainer (Static) - 3 PDUs
    └─ PDU: BatteryPDU (ID: 1, 8 bytes)
    └─ PDU: TempPDU (ID: 2, 6 bytes)
    └─ PDU: StatusPDU (ID: 3, 4 bytes)

[0.000501s] CH0 0x0 BatteryPDU
    Voltage: 400.00V
    Current: 42.50A
    SOC: 75.00%

[0.000502s] CH0 0x0 TempPDU
    BatteryTemp: 45.00°C
    InletTemp: 38.50°C
```

## What Gets Decoded

### 1. Regular CAN Messages
- **With DBC/ARXML**: Shows message name + signal values
- **Without definitions**: Shows as RAW frame

### 2. AUTOSAR Container PDUs (requires ARXML)
- **Container event**: Shows container type and number of PDUs
- **Contained PDU events**: Shows each extracted PDU
- **Signal events**: Decodes signals from each contained PDU

### 3. Signal Values
- **Engineering units**: °C, V, A, %, rpm, km/h, etc.
- **Value descriptions**: "Running", "Charging", "Error", etc.
- **Multiplexed signals**: Shows multiplexer value (MUX=X)

## Troubleshooting

### "No signal definitions loaded!"
- You didn't provide any DBC or ARXML files
- All frames will show as RAW (undecoded)
- **Fix**: Add `--dbc` or `--arxml` arguments

### "0 messages, 0 signals"
- DBC/ARXML files loaded but didn't match log data
- Check that definition files correspond to the ECU/system in the log
- **Note**: This is OK - decoder will still show raw frames

### No Container PDUs shown
- Log doesn't contain AUTOSAR containers, OR
- ARXML file doesn't define containers for the CAN IDs in the log
- **Note**: Most systems don't use containers - this is normal

### Large files take a long time
- Use `--limit 1000` to process only first 1000 frames
- Consider filtering to specific time range in CANalyzer first
- Decoding speed: ~10,000-50,000 frames/second (depends on complexity)

## Performance Tips

1. **Use --limit** for quick inspection: `--limit 100`
2. **Skip verbose** for large files: omit `-v` flag
3. **Test with small sample** first: process 1-minute excerpt
4. **Redirect output** for full analysis: `> output.txt`

## File Compatibility

### Supported Log Formats
- ✅ **BLF** (Binary Log Format) - Vector CANalyzer/CANoe
  - Type 86 (CanMessage2) - CAN 2.0 & CAN-FD
  - Type 100/101 (CAN-FD messages)
  - Type 73 (Error frames)

- ✅ **MF4** (MDF4 format) - ASAM standard
  - Via mdflib C++ library
  - CAN channel data

### Supported Definition Formats
- ✅ **DBC** - CAN database (Vector, open standard)
- ✅ **ARXML** - AUTOSAR XML (all 4.x versions)
  - I-SIGNAL-I-PDU (regular messages)
  - MULTIPLEXED-I-PDU (multiplexed messages)
  - CONTAINER-I-PDU (Static/Dynamic/Queued containers)

## Real-World Usage

### Scenario 1: Quick Inspection
**Goal**: See what's in the log file

```bash
decode_log.exe your_log.blf --limit 50
```

### Scenario 2: Check Specific Message
**Goal**: Find if "BatteryStatus" message is present

```bash
decode_log.exe trace.blf --dbc battery.dbc --verbose | findstr "BatteryStatus"
```

### Scenario 3: Container Analysis
**Goal**: See AUTOSAR container unpacking

```bash
decode_log.exe trace.blf --arxml system.arxml --verbose --limit 500 > containers.txt
```

### Scenario 4: Full Analysis
**Goal**: Decode entire log with all available definitions

```bash
decode_log.exe full_trace.blf --dbc powertrain.dbc --dbc body.dbc --arxml system.arxml > decoded.txt
```

## Expected Results with Your Files

### With BLF + ARXML + DBC:
```
✅ Should see:
- Database: X messages, Y signals, Z containers loaded
- Decoded messages with signal names
- Container PDUs unpacked (if present in log)
- Signals from contained PDUs
- Statistics summary

⚠ May see:
- Some RAW frames (unknown CAN IDs)
- "No signal definition found" warnings (normal for unmapped PDUs)
```

### With BLF only (no definitions):
```
✅ Should see:
- All frames as RAW
- CAN IDs and byte lengths
- Frame timestamps
- Statistics: X raw frames processed
```

## Next Steps

After running the decoder:

1. **Check statistics** - Verify expected number of messages
2. **Look for your signals** - Search for known signal names
3. **Verify containers** - If using AUTOSAR, check PDU unpacking
4. **Adjust limit** - Use `--limit` to focus on specific time range
5. **Try verbose** - Add `-v` to see signal values

## Technical Notes

- **Channel numbers**: Start at 0 (CH0, CH1, CH2...)
- **Timestamps**: Seconds since log start (e.g., [0.000100s])
- **CAN IDs**: Shown in hex (e.g., 0x123)
- **Multiplexed signals**: Shows active multiplexer value
- **Contained PDUs**: Channel=0 (no specific CAN channel)

## Support

If you encounter issues:
1. Check file paths are correct
2. Verify DBC/ARXML files match the ECU/system
3. Try with `--limit 10` first to test
4. Check that log file is valid (open in CANalyzer)

---

**Tool Version**: can-log-decoder v0.1.0
**Built**: January 2026
**Architecture**: Standalone executable (no DLL dependencies)
