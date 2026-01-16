//! ARXML (AUTOSAR XML) file parser using autosar-data crate
//!
//! Parses AUTOSAR ARXML files to extract signal and container PDU definitions.
//! Uses the autosar-data crate for robust AUTOSAR 4.x support.

use crate::signals::database::{
    ByteOrder, ContainerDefinition, ContainerLayout, MessageDefinition,
    MultiplexerInfo, SignalDefinition, ValueType,
};
use crate::types::{ContainerType, DecoderError, Result};
use autosar_data::*;
use std::path::Path;

/// Parse an ARXML file and return message and container definitions
pub fn parse_arxml_file(
    path: &Path,
) -> Result<(Vec<MessageDefinition>, Vec<ContainerDefinition>)> {
    log::info!("Parsing ARXML file with autosar-data: {:?}", path);

    if !path.exists() {
        return Err(DecoderError::ArxmlParseError(format!(
            "File not found: {:?}",
            path
        )));
    }

    let source_filename = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown.arxml")
        .to_string();

    // Load ARXML file using autosar-data
    let model = AutosarModel::new();
    let (_file, warnings) = model
        .load_file(path, false)
        .map_err(|e| DecoderError::ArxmlParseError(format!("Failed to load ARXML: {}", e)))?;

    // Log warnings
    for warning in warnings {
        log::warn!("ARXML warning: {}", warning);
    }

    // Parse messages and containers
    let mut parser = ArxmlParser::new(model, source_filename);
    parser.parse()?;

    log::info!(
        "ARXML parsing complete: {} messages, {} containers",
        parser.messages.len(),
        parser.containers.len()
    );

    Ok((parser.messages, parser.containers))
}

/// ARXML parser using autosar-data
struct ArxmlParser {
    model: AutosarModel,
    source: String,
    messages: Vec<MessageDefinition>,
    containers: Vec<ContainerDefinition>,
    /// Lookup map: PDU name → CAN ID (built once for performance)
    pdu_to_can_id: std::collections::HashMap<String, u32>,
}

impl ArxmlParser {
    fn new(model: AutosarModel, source: String) -> Self {
        Self {
            model,
            source,
            messages: Vec::new(),
            containers: Vec::new(),
            pdu_to_can_id: std::collections::HashMap::new(),
        }
    }

    fn parse(&mut self) -> Result<()> {
        // PERFORMANCE FIX: Build PDU-to-CAN-ID lookup map once (O(n) instead of O(n²))
        self.build_pdu_to_can_id_map()?;
        log::info!("Built PDU-to-CAN-ID map with {} entries", self.pdu_to_can_id.len());

        // Iterate through all elements in the model
        for (_depth, element) in self.model.elements_dfs() {
            let element_name = element.element_name();
            match element_name {
                ElementName::ISignalIPdu => {
                    if let Some(msg) = self.parse_i_signal_i_pdu(&element)? {
                        self.messages.push(msg);
                    }
                }
                ElementName::MultiplexedIPdu => {
                    if let Some(msg) = self.parse_multiplexed_i_pdu(&element)? {
                        self.messages.push(msg);
                    }
                }
                ElementName::ContainerIPdu => {
                    if let Some(container) = self.parse_container_i_pdu(&element)? {
                        self.containers.push(container);
                    }
                }
                _ => {}
            }
        }

        Ok(())
    }

    /// Build a lookup map of PDU name → CAN ID by scanning all PDU-TO-FRAME-MAPPINGs once
    fn build_pdu_to_can_id_map(&mut self) -> Result<()> {
        for (_depth, element) in self.model.elements_dfs() {
            if element.element_name() == ElementName::PduToFrameMapping {
                // Get the PDU reference
                if let Some(pdu_ref) = self.find_sub_element(&element, "PDU-REF")? {
                    if let Some(ref_text) = pdu_ref.character_data() {
                        let pdu_path = ref_text.string_value().unwrap_or_default();
                        let pdu_name = pdu_path.split('/').last().unwrap_or("");

                        // Get the CAN ID from the frame
                        if let Some(can_id) = self.get_can_id_from_frame_mapping(&element) {
                            self.pdu_to_can_id.insert(pdu_name.to_string(), can_id);
                        }
                    }
                }
            }
        }
        Ok(())
    }

