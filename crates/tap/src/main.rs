use clap::{Parser, Subcommand};
use std::time::Instant;

#[derive(Parser)]
#[command(name = "tap", about = "Pipeline stage debugger")]
struct Cli {
    #[command(subcommand)]
    command: Option<Cmd>,

    /// Capture point name
    #[arg(short = 'n', long = "name")]
    name: Option<String>,

    /// Summary mode: count lines/bytes but don't write data
    #[arg(short = 's', long = "summary")]
    summary: bool,

    /// Max lines to capture
    #[arg(short = 'l', long = "lines")]
    max_lines: Option<u64>,

    /// Max bytes to capture
    #[arg(short = 'b', long = "bytes")]
    max_bytes: Option<u64>,

    /// Auto-detect format
    #[arg(short = 'f', long = "format")]
    detect_format: bool,
}

#[derive(Subcommand)]
enum Cmd {
    /// Display captured data
    Show {
        name: String,
        #[arg(short = 'S', long = "session")]
        session: Option<String>,
    },
    /// Diff two captures
    Diff {
        name1: String,
        name2: String,
        #[arg(short = 'S', long = "session")]
        session: Option<String>,
    },
    /// Show capture stats table
    Stats {
        #[arg(short = 'S', long = "session")]
        session: Option<String>,
    },
    /// Replay captured data to stdout
    Replay {
        name: String,
        #[arg(short = 'S', long = "session")]
        session: Option<String>,
    },
    /// List most recent session captures
    Last,
    /// List all sessions
    Sessions,
    /// Remove old sessions
    Clean {
        #[arg(long = "older-than")]
        older_than: String,
    },
}

fn main() {
    let cli = Cli::parse();

    let result = match cli.command {
        Some(Cmd::Show { name, session }) => tap::query::show(&name, &session),
        Some(Cmd::Diff { name1, name2, session }) => tap::query::diff(&name1, &name2, &session),
        Some(Cmd::Stats { session }) => tap::query::stats(&session),
        Some(Cmd::Replay { name, session }) => tap::query::replay(&name, &session),
        Some(Cmd::Last) => tap::query::last(),
        Some(Cmd::Sessions) => tap::query::sessions(),
        Some(Cmd::Clean { older_than }) => tap::query::clean(&older_than),
        None => run_capture(cli),
    };

    if let Err(e) = result {
        eprintln!("tap: {}", e);
        std::process::exit(1);
    }
}

fn run_capture(cli: Cli) -> Result<(), String> {
    // No name = pure passthrough
    let Some(name) = cli.name else {
        tap::passthrough::relay(None).map_err(|e| e.to_string())?;
        return Ok(());
    };

    let start = Instant::now();
    let sid = tap::session::session_id();
    let data_path = tap::session::data_path(&sid, &name);

    // Set up capture channel
    let (tx, rx) = crossbeam_channel::bounded::<Vec<u8>>(256);

    let capture_opts = tap::capture::CaptureOpts {
        data_path: data_path.clone(),
        summary_only: cli.summary,
        max_lines: cli.max_lines,
        max_bytes: cli.max_bytes,
    };

    let handle = tap::capture::capture_thread(rx, capture_opts);

    // Relay stdin -> stdout, sending copies to capture thread
    let (total_bytes, total_lines) = tap::passthrough::relay(Some(&tx)).map_err(|e| e.to_string())?;
    drop(tx);

    let result = handle.join().map_err(|_| "capture thread panicked")?;
    let elapsed = start.elapsed().as_secs_f64();

    let format = if cli.detect_format {
        tap::detect::detect_format(&result.sample)
    } else {
        tap::detect::Format::Text
    };

    // Write metadata
    let meta = tap::session::Meta {
        name: name.clone(),
        session_id: sid.clone(),
        timestamp: chrono::Utc::now().to_rfc3339(),
        bytes: if cli.summary { total_bytes } else { result.bytes_written },
        lines: if cli.summary { total_lines } else { result.lines_written },
        duration_secs: elapsed,
        format,
        truncated: result.truncated,
    };

    let meta_path = tap::session::meta_path(&sid, &name);
    if let Some(parent) = meta_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let meta_json = serde_json::to_string_pretty(&meta).map_err(|e| e.to_string())?;
    std::fs::write(&meta_path, meta_json).map_err(|e| e.to_string())?;

    // Print summary to stderr
    eprintln!(
        "tap: {} ({}, {} lines, {}, {})",
        name,
        shared::human::format_bytes(meta.bytes),
        meta.lines,
        format,
        shared::human::format_duration(elapsed),
    );

    Ok(())
}
