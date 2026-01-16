//! CAN Log Reader CLI Application
//!
//! This is the command-line interface for the CAN log reader/parser.
//! It uses the can-log-decoder library and adds:
//! - Signal change tracking (old→new values)
//! - Event detection and state machines
//! - Expression evaluation
//! - Callback system (C FFI + declarative)
//! - Report generation (TXT/HTML)

use anyhow::Result;
use clap::Parser;
use std::path::PathBuf;

mod config;
mod state;
mod events;
mod callbacks;
mod report;

/// CAN Log Reader - Decode and analyze CAN log files
#[derive(Parser, Debug)]
#[command(name = "can-log-cli")]
#[command(about = "Decode and analyze CAN log files (BLF, MF4)", long_about = None)]
#[command(version)]
struct Args {
    /// Path to BLF/MF4 log file to decode
    #[arg(short, long, value_name = "FILE")]
    log: Option<PathBuf>,

    /// Path to DBC file(s) (can be repeated)
    #[arg(long, value_name = "FILE")]
    dbc: Vec<PathBuf>,

    /// Path to ARXML file(s) (can be repeated)
    #[arg(long, value_name = "FILE")]
    arxml: Vec<PathBuf>,

    /// Output file for decoded signals (default: stdout)
    #[arg(short, long, value_name = "FILE")]
    output: Option<PathBuf>,

    /// Path to configuration file (config.toml) - for advanced features
    #[arg(short, long, value_name = "FILE")]
    config: Option<PathBuf>,

    /// Maximum number of frames to decode (for testing)
    #[arg(long, value_name = "COUNT")]
    max_frames: Option<usize>,

    /// Verbosity level (can be repeated: -v, -vv, -vvv)
    #[arg(short, long, action = clap::ArgAction::Count)]
    verbose: u8,

    /// Suppress all output except errors
    #[arg(short, long)]
    quiet: bool,
}

fn main() -> Result<()> {
    // Parse command line arguments
    let args = Args::parse();

    // Initialize logging
    init_logging(args.verbose, args.quiet);

    log::info!("CAN Log Reader CLI v{}", env!("CARGO_PKG_VERSION"));
    log::info!("Using decoder library v{}", can_log_decoder::VERSION);

    // Check if simple decode mode or config mode
    if args.log.is_some() || !args.dbc.is_empty() || !args.arxml.is_empty() {
        // Simple decode mode - just decode and print signals
        simple_decode_mode(&args)?;
    } else if let Some(config_path) = &args.config {
        // Advanced config mode (for future phases)
        advanced_config_mode(config_path, &args)?;
    } else {
        // No arguments - show help
        println!("CAN Log Reader - No input specified");
        println!("\nQuick Start:");
        println!("  can-log-cli --log trace.blf --dbc signals.dbc");
        println!("  can-log-cli --log trace.blf --arxml system.arxml");
        println!("\nFor advanced features:");
        println!("  can-log-cli --config config.toml");
        println!("\nUse --help for more options");
    }

    Ok(())
}

/// Simple decode mode - load signals, decode log, print results
fn simple_decode_mode(args: &Args) -> Result<()> {
    use can_log_decoder::Decoder;
    use std::fs::File;
    use std::io::{self, Write};

    println!("═══════════════════════════════════════════════");
    println!("  CAN Log Decoder - Simple Mode");
    println!("═══════════════════════════════════════════════\n");

    // Create decoder
    let mut decoder = Decoder::new();

    // Load DBC files
    for dbc_path in &args.dbc {
        print!("Loading DBC: {:?} ... ", dbc_path);
        io::stdout().flush()?;
        match decoder.add_dbc(dbc_path) {
            Ok(_) => println!("✓"),
            Err(e) => {
                println!("✗");
                eprintln!("Error loading DBC: {}", e);
                return Err(e.into());
            }
        }
    }

    // Load ARXML files
    for arxml_path in &args.arxml {
        print!("Loading ARXML: {:?} ... ", arxml_path);
        io::stdout().flush()?;
        match decoder.add_arxml(arxml_path) {
            Ok(_) => println!("✓"),
            Err(e) => {
                println!("✗");
                eprintln!("Error loading ARXML: {}", e);
                return Err(e.into());
            }
        }
    }

    // Show database stats
    let stats = decoder.database_stats();
    println!("\n📊 Signal Database:");
    println!("  Messages: {}", stats.num_messages);
    println!("  Signals:  {}", stats.num_signals);
    println!("  Containers: {}", stats.num_containers);

    // Check if we have a log file to decode
    if let Some(log_path) = &args.log {
        println!("\n📄 Decoding log file: {:?}", log_path);
        println!("───────────────────────────────────────────────\n");

        // TODO: Implement actual decoding when BLF parser is complete
        // For now, just show what would happen
        println!("⚠️  Log file parsing not yet implemented (Phase 3 stub)");
        println!("   BLF parser integration coming in next session!");
        println!("\nWhat WILL work when BLF parser is ready:");
        println!("  ✓ Parse BLF file");
        println!("  ✓ Extract CAN frames");
        println!("  ✓ Decode signals using loaded DBC/ARXML");
        println!("  ✓ Show physical values with units");
        println!("  ✓ Handle multiplexed signals");

    } else {
        println!("\n✓ Signal database loaded successfully!");
        println!("  Add --log <file.blf> to decode CAN frames");
    }

    Ok(())
}

/// Advanced config mode - full features (future phases)
fn advanced_config_mode(config_path: &PathBuf, _args: &Args) -> Result<()> {
    println!("═══════════════════════════════════════════════");
    println!("  CAN Log Decoder - Advanced Mode");
    println!("═══════════════════════════════════════════════\n");

    log::info!("Loading configuration from: {:?}", config_path);
    let config = config::load_config(config_path)?;
    log::debug!("Configuration loaded successfully");

    println!("✓ Configuration loaded: {:?}", config_path);
    println!("\n⚠️  Advanced features coming in future phases:");
    println!("  • Event tracking (Phase 10)");
    println!("  • Expression evaluation (Phase 9)");
    println!("  • Callbacks (Phase 11)");
    println!("  • HTML reports (Phase 12)");
    println!("  • Multi-file processing (Phase 13)");

    Ok(())
}

/// Initialize logging based on verbosity level
fn init_logging(verbose: u8, quiet: bool) {
    use env_logger::Builder;
    use log::LevelFilter;
    use std::io::Write;

    let level = if quiet {
        LevelFilter::Error
    } else {
        match verbose {
            0 => LevelFilter::Info,
            1 => LevelFilter::Debug,
            _ => LevelFilter::Trace,
        }
    };

    Builder::new()
        .filter_level(level)
        .format(|buf, record| {
            writeln!(
                buf,
                "[{} {}] {}",
                record.level(),
                record.target(),
                record.args()
            )
        })
        .init();
}