    fn parse_i_signal_i_pdu(&self, element: &Element) -> Result<Option<MessageDefinition>> {
        // Get SHORT-NAME
        let name = self.get_short_name(element)?;

        // Get LENGTH
        let length = self
            .get_sub_element_text(element, "LENGTH")?
            .and_then(|s| s.parse::<usize>().ok())
            .unwrap_or(8);

        // Get CAN ID from lookup map (O(1) instead of O(n) DFS)
        let can_id = match self.pdu_to_can_id.get(&name) {
            Some(&id) => id,
            None => {
                log::warn!("No CAN ID found for I-PDU: {}", name);
                return Ok(None);
            }
        };

        // Parse signals
        let signals = self.parse_signal_mappings(element)?;

        if signals.is_empty() {
            return Ok(None);
        }

        Ok(Some(MessageDefinition {
            id: can_id,
            name,
            size: length,
            sender: None,
            signals,
            is_multiplexed: false,
            multiplexer_signal: None,
            source: self.source.clone(),
        }))
    }

    fn parse_multiplexed_i_pdu(&self, element: &Element) -> Result<Option<MessageDefinition>> {
        let name = self.get_short_name(element)?;

        let length = self
            .get_sub_element_text(element, "LENGTH")?
            .and_then(|s| s.parse::<usize>().ok())
            .unwrap_or(10);

        let can_id = match self.pdu_to_can_id.get(&name) {
            Some(&id) => id,
            None => {
                log::warn!("No CAN ID found for multiplexed I-PDU: {}", name);
                return Ok(None);
            }
        };

        // Get selector field information
        let selector_start = self
            .get_sub_element_text(element, "SELECTOR-FIELD-START-POSITION")?
            .and_then(|s| s.parse::<u16>().ok())
            .unwrap_or(0);

        let selector_length = self
            .get_sub_element_text(element, "SELECTOR-FIELD-LENGTH")?
            .and_then(|s| s.parse::<u16>().ok())
            .unwrap_or(8);

        let mut signals = Vec::new();
        let multiplexer_signal_name = format!("{}_selector", name);

        // Add the selector signal itself
        signals.push(SignalDefinition {
            name: multiplexer_signal_name.clone(),
            start_bit: selector_start,
            length: selector_length,
            byte_order: ByteOrder::LittleEndian,
            value_type: ValueType::Unsigned,
            factor: 1.0,
            offset: 0.0,
            min: 0.0,
            max: (1u64 << selector_length) as f64 - 1.0,
            unit: None,
            value_table: None,
            multiplexer_info: None,
        });

        // Parse static part signals
        if let Some(static_part) = self.find_sub_element(element, "STATIC-PARTS")? {
            for i_pdu_ref in self.find_all_sub_elements(&static_part, "I-PDU-REF")? {
                if let Some(ref_text) = i_pdu_ref.character_data() {
                    let pdu_name = ref_text.string_value().unwrap_or_default();
                    let pdu_short_name = pdu_name.split('/').last().unwrap_or("");

                    if let Some(referenced_pdu) = self.find_element_by_short_name(pdu_short_name)? {
                        let mut static_signals = self.parse_signal_mappings(&referenced_pdu)?;
                        signals.append(&mut static_signals);
                    }
                }
            }
        }

        // Parse dynamic part signals with multiplexer info
        if let Some(dynamic_parts) = self.find_sub_element(element, "DYNAMIC-PARTS")? {
            for dynamic_alt in self.find_all_sub_elements(&dynamic_parts, "DYNAMIC-PART-ALTERNATIVE")? {
                // Get selector value
                let selector_value = self
                    .get_sub_element_text(&dynamic_alt, "SELECTOR-FIELD-CODE")?
                    .and_then(|s| s.parse::<u64>().ok())
                    .unwrap_or(0);

                // Get I-PDU reference
                for i_pdu_ref in self.find_all_sub_elements(&dynamic_alt, "I-PDU-REF")? {
                    if let Some(ref_text) = i_pdu_ref.character_data() {
                        let pdu_name = ref_text.string_value().unwrap_or_default();
                        let pdu_short_name = pdu_name.split('/').last().unwrap_or("");

                        if let Some(referenced_pdu) = self.find_element_by_short_name(pdu_short_name)? {
                            let mut dynamic_signals = self.parse_signal_mappings(&referenced_pdu)?;

                            // Add multiplexer info to each signal
                            for signal in &mut dynamic_signals {
                                signal.multiplexer_info = Some(MultiplexerInfo {
                                    multiplexer_signal: multiplexer_signal_name.clone(),
                                    multiplexer_values: vec![selector_value],
                                });
                            }

                            signals.append(&mut dynamic_signals);
                        }
                    }
                }
            }
        }

        if signals.is_empty() {
            return Ok(None);
        }

        Ok(Some(MessageDefinition {
            id: can_id,
            name,
            size: length,
            sender: None,
            signals,
            is_multiplexed: true,
            multiplexer_signal: Some(multiplexer_signal_name),
            source: self.source.clone(),
        }))
    }

