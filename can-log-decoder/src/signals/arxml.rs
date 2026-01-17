//! ARXML (AUTOSAR XML) file parser using autosar-data crate
//!
//! Parses AUTOSAR ARXML files to extract signal and container PDU definitions.
//! Uses the autosar-data crate for robust AUTOSAR 4.x support.

use crate::signals::database::{
    ByteOrder, ContainedPduInfo, ContainerDefinition, ContainerLayout, MessageDefinition,
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
                    match self.parse_i_signal_i_pdu(&element) {
                        Ok(Some(msg)) => self.messages.push(msg),
                        Ok(None) => {}, // Skipped (no CAN ID, etc)
                        Err(e) => {
                            log::warn!("Failed to parse I-SIGNAL-I-PDU: {} (continuing...)", e);
                        }
                    }
                }
                ElementName::MultiplexedIPdu => {
                    match self.parse_multiplexed_i_pdu(&element) {
                        Ok(Some(msg)) => self.messages.push(msg),
                        Ok(None) => {},
                        Err(e) => {
                            log::warn!("Failed to parse MULTIPLEXED-I-PDU: {} (continuing...)", e);
                        }
                    }
                }
                ElementName::ContainerIPdu => {
                    match self.parse_container_i_pdu(&element) {
                        Ok(Some(container)) => self.containers.push(container),
                        Ok(None) => {},
                        Err(e) => {
                            log::warn!("Failed to parse CONTAINER-I-PDU: {} (continuing...)", e);
                        }
                    }
                }
                _ => {}
            }
        }

        Ok(())
    }

    /// Build a lookup map of PDU name → CAN ID by scanning all CAN-FRAME-TRIGGERINGs once
    ///
    /// AUTOSAR structure:
    /// CAN-FRAME-TRIGGERING → IDENTIFIER (CAN ID) + FRAME-REF
    /// CAN-FRAME → PDU-TO-FRAME-MAPPING → PDU-REF (PDU name)
    fn build_pdu_to_can_id_map(&mut self) -> Result<()> {
        // Step 1: Build FRAME-REF → CAN-ID map from CAN-FRAME-TRIGGERINGs
        let mut frame_to_can_id = std::collections::HashMap::new();

        for (_depth, element) in self.model.elements_dfs() {
            if element.element_name() == ElementName::CanFrameTriggering {
                // Get IDENTIFIER (CAN ID)
                if let Some(id_text) = self.get_sub_element_text(&element, "IDENTIFIER")? {
                    if let Some(can_id) = self.parse_can_id(&id_text) {
                        // Get FRAME-REF (link to CAN-FRAME)
                        if let Some(frame_ref) = self.find_sub_element(&element, "FRAME-REF")? {
                            if let Some(ref_text) = frame_ref.character_data() {
                                let frame_path = ref_text.string_value().unwrap_or_default();
                                frame_to_can_id.insert(frame_path.clone(), can_id);
                            }
                        }
                    }
                }
            }
        }

        // Step 2: Map PDU name → CAN ID by finding PDU-TO-FRAME-MAPPINGs
        for (_depth, element) in self.model.elements_dfs() {
            if element.element_name() == ElementName::CanFrame {
                // Get this frame's AUTOSAR path
                if let Ok(frame_path) = element.path() {
                    // Look up CAN ID for this frame
                    if let Some(&can_id) = frame_to_can_id.get(&frame_path) {
                        // Find all PDU-TO-FRAME-MAPPINGs in this frame
                        if let Some(mappings) = self.find_sub_element(&element, "PDU-TO-FRAME-MAPPINGS")? {
                            for mapping in self.find_all_sub_elements(&mappings, "PDU-TO-FRAME-MAPPING")? {
                                // Get PDU-REF
                                if let Some(pdu_ref) = self.find_sub_element(&mapping, "PDU-REF")? {
                                    if let Some(ref_text) = pdu_ref.character_data() {
                                        let pdu_path = ref_text.string_value().unwrap_or_default();
                                        let pdu_name = pdu_path.split('/').last().unwrap_or("");

                                        if !pdu_name.is_empty() {
                                            self.pdu_to_can_id.insert(pdu_name.to_string(), can_id);
                                        }
                                    }
                                }
                            }
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

        log::debug!("Container {} has HEADER-TYPE: {:?}", name, header_type);

        let (container_type, header_size) = if header_type.contains("SHORT-HEADER") {
            (ContainerType::Dynamic, 4)
        } else if header_type.contains("LONG-HEADER") {
            (ContainerType::Dynamic, 8)
        } else {
            (ContainerType::Static, 0)
        };

        // Parse contained PDU information from CONTAINED-PDU-TRIGGERING-REFS
        let contained_pdus = self.parse_contained_pdus(element, length)?;

        let layout = match container_type {
            ContainerType::Dynamic => ContainerLayout::Dynamic {
                header_size,
                pdus: contained_pdus,
            },
            ContainerType::Static => ContainerLayout::Static {
                pdus: contained_pdus,
            },
            ContainerType::Queued => {
                // For queued containers, all PDUs should be the same type
                if let Some(first_pdu) = contained_pdus.first() {
                    ContainerLayout::Queued {
                        pdu_id: first_pdu.pdu_id,
                        pdu_size: first_pdu.size,
                    }
                } else {
                    log::warn!("Queued container has no PDUs defined");
                    ContainerLayout::Dynamic {
                        header_size: 0,
                        pdus: Vec::new(),
                    }
                }
            }
        };

        Ok(Some(ContainerDefinition {
            id: can_id,
            name,
            container_type,
            layout,
            source: self.source.clone(),
        }))
    }

    /// Parse contained PDU information from CONTAINED-PDU-TRIGGERING-REFS
    ///
    /// The structure is:
    /// CONTAINER-I-PDU → CONTAINED-PDU-TRIGGERING-REF → PDU-TRIGGERING → I-PDU-REF → I-SIGNAL-I-PDU
    fn parse_contained_pdus(
        &self,
        container_element: &Element,
        container_length: usize,
    ) -> Result<Vec<ContainedPduInfo>> {
        use autosar_data::ElementName;

        let mut contained_pdus = Vec::new();
        let mut current_position = 0;

        // Find CONTAINED-PDU-TRIGGERING-REFS
        if let Some(refs_element) = self.find_sub_element(container_element, "CONTAINED-PDU-TRIGGERING-REFS")? {
            // Get all CONTAINED-PDU-TRIGGERING-REF elements
            for ref_element in self.find_all_sub_elements(&refs_element, "CONTAINED-PDU-TRIGGERING-REF")? {
                if let Some(ref_text) = ref_element.character_data() {
                    let pdu_triggering_path = ref_text.string_value().unwrap_or_default();

                    // Find the PDU-TRIGGERING element by path
                    if let Some(pdu_triggering) = self.find_element_by_path(&pdu_triggering_path)? {
                        // Get I-PDU-REF from PDU-TRIGGERING
                        if let Some(ipdu_ref) = self.find_sub_element(&pdu_triggering, "I-PDU-REF")? {
                            if let Some(ipdu_ref_text) = ipdu_ref.character_data() {
                                let ipdu_path = ipdu_ref_text.string_value().unwrap_or_default();
                                let ipdu_name = ipdu_path.split('/').last().unwrap_or("Unknown");

                                // Try to find the I-PDU to get its LENGTH
                                let pdu_size = if let Some(ipdu_element) = self.find_element_by_path(&ipdu_path)? {
                                    self.get_sub_element_text(&ipdu_element, "LENGTH")?
                                        .and_then(|s| s.parse::<usize>().ok())
                                        .unwrap_or(8) // Default to 8 bytes if not specified
                                } else {
                                    8 // Default size
                                };

                                // Generate a PDU ID from the name hash (since AUTOSAR doesn't have explicit PDU IDs)
                                let pdu_id = Self::generate_pdu_id(ipdu_name);

                                contained_pdus.push(ContainedPduInfo {
                                    pdu_id,
                                    name: ipdu_name.to_string(),
                                    position: current_position,
                                    size: pdu_size,
                                });

                                current_position += pdu_size;

                                // Check if we've exceeded container length
                                if current_position > container_length {
                                    log::warn!(
                                        "Contained PDUs exceed container length: {} > {}",
                                        current_position,
                                        container_length
                                    );
                                    break;
                                }
                            }
                        }
                    }
                }
            }
        }

        Ok(contained_pdus)
    }

    /// Generate a deterministic PDU ID from a PDU name using a simple hash
    fn generate_pdu_id(name: &str) -> u32 {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        name.hash(&mut hasher);
        (hasher.finish() & 0xFFFF) as u32 // Use lower 16 bits for PDU ID
    }

    /// Find an element by its AUTOSAR path
    fn find_element_by_path(&self, path: &str) -> Result<Option<Element>> {
        // Try to find element by matching the path
        // This is a simplified implementation - in a real parser you'd navigate the AR model properly
        for (_depth, element) in self.model.elements_dfs() {
            if let Ok(elem_path) = element.path() {
                if elem_path == path {
                    return Ok(Some(element));
                }
            }
        }
        Ok(None)
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
        // Get signal name from I-SIGNAL-REF (not from mapping's SHORT-NAME)
        let signal_name = if let Some(i_signal_ref) = self.find_sub_element(mapping, "I-SIGNAL-REF")? {
            if let Some(ref_text) = i_signal_ref.character_data() {
                let signal_path = ref_text.string_value().unwrap_or_default();
                signal_path.split('/').last().unwrap_or("Unknown").to_string()
            } else {
                log::warn!("I-SIGNAL-REF has no character data, skipping mapping");
                return Ok(None);
            }
        } else {
            log::warn!("Signal mapping has no I-SIGNAL-REF, skipping");
            return Ok(None);
        };

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
        let element_type = element.element_name();
        // Use item_name() to get SHORT-NAME from identifiable elements
        element.item_name()
            .ok_or_else(|| DecoderError::ArxmlParseError(
                format!("Missing SHORT-NAME in element type: {:?}", element_type)
            ))
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
        // Use autosar-data's typed API when possible
        // For named sub-elements (like LENGTH, START-POSITION), we need to parse the string
        // into an ElementName enum
        use autosar_data::ElementName;

        // Try to parse name string into ElementName enum
        if let Ok(element_name) = name.parse::<ElementName>() {
            // Use typed API (more efficient)
            return Ok(element.get_sub_element(element_name));
        }

        // Fallback should never be needed since autosar-data has all AUTOSAR element names
        log::warn!("Failed to parse element name '{}' into ElementName enum", name);
        Ok(None)
    }

    fn find_all_sub_elements(&self, element: &Element, name: &str) -> Result<Vec<Element>> {
        use autosar_data::ElementName;

        // Parse the string into an ElementName enum
        if let Ok(target_element_name) = name.parse::<ElementName>() {
            let mut results = Vec::new();
            for sub_elem in element.sub_elements() {
                if sub_elem.element_name() == target_element_name {
                    results.push(sub_elem);
                }
            }
            return Ok(results);
        }

        log::warn!("Failed to parse element name '{}' into ElementName enum", name);
        Ok(Vec::new())
    }

    fn find_element_by_short_name(&self, short_name: &str) -> Result<Option<Element>> {
        for (_depth, element) in self.model.elements_dfs() {
            // Use item_name() directly (more efficient than get_short_name which checks errors)
            if let Some(name) = element.item_name() {
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
        let workspace_root = std::env::var("CARGO_MANIFEST_DIR")
            .map(|p| std::path::PathBuf::from(p).parent().unwrap().to_path_buf())
            .unwrap_or_else(|_| std::path::PathBuf::from(".."));
        let test_path = workspace_root.join("arxml/system-4.2.arxml");
        if test_path.exists() {
            let result = parse_arxml_file(&test_path);
            match result {
                Ok((messages, containers)) => {
                    println!("✓ Parsed {} messages and {} containers", messages.len(), containers.len());

                    // Print some details
                    for msg in messages.iter().take(3) {
                        println!("  Message: {} (ID: 0x{:X}, {} signals)",
                            msg.name, msg.id, msg.signals.len());
                    }

                    // Print container details
                    for container in &containers {
                        println!("  Container: {} (ID: 0x{:X}, type: {:?})",
                            container.name, container.id, container.container_type);
                        match &container.layout {
                            ContainerLayout::Static { pdus } | ContainerLayout::Dynamic { pdus, .. } => {
                                println!("    Contains {} PDUs:", pdus.len());
                                for pdu in pdus {
                                    println!("      - {} (ID: {}, pos: {}, size: {})",
                                        pdu.name, pdu.pdu_id, pdu.position, pdu.size);
                                }
                            }
                            ContainerLayout::Queued { pdu_id, pdu_size } => {
                                println!("    Queued PDU ID: {}, size: {}", pdu_id, pdu_size);
                            }
                        }
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
