//! Standalone CAN log decoder tool
//!
//! This tool decodes BLF/MF4 log files using DBC/ARXML signal definitions
//! and displays decoded messages, signals, and container PDUs.
//!
//! Usage:
//!   decode_log.exe <log_file.blf> [--dbc <file.dbc>] [--arxml <file.arxml>] [--limit <count>]
//!
//! Example:
//!   decode_log.exe trace.blf --dbc powertrain.dbc --arxml system.arxml --limit 100

use can_log_decoder::{Decoder, DecoderConfig, DecodedEvent, SignalValue, Timestamp};
use std::collections::HashMap;
use std::env;
use std::path::PathBuf;

fn timestamp_to_secs(ts: &Timestamp) -> f64 {
    ts.timestamp() as f64 + (ts.timestamp_subsec_nanos() as f64 / 1_000_000_000.0)
}

struct DecoderStats {
    total_frames: usize,
    raw_frames: usize,
    decoded_messages: usize,
    container_pdus: usize,
    contained_pdus_extracted: usize,
    signals_decoded: usize,
    unique_can_ids: HashMap<u32, usize>,
    unique_messages: HashMap<String, usize>,
}

impl DecoderStats {
    fn new() -> Self {
        Self {
            total_frames: 0,
            raw_frames: 0,
            decoded_messages: 0,
            container_pdus: 0,
            contained_pdus_extracted: 0,
            signals_decoded: 0,
            unique_can_ids: HashMap::new(),
            unique_messages: HashMap::new(),
        }
    }

    fn print_summary(&self) {
        println!("\n=== DECODING SUMMARY ===");
        println!("Total frames processed: {}", self.total_frames);
        println!("Raw frames (unknown): {}", self.raw_frames);
        println!("Decoded messages: {}", self.decoded_messages);
        println!("Container PDUs: {}", self.container_pdus);
        println!("Contained PDUs extracted: {}", self.contained_pdus_extracted);
        println!("Total signals decoded: {}", self.signals_decoded);
        println!("Unique CAN IDs seen: {}", self.unique_can_ids.len());
        println!("Unique message names: {}", self.unique_messages.len());

        if !self.unique_messages.is_empty() {
            println!("\nTop 10 Most Frequent Messages:");
            let mut sorted: Vec<_> = self.unique_messages.iter().collect();
            sorted.sort_by(|a, b| b.1.cmp(a.1));
            for (name, count) in sorted.iter().take(10) {
                println!("  {}: {} times", name, count);
            }
        }
    }
}

fn format_signal_value(value: &SignalValue) -> String {
    match value {
        SignalValue::Boolean(b) => format!("{}", b),
        SignalValue::Integer(i) => format!("{}", i),
        SignalValue::Float(f) => format!("{:.2}", f),
    }
}