    fn parse_container_i_pdu(&self, element: &Element) -> Result<Option<ContainerDefinition>> {
        let name = self.get_short_name(element)?;

        let length = self
            .get_sub_element_text(element, "LENGTH")?
            .and_then(|s| s.parse::<usize>().ok())
            .unwrap_or(64);

        let can_id = match self.pdu_to_can_id.get(&name) {
            Some(&id) => id,
            None => {
                log::warn!("No CAN ID found for container I-PDU: {}", name);
                return Ok(None);
            }
        };

        // Get header type
        let header_type = self
            .get_sub_element_text(element, "HEADER-TYPE")?
            .unwrap_or_else(|| "NONE".to_string());

        let (container_type, header_size) = if header_type.contains("SHORT-HEADER") {
            (ContainerType::Dynamic, 4)
        } else if header_type.contains("LONG-HEADER") {
            (ContainerType::Dynamic, 8)
        } else {
            (ContainerType::Static, 0)
        };

        let layout = ContainerLayout::Dynamic {
            header_size,
            pdus: Vec::new(), // TODO: Parse contained PDU information
        };

        Ok(Some(ContainerDefinition {
            id: can_id,
            name,
            container_type,
            layout,
            source: self.source.clone(),
        }))
    }

    fn parse_signal_mappings(&self, pdu_element: &Element) -> Result<Vec<SignalDefinition>> {
        let mut signals = Vec::new();

        // Find I-SIGNAL-TO-PDU-MAPPINGS or I-SIGNAL-TO-I-PDU-MAPPINGS
        if let Some(mappings) = self.find_sub_element(pdu_element, "I-SIGNAL-TO-PDU-MAPPINGS")
            .or_else(|_| self.find_sub_element(pdu_element, "I-SIGNAL-TO-I-PDU-MAPPINGS"))?
        {
            for mapping in self.find_all_sub_elements(&mappings, "I-SIGNAL-TO-I-PDU-MAPPING")
                .or_else(|_| self.find_all_sub_elements(&mappings, "I-SIGNAL-TO-PDU-MAPPING"))?
            {
                if let Some(signal) = self.parse_signal_mapping(&mapping)? {
                    signals.push(signal);
                }
            }
        }

        Ok(signals)
    }

