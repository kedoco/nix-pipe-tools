use clap::Parser;
use std::io::Read;

#[derive(Parser)]
#[command(name = "cel", about = "Extract columns from tabular text")]
struct Cli {
    /// Column selector: names, numbers, ranges (e.g. "name,status" or "1,3-5")
    columns: Option<String>,

    /// Input format override (csv, tsv, markdown, ascii, box, table, plain)
    #[arg(short = 't', long = "type")]
    format: Option<String>,

    /// Output format (table, csv, tsv, json, plain, markdown, ascii, box)
    #[arg(short = 'o', long = "output", default_value = "table")]
    output_format: String,

    /// Exclude selected columns instead of including them
    #[arg(short = 'x', long = "exclude")]
    exclude: bool,

    /// Row filter expressions (multiple = AND)
    #[arg(short = 'w', long = "where")]
    filter: Vec<String>,

    /// Input has no header row
    #[arg(long = "no-header")]
    no_header: bool,

    /// Override header names (comma-separated)
    #[arg(long = "header")]
    header: Option<String>,

    /// List detected columns and exit
    #[arg(short = 'l', long = "list")]
    list_columns: bool,

    /// Case-sensitive header matching
    #[arg(long = "case-sensitive")]
    case_sensitive: bool,
}

fn main() {
    let cli = Cli::parse();
    if let Err(e) = run(cli) {
        eprintln!("cel: {}", e);
        std::process::exit(1);
    }
}

fn run(cli: Cli) -> Result<(), String> {
    // Read all stdin
    let mut input = String::new();
    std::io::stdin()
        .read_to_string(&mut input)
        .map_err(|e| format!("reading stdin: {}", e))?;

    if input.trim().is_empty() {
        return Ok(());
    }

    // Detect or override format
    let format = if let Some(ref fmt) = cli.format {
        cel::detect::Format::from_str_opt(fmt)?
    } else {
        let sample_lines: Vec<&str> = input.lines().take(64).collect();
        cel::detect::detect(&sample_lines)
    };

    // Parse input
    let mut table = cel::parse::parse(&input, format)?;

    // Override headers if requested
    if let Some(ref header_str) = cli.header {
        table.headers = header_str.split(',').map(|s| s.trim().to_string()).collect();
    }

    // List columns mode
    if cli.list_columns {
        if table.headers.is_empty() {
            let num_cols = table.rows.first().map_or(0, |r| r.len());
            for i in 1..=num_cols {
                println!("{}  col{}", i, i);
            }
        } else {
            for (i, h) in table.headers.iter().enumerate() {
                println!("{}  {}", i + 1, h);
            }
        }
        return Ok(());
    }

    // Apply filters first (against original column names)
    let rows = if cli.filter.is_empty() {
        table.rows
    } else {
        let filters: Vec<cel::filter::Filter> = cli
            .filter
            .iter()
            .map(|f| cel::filter::parse_filter(f))
            .collect::<Result<Vec<_>, _>>()?;
        cel::filter::apply_filters(&filters, &table.headers, table.rows)?
    };

    // If no column selector, output everything
    let Some(ref columns) = cli.columns else {
        let out_fmt = cel::output::OutputFormat::parse(&cli.output_format)?;
        return cel::output::write_output(&table.headers, &rows, &out_fmt, cli.no_header);
    };

    // Parse selectors and extract columns
    let selectors = cel::selector::parse(columns)?;
    let filtered_table = cel::parse::Table {
        headers: table.headers,
        rows,
    };
    let extracted = cel::extract::extract(&filtered_table, &selectors, cli.exclude)?;

    let out_fmt = cel::output::OutputFormat::parse(&cli.output_format)?;
    cel::output::write_output(&extracted.headers, &extracted.rows, &out_fmt, cli.no_header)
}