fn print_decoded_event(event: &DecodedEvent, verbose: bool) {
    match event {
        DecodedEvent::Message {
            timestamp,
            channel,
            can_id,
            message_name,
            signals,
            is_multiplexed,
            multiplexer_value,
            ..
        } => {
            println!(
                "[{:.6}s] CH{} 0x{:03X} {}{}",
                timestamp_to_secs(timestamp),
                channel,
                can_id,
                message_name.as_deref().unwrap_or("Unknown"),
                if *is_multiplexed {
                    format!(" (MUX={})", multiplexer_value.unwrap_or(0))
                } else {
                    String::new()
                }
            );

            if verbose && !signals.is_empty() {
                for signal in signals.iter().take(5) {
                    // Show first 5 signals
                    let value_str = format_signal_value(&signal.value);
                    let unit_str = signal.unit.as_deref().unwrap_or("");
                    let desc_str = signal
                        .value_description
                        .as_deref()
                        .map(|d| format!(" \"{}\"", d))
                        .unwrap_or_default();

                    println!("    {}: {}{}{}", signal.name, value_str, unit_str, desc_str);
                }
                if signals.len() > 5 {
                    println!("    ... and {} more signals", signals.len() - 5);
                }
            }
        }

        DecodedEvent::ContainerPdu {
            timestamp,
            container_id,
            container_name,
            container_type,
            contained_pdus,
        } => {
            println!(
                "[{:.6}s] CONTAINER 0x{:03X} {} ({:?}) - {} PDUs",
                timestamp_to_secs(timestamp),
                container_id,
                container_name,
                container_type,
                contained_pdus.len()
            );

            if verbose {
                for pdu in contained_pdus {
                    println!(
                        "    └─ PDU: {} (ID: {}, {} bytes)",
                        pdu.name,
                        pdu.pdu_id,
                        pdu.data.len()
                    );
                }
            }
        }

        DecodedEvent::RawFrame {
            timestamp,
            channel,
            can_id,
            data,
            ..
        } => {
            if verbose {
                println!(
                    "[{:.6}s] CH{} 0x{:03X} RAW [{} bytes]",
                    timestamp_to_secs(timestamp),
                    channel,
                    can_id,
                    data.len()
                );
            }
        }

        _ => {} // Skip other event types for now
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        eprintln!("Usage: {} <log_file.blf|.mf4> [--dbc <file.dbc>] [--arxml <file.arxml>] [--limit <count>] [--verbose]", args[0]);
        eprintln!("\nExample:");
        eprintln!("  {} trace.blf --dbc powertrain.dbc --arxml system.arxml --limit 100", args[0]);
        std::process::exit(1);
    }

    let log_file = PathBuf::from(&args[1]);
    let mut dbc_files = Vec::new();
    let mut arxml_files = Vec::new();
    let mut limit: Option<usize> = None;
    let mut verbose = false;

    // Parse arguments
    let mut i = 2;
    while i < args.len() {
        match args[i].as_str() {
            "--dbc" => {
                i += 1;
                if i < args.len() {
                    dbc_files.push(PathBuf::from(&args[i]));
                }
            }
            "--arxml" => {
                i += 1;
                if i < args.len() {
                    arxml_files.push(PathBuf::from(&args[i]));
                }
            }
            "--limit" => {
                i += 1;
                if i < args.len() {
                    limit = Some(args[i].parse()?);
                }
            }
            "--verbose" | "-v" => {
                verbose = true;
            }
            _ => {
                eprintln!("Unknown argument: {}", args[i]);
            }
        }
        i += 1;
    }

    println!("=== CAN Log Decoder ===");
    println!("Log file: {:?}", log_file);
    println!("DBC files: {} loaded", dbc_files.len());
    println!("ARXML files: {} loaded", arxml_files.len());
    if let Some(n) = limit {
        println!("Limit: {} events", n);
    }
    println!("Verbose: {}", verbose);
    println!();

    // Create decoder and load signal definitions
    let mut decoder = Decoder::new();

    for dbc_file in &dbc_files {
        println!("Loading DBC: {:?}", dbc_file);
        decoder.add_dbc(dbc_file)?;
    }

    for arxml_file in &arxml_files {
        println!("Loading ARXML: {:?}", arxml_file);
        decoder.add_arxml(arxml_file)?;
    }

    // Print database statistics
    let db_stats = decoder.database_stats();
    println!("\n=== SIGNAL DATABASE ===");
    println!("Messages: {}", db_stats.num_messages);
    println!("Signals: {}", db_stats.num_signals);
    println!("Containers: {}", db_stats.num_containers);
    println!();

    if db_stats.num_messages == 0 {
        println!("⚠ Warning: No signal definitions loaded!");
        println!("  All frames will be shown as RAW (undecoded)");
        println!();
    }

    // Decode log file
    println!("=== DECODING LOG FILE ===\n");
    let config = DecoderConfig::new();
    let events = decoder.decode_file(&log_file, config)?;

    let mut stats = DecoderStats::new();
    let mut event_count = 0;

    for result in events {
        match result {
            Ok(event) => {
                stats.total_frames += 1;

                // Update statistics
                match &event {
                    DecodedEvent::Message {
                        can_id,
                        message_name,
                        signals,
                        ..
                    } => {
                        stats.decoded_messages += 1;
                        stats.signals_decoded += signals.len();
                        *stats.unique_can_ids.entry(*can_id).or_insert(0) += 1;
                        if let Some(name) = message_name {
                            *stats.unique_messages.entry(name.clone()).or_insert(0) += 1;
                        }
                    }
                    DecodedEvent::ContainerPdu { contained_pdus, .. } => {
                        stats.container_pdus += 1;
                        stats.contained_pdus_extracted += contained_pdus.len();
                    }
                    DecodedEvent::RawFrame { .. } => {
                        stats.raw_frames += 1;
                    }
                    _ => {}
                }

                // Print event (if within limit)
                if let Some(max) = limit {
                    if event_count >= max {
                        println!("\n... (limit of {} events reached)", max);
                        break;
                    }
                }

                print_decoded_event(&event, verbose);
                event_count += 1;
            }
            Err(e) => {
                eprintln!("Error decoding event: {}", e);
            }
        }
    }

    stats.print_summary();

    Ok(())
}