    fn parse_signal_mapping(&self, mapping: &Element) -> Result<Option<SignalDefinition>> {
        let signal_name = self.get_short_name(mapping)?;

        let start_position = self
            .get_sub_element_text(mapping, "START-POSITION")?
            .and_then(|s| s.parse::<u16>().ok())
            .unwrap_or(0);

        // Get byte order
        let byte_order_text = self
            .get_sub_element_text(mapping, "PACKING-BYTE-ORDER")?
            .unwrap_or_else(|| "MOST-SIGNIFICANT-BYTE-LAST".to_string());
        let byte_order = if byte_order_text.contains("MOST-SIGNIFICANT-BYTE-FIRST") {
            ByteOrder::BigEndian
        } else {
            ByteOrder::LittleEndian
        };

        // Try to get I-SIGNAL reference to find signal properties
        let (length, factor, offset, unit, min, max) = if let Some(i_signal_ref) =
            self.find_sub_element(mapping, "I-SIGNAL-REF")?
        {
            if let Some(ref_text) = i_signal_ref.character_data() {
                let signal_path = ref_text.string_value().unwrap_or_default();
                let signal_short_name = signal_path.split('/').last().unwrap_or("");

                if let Some(i_signal) = self.find_element_by_short_name(signal_short_name)? {
                    let len = self
                        .get_sub_element_text(&i_signal, "LENGTH")?
                        .and_then(|s| s.parse::<u16>().ok())
                        .unwrap_or(8);

                    // Parse SYSTEM-SIGNAL-REF for physical value conversion
                    let (factor, offset, unit, min, max) = self.parse_system_signal(&i_signal, len)?;

                    (len, factor, offset, unit, min, max)
                } else {
                    let default_max = (1u64 << 8) as f64 - 1.0;
                    (8, 1.0, 0.0, None, 0.0, default_max)
                }
            } else {
                let default_max = (1u64 << 8) as f64 - 1.0;
                (8, 1.0, 0.0, None, 0.0, default_max)
            }
        } else {
            let default_max = (1u64 << 8) as f64 - 1.0;
            (8, 1.0, 0.0, None, 0.0, default_max)
        };

        Ok(Some(SignalDefinition {
            name: signal_name,
            start_bit: start_position,
            length,
            byte_order,
            value_type: ValueType::Unsigned,
            factor,
            offset,
            min,
            max,
            unit,
            value_table: None,
            multiplexer_info: None,
        }))
    }

    /// Parse SYSTEM-SIGNAL for physical value conversion (factor, offset, unit, min, max)
    fn parse_system_signal(&self, i_signal: &Element, bit_length: u16) -> Result<(f64, f64, Option<String>, f64, f64)> {
        // Default values if SYSTEM-SIGNAL not found
        let default_max = (1u64 << bit_length) as f64 - 1.0;
        let mut factor = 1.0;
        let mut offset = 0.0;
        let mut unit = None;
        let mut min = 0.0;
        let mut max = default_max;

        // Find SYSTEM-SIGNAL-REF
        if let Some(sys_signal_ref) = self.find_sub_element(i_signal, "SYSTEM-SIGNAL-REF")? {
            if let Some(ref_text) = sys_signal_ref.character_data() {
                let sys_signal_path = ref_text.string_value().unwrap_or_default();
                let sys_signal_name = sys_signal_path.split('/').last().unwrap_or("");

                // Find the SYSTEM-SIGNAL element
                if let Some(system_signal) = self.find_element_by_short_name(sys_signal_name)? {
                    // Parse UNIT-REF (optional)
                    if let Some(unit_ref) = self.find_sub_element(&system_signal, "UNIT-REF")? {
                        if let Some(unit_text) = unit_ref.character_data() {
                            let unit_path = unit_text.string_value().unwrap_or_default();
                            let unit_name = unit_path.split('/').last().unwrap_or("");
                            if !unit_name.is_empty() {
                                unit = Some(unit_name.to_string());
                            }
                        }
                    }

                    // Parse COMPU-METHOD for factor/offset
                    if let Some((f, o, mi, ma)) = self.parse_compu_method(&system_signal)? {
                        factor = f;
                        offset = o;
                        if mi.is_finite() {
                            min = mi;
                        }
                        if ma.is_finite() {
                            max = ma;
                        }
                    }
                }
            }
        }

        Ok((factor, offset, unit, min, max))
    }

