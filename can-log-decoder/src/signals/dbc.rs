//! DBC file parser
//!
//! Parses Vector DBC files and converts them into our internal signal database format.

use crate::signals::database::{
    ByteOrder, MessageDefinition, MultiplexerInfo, SignalDefinition, ValueType,
};
use crate::types::{DecoderError, Result};
use std::collections::HashMap;
use std::path::Path;

/// Parse a DBC file and return message definitions
pub fn parse_dbc_file(path: &Path) -> Result<Vec<MessageDefinition>> {
    log::info!("Parsing DBC file: {:?}", path);

    // Read the DBC file as bytes first (handle non-UTF8 encodings)
    let bytes = std::fs::read(path).map_err(|e| {
        DecoderError::DbcParseError(format!("Failed to read file {:?}: {}", path, e))
    })?;

    // Try UTF-8 first, then fallback to Latin-1/Windows-1252 encoding
    let dbc_content = String::from_utf8(bytes.clone())
        .or_else(|_| {
            // Try Latin-1 encoding (compatible with Windows-1252)
            log::warn!("DBC file is not UTF-8, trying Latin-1 encoding");
            Ok::<String, std::string::FromUtf8Error>(
                bytes.iter().map(|&b| b as char).collect()
            )
        })
        .map_err(|e| {
            DecoderError::DbcParseError(format!("Failed to decode file {:?}: {}", path, e))
        })?;

    // Parse using can-dbc crate
    let dbc = can_dbc::DBC::from_slice(dbc_content.as_bytes()).map_err(|e| {
        DecoderError::DbcParseError(format!("Failed to parse DBC file {:?}: {:?}", path, e))
    })?;

    let source_filename = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown.dbc")
        .to_string();

    // Convert to our internal format
    let mut messages = Vec::new();

    for dbc_msg in dbc.messages() {
        let message = convert_message(dbc_msg, &source_filename)?;
        messages.push(message);
    }

    log::info!(
        "Parsed {} messages from {:?}",
        messages.len(),
        path
    );

    Ok(messages)
}

/// Convert a can-dbc message to our MessageDefinition
fn convert_message(
    dbc_msg: &can_dbc::Message,
    source: &str,
) -> Result<MessageDefinition> {
    let mut signals = Vec::new();
    let mut is_multiplexed = false;
    let mut multiplexer_signal_name: Option<String> = None;

    // First pass: identify multiplexer signal
    for dbc_sig in dbc_msg.signals() {
        if let can_dbc::MultiplexIndicator::Multiplexor = dbc_sig.multiplexer_indicator() {
            is_multiplexed = true;
            multiplexer_signal_name = Some(dbc_sig.name().to_string());
            break;
        } else if matches!(
            dbc_sig.multiplexer_indicator(),
            can_dbc::MultiplexIndicator::MultiplexedSignal(_)
        ) {
            is_multiplexed = true;
        }
    }

    // Second pass: convert all signals
    for dbc_sig in dbc_msg.signals() {
        let signal = convert_signal(dbc_sig, multiplexer_signal_name.as_deref())?;
        signals.push(signal);
    }

    Ok(MessageDefinition {
        id: dbc_msg.message_id().0,  // Extract raw ID from MessageId tuple struct
        name: dbc_msg.message_name().to_string(),
        size: *dbc_msg.message_size() as usize,
        sender: match dbc_msg.transmitter() {
            can_dbc::Transmitter::NodeName(name) => Some(name.to_string()),
            _ => None,
        },
        signals,
        is_multiplexed,
        multiplexer_signal: multiplexer_signal_name,
        source: source.to_string(),
    })
}

