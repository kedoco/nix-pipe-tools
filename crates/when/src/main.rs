use std::io::{self, BufRead, IsTerminal, Write};

use clap::Parser;

mod expr;
mod format;
mod parse;

#[derive(Parser)]
#[command(version = shared::VERSION, about = "Timestamp converter and time arithmetic")]
struct Cli {
    /// Timestamp or expression (e.g. "now", "1709740800", "now + 90d", "2024-12-25 - now")
    expr: Vec<String>,

    /// Output format: rfc3339, iso8601, epoch, epoch-ms, epoch-us, epoch-ns,
    /// relative, or strftime pattern (e.g. "%Y-%m-%d")
    #[arg(short, long, default_value = "rfc3339")]
    output: String,
}

fn run() -> Result<(), String> {
    let cli = Cli::parse();
    let fmt = format::parse_output_format(&cli.output)?;
    let now = parse::Timestamp::now();

    if cli.expr.is_empty() {
        if io::stdin().is_terminal() {
            // No args, interactive: show current time
            let result = expr::ExprResult::Time(now);
            let out = format::format_result(&result, &fmt, now)?;
            println!("{}", out);
        } else {
            // Read from stdin
            let stdin = io::stdin();
            let stdout = io::stdout();
            let mut out = stdout.lock();
            for line in stdin.lock().lines() {
                let line = line.map_err(|e| e.to_string())?;
                let line = line.trim();
                if line.is_empty() {
                    continue;
                }
                let result = expr::eval_expr(line)?;
                let formatted = format::format_result(&result, &fmt, now)?;
                if writeln!(out, "{}", formatted).is_err() {
                    break; // broken pipe
                }
            }
        }
    } else {
        let input = cli.expr.join(" ");
        let result = expr::eval_expr(&input)?;
        let out = format::format_result(&result, &fmt, now)?;
        println!("{}", out);
    }

    Ok(())
}

fn main() {
    if let Err(e) = run() {
        eprintln!("when: {}", e);
        std::process::exit(1);
    }
}