    /// Parse COMPU-METHOD to extract factor, offset, min, max
    /// Returns (factor, offset, min, max)
    fn parse_compu_method(&self, system_signal: &Element) -> Result<Option<(f64, f64, f64, f64)>> {
        // Navigate to COMPU-METHOD (can be inline or referenced)
        let compu_method = if let Some(inline) = self.find_sub_element(system_signal, "COMPU-METHOD")? {
            Some(inline)
        } else if let Some(compu_ref) = self.find_sub_element(system_signal, "COMPU-METHOD-REF")? {
            // Follow reference
            if let Some(ref_text) = compu_ref.character_data() {
                let compu_path = ref_text.string_value().unwrap_or_default();
                let compu_name = compu_path.split('/').last().unwrap_or("");
                self.find_element_by_short_name(compu_name)?
            } else {
                None
            }
        } else {
            None
        };

        if let Some(compu) = compu_method {
            // Parse COMPU-INTERNAL-TO-PHYS → COMPU-SCALES → COMPU-SCALE
            if let Some(internal_to_phys) = self.find_sub_element(&compu, "COMPU-INTERNAL-TO-PHYS")? {
                if let Some(compu_scales) = self.find_sub_element(&internal_to_phys, "COMPU-SCALES")? {
                    // Get first COMPU-SCALE (typically linear scaling)
                    let scales = self.find_all_sub_elements(&compu_scales, "COMPU-SCALE")?;
                    if let Some(scale) = scales.first() {
                        let mut factor = 1.0;
                        let mut offset = 0.0;
                        let mut min = f64::NEG_INFINITY;
                        let mut max = f64::INFINITY;

                        // Parse LOWER-LIMIT and UPPER-LIMIT
                        if let Some(lower_text) = self.get_sub_element_text(scale, "LOWER-LIMIT")? {
                            if let Ok(val) = lower_text.parse::<f64>() {
                                min = val;
                            }
                        }
                        if let Some(upper_text) = self.get_sub_element_text(scale, "UPPER-LIMIT")? {
                            if let Ok(val) = upper_text.parse::<f64>() {
                                max = val;
                            }
                        }

                        // Parse COMPU-RATIONAL-COEFFS (linear: y = (a0 + a1*x) / (b0 + b1*x))
                        // Simplified for linear case: y = offset + factor * x
                        if let Some(rational) = self.find_sub_element(scale, "COMPU-RATIONAL-COEFFS")? {
                            if let Some(numerator) = self.find_sub_element(&rational, "COMPU-NUMERATOR")? {
                                let v_elems = self.find_all_sub_elements(&numerator, "V")?;
                                if v_elems.len() >= 2 {
                                    // v[0] = offset (a0), v[1] = factor (a1)
                                    if let Some(v0_text) = v_elems[0].character_data() {
                                        if let Ok(val) = v0_text.string_value().unwrap_or_default().parse::<f64>() {
                                            offset = val;
                                        }
                                    }
                                    if let Some(v1_text) = v_elems[1].character_data() {
                                        if let Ok(val) = v1_text.string_value().unwrap_or_default().parse::<f64>() {
                                            factor = val;
                                        }
                                    }
                                }
                            }
                        }

                        // Parse COMPU-CONST (constant offset, no scaling)
                        if let Some(compu_const) = self.find_sub_element(scale, "COMPU-CONST")? {
                            if let Some(vt) = self.find_sub_element(&compu_const, "VT")? {
                                if let Some(vt_text) = vt.character_data() {
                                    if let Ok(val) = vt_text.string_value().unwrap_or_default().parse::<f64>() {
                                        offset = val;
                                        factor = 0.0; // Constant value
                                    }
                                }
                            }
                        }

                        return Ok(Some((factor, offset, min, max)));
                    }
                }
            }
        }

        Ok(None)
    }