/// Convert a can-dbc signal to our SignalDefinition
fn convert_signal(
    dbc_sig: &can_dbc::Signal,
    multiplexer_signal_name: Option<&str>,
) -> Result<SignalDefinition> {
    // Determine byte order
    let byte_order = match *dbc_sig.byte_order() {
        can_dbc::ByteOrder::LittleEndian => ByteOrder::LittleEndian,
        can_dbc::ByteOrder::BigEndian => ByteOrder::BigEndian,
    };

    // Determine value type
    let value_type = match *dbc_sig.value_type() {
        can_dbc::ValueType::Signed => ValueType::Signed,
        can_dbc::ValueType::Unsigned => ValueType::Unsigned,
    };

    // Extract value table if present
    let value_table = None;  // TODO: can-dbc v5.0 API for value descriptions needs investigation

    // Handle multiplexer information
    let multiplexer_info = match *dbc_sig.multiplexer_indicator() {
        can_dbc::MultiplexIndicator::MultiplexedSignal(switch_value) => {
            Some(MultiplexerInfo {
                multiplexer_signal: multiplexer_signal_name
                    .ok_or_else(|| {
                        DecoderError::InvalidSignalDefinition(format!(
                            "Multiplexed signal '{}' but no multiplexer found",
                            dbc_sig.name()
                        ))
                    })?
                    .to_string(),
                multiplexer_values: vec![switch_value as u64],  // switch_value is already u64
            })
        }
        _ => None,
    };

    Ok(SignalDefinition {
        name: dbc_sig.name().to_string(),
        start_bit: *dbc_sig.start_bit() as u16,
        length: *dbc_sig.signal_size() as u16,
        byte_order,
        value_type,
        factor: *dbc_sig.factor(),
        offset: *dbc_sig.offset(),
        min: *dbc_sig.min(),
        max: *dbc_sig.max(),
        unit: if dbc_sig.unit().is_empty() {
            None
        } else {
            Some(dbc_sig.unit().to_string())
        },
        value_table,
        multiplexer_info,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_parse_simple_dbc() {
        // Create a minimal DBC file for testing
        let dbc_content = r#"
VERSION ""

NS_ :
    NS_DESC_
    CM_
    BA_DEF_
    BA_
    VAL_
    CAT_DEF_
    CAT_
    FILTER
    BA_DEF_DEF_
    EV_DATA_
    ENVVAR_DATA_
    SGTYPE_
    SGTYPE_VAL_
    BA_DEF_SGTYPE_
    BA_SGTYPE_
    SIG_TYPE_REF_
    VAL_TABLE_
    SIG_GROUP_
    SIG_VALTYPE_
    SIGTYPE_VALTYPE_
    BO_TX_BU_
    BA_DEF_REL_
    BA_REL_
    BA_SGTYPE_REL_
    SG_MUL_VAL_

BS_:

BU_: ECU1 ECU2

BO_ 291 EngineData: 8 ECU1
 SG_ EngineSpeed : 0|16@1+ (1,0) [0|8000] "rpm" ECU2
 SG_ EngineTemp : 16|8@1+ (1,-40) [-40|215] "C" ECU2

BO_ 512 BatteryStatus: 8 ECU1
 SG_ BatteryVoltage : 0|16@1+ (0.01,0) [0|16] "V" ECU2
"#;

        // Write to temporary file
        let mut temp_file = NamedTempFile::new().unwrap();
        temp_file.write_all(dbc_content.as_bytes()).unwrap();
        temp_file.flush().unwrap();

        // Parse the file
        let messages = parse_dbc_file(temp_file.path()).unwrap();

        // Verify results
        assert_eq!(messages.len(), 2);

        // Check first message
        let msg1 = &messages[0];
        assert_eq!(msg1.id, 291);
        assert_eq!(msg1.name, "EngineData");
        assert_eq!(msg1.size, 8);
        assert_eq!(msg1.sender, Some("ECU1".to_string()));
        assert_eq!(msg1.signals.len(), 2);

        // Check first signal
        let sig1 = &msg1.signals[0];
        assert_eq!(sig1.name, "EngineSpeed");
        assert_eq!(sig1.start_bit, 0);
        assert_eq!(sig1.length, 16);
        assert_eq!(sig1.factor, 1.0);
        assert_eq!(sig1.offset, 0.0);
        assert_eq!(sig1.unit, Some("rpm".to_string()));
    }

    #[test]
    fn test_parse_multiplexed_signals() {
        let dbc_content = r#"
VERSION ""

NS_ :

BS_:

BU_: ECU1

BO_ 512 MultiplexedMsg: 8 ECU1
 SG_ Mode M : 0|8@1+ (1,0) [0|3] "" ECU1
 SG_ SignalA m0 : 8|16@1+ (1,0) [0|100] "%" ECU1
 SG_ SignalB m1 : 8|16@1+ (0.1,0) [0|1000] "mV" ECU1
"#;

        let mut temp_file = NamedTempFile::new().unwrap();
        temp_file.write_all(dbc_content.as_bytes()).unwrap();
        temp_file.flush().unwrap();

        let messages = parse_dbc_file(temp_file.path()).unwrap();

        assert_eq!(messages.len(), 1);
        let msg = &messages[0];
        assert!(msg.is_multiplexed);
        assert_eq!(msg.multiplexer_signal, Some("Mode".to_string()));
        assert_eq!(msg.signals.len(), 3);

        // Check multiplexed signals
        let sig_a = msg.signals.iter().find(|s| s.name == "SignalA").unwrap();
        assert!(sig_a.multiplexer_info.is_some());
        assert_eq!(
            sig_a.multiplexer_info.as_ref().unwrap().multiplexer_signal,
            "Mode"
        );
    }
}
