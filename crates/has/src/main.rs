use std::io::{self, BufRead, IsTerminal};

use clap::Parser;

#[derive(Parser)]
#[command(name = "has", version = shared::VERSION, about = "Find what process has a file, port, or resource open")]
struct Cli {
    /// Resources to look up: file paths, :port, IP address, or hostname
    resources: Vec<String>,

    /// Suppress header row
    #[arg(short = 'H', long = "no-header")]
    no_header: bool,
}

fn main() {
    let cli = Cli::parse();
    if let Err(e) = run(cli) {
        eprintln!("has: {}", e);
        std::process::exit(1);
    }
}

fn run(cli: Cli) -> Result<(), String> {
    let mut inputs: Vec<String> = cli.resources;

    // Read from stdin if no args and stdin is not a terminal
    if inputs.is_empty() {
        let stdin = io::stdin();
        if stdin.is_terminal() {
            return Err("no resources specified".to_string());
        }
        for line in stdin.lock().lines() {
            let line = line.map_err(|e| format!("reading stdin: {}", e))?;
            let trimmed = line.trim().to_string();
            if !trimmed.is_empty() {
                inputs.push(trimmed);
            }
        }
    }

    if inputs.is_empty() {
        return Ok(());
    }

    let mut all_entries = Vec::new();
    let mut errors = Vec::new();

    for input in &inputs {
        match has::query::parse_query(input) {
            Ok(query) => match has::query::execute(&query) {
                Ok(entries) => all_entries.extend(entries),
                Err(e) => errors.push(format!("{}: {}", input, e)),
            },
            Err(e) => errors.push(e),
        }
    }

    if !all_entries.is_empty() {
        has::output::print_process_table(&all_entries, cli.no_header);
    }

    for e in &errors {
        eprintln!("has: {}", e);
    }

    if all_entries.is_empty() && !errors.is_empty() {
        std::process::exit(1);
    }

    Ok(())
}
