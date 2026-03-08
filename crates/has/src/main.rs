use clap::Parser;

#[derive(Parser)]
#[command(name = "has", version = shared::VERSION, about = "Find what process has a file, port, or resource open")]
struct Cli {
    /// Resource to look up: file path, :port, or PID
    resource: String,

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
    let query = has::query::parse_query(&cli.resource)?;
    let entries = has::query::execute(&query)?;

    if entries.is_empty() {
        return Ok(());
    }

    match query {
        has::query::Query::Pid(_) => has::output::print_resource_table(&entries, cli.no_header),
        _ => has::output::print_process_table(&entries, cli.no_header),
    }

    Ok(())
}