    fn get_can_id_from_frame_mapping(&self, mapping: &Element) -> Option<u32> {
        // Navigate up to find CAN-FRAME with IDENTIFIER
        if let Ok(Some(parent)) = mapping.parent() {
            if let Ok(Some(grandparent)) = parent.parent() {
                // Look for CAN-FRAME in the parent hierarchy
                if let Ok(can_frames) = self.find_all_sub_elements(&grandparent, "CAN-FRAME") {
                    for frame in can_frames {
                        if let Ok(Some(id_text)) = self.get_sub_element_text(&frame, "IDENTIFIER") {
                            return self.parse_can_id(&id_text);
                        }
                    }
                }
            }
        }
        None
    }

    fn parse_can_id(&self, text: &str) -> Option<u32> {
        let text = text.trim();
        if text.starts_with("0x") || text.starts_with("0X") {
            u32::from_str_radix(&text[2..], 16).ok()
        } else {
            text.parse::<u32>().ok()
        }
    }

    // Helper methods for navigating autosar-data elements

    fn get_short_name(&self, element: &Element) -> Result<String> {
        self.get_sub_element_text(element, "SHORT-NAME")?
            .ok_or_else(|| DecoderError::ArxmlParseError("Missing SHORT-NAME".to_string()))
    }

    fn get_sub_element_text(&self, element: &Element, name: &str) -> Result<Option<String>> {
        if let Some(sub_elem) = self.find_sub_element(element, name)? {
            if let Some(char_data) = sub_elem.character_data() {
                return Ok(Some(char_data.string_value().unwrap_or_default()));
            }
        }
        Ok(None)
    }

    fn find_sub_element(&self, element: &Element, name: &str) -> Result<Option<Element>> {
        // Helper to match element name string
        for sub_elem in element.sub_elements() {
            if let Some(elem_name_str) = sub_elem.item_name() {
                if elem_name_str == name {
                    return Ok(Some(sub_elem));
                }
            }
        }
        Ok(None)
    }

    fn find_all_sub_elements(&self, element: &Element, name: &str) -> Result<Vec<Element>> {
        let mut results = Vec::new();
        for sub_elem in element.sub_elements() {
            if let Some(elem_name_str) = sub_elem.item_name() {
                if elem_name_str == name {
                    results.push(sub_elem);
                }
            }
        }
        Ok(results)
    }

    fn find_element_by_short_name(&self, short_name: &str) -> Result<Option<Element>> {
        for (_depth, element) in self.model.elements_dfs() {
            if let Ok(name) = self.get_short_name(&element) {
                if name == short_name {
                    return Ok(Some(element));
                }
            }
        }
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_arxml_file_not_found() {
        let result = parse_arxml_file(Path::new("nonexistent.arxml"));
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_real_arxml() {
        // Test with the actual example file if it exists
        let test_path = Path::new("../../arxml/system-4.2.arxml");
        if test_path.exists() {
            let result = parse_arxml_file(test_path);
            match result {
                Ok((messages, containers)) => {
                    println!("✓ Parsed {} messages and {} containers", messages.len(), containers.len());

                    // Print some details
                    for msg in messages.iter().take(3) {
                        println!("  Message: {} (ID: 0x{:X}, {} signals)",
                            msg.name, msg.id, msg.signals.len());
                    }
                }
                Err(e) => {
                    println!("✗ Parse error: {}", e);
                }
            }
        } else {
            println!("Test file not found: {:?}", test_path);
        }
    }
}
