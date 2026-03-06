use clap::{Parser, Subcommand};
use prv::config::Config;
use prv::db::Database;
use std::path::Path;

#[derive(Parser)]
#[command(name = "prv", version, about = "File provenance tracker")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Trace a command and record file accesses
    Wrap {
        /// Command to run
        command: String,
        /// Arguments to pass
        #[arg(trailing_var_arg = true)]
        args: Vec<String>,
    },
    /// Output shell hook code for eval
    Init {
        /// Generate zsh hooks
        #[arg(long)]
        zsh: bool,
        /// Generate bash hooks
        #[arg(long)]
        bash: bool,
    },
    /// Record a command from shell hook
    Record {
        /// The command string
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        command_string: Vec<String>,
        /// Exit code of the command
        #[arg(long)]
        exit_code: Option<i32>,
    },
    /// Show provenance log for a file
    Log {
        /// File to query
        file: String,
    },
    /// Show full trace for a file
    Trace {
        /// File to query
        file: String,
    },
    /// Show dependencies (inputs) of a file
    Deps {
        /// File to query
        file: String,
    },
    /// Show reverse dependencies (outputs) of a file
    Rdeps {
        /// File to query
        file: String,
    },
    /// Replay commands to reproduce a file
    Replay {
        /// Target file to reproduce
        file: String,
        /// Only print commands, don't execute
        #[arg(long)]
        dry_run: bool,
    },
    /// Generate dependency graph in DOT or Mermaid format
    Dot {
        /// File to graph
        file: String,
        /// Output Mermaid format instead of DOT
        #[arg(long)]
        mermaid: bool,
    },
    /// Search commands by pattern
    Search {
        /// Search pattern
        pattern: String,
    },
    /// Garbage-collect old records
    Gc {
        /// Remove records older than this duration (e.g. "7d", "24h")
        #[arg(long)]
        older_than: String,
    },
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let config = Config::load();

    match cli.command {
        Commands::Wrap { command, args } => {
            let db = Database::open()?;
            #[cfg(target_os = "linux")]
            {
                let code = prv::trace_linux::wrap_command(&command, &args, &db, &config)?;
                std::process::exit(code);
            }
            #[cfg(target_os = "macos")]
            {
                let code = prv::trace_macos::wrap_command(&command, &args, &db, &config)?;
                std::process::exit(code);
            }
            #[cfg(not(any(target_os = "linux", target_os = "macos")))]
            {
                let _ = (&command, &args, &db, &config);
                anyhow::bail!("wrap is only supported on Linux and macOS");
            }
        }
        Commands::Init { zsh, bash } => {
            if bash {
                print!("{}", prv::shell_hook::generate_bash_hook());
            } else if zsh {
                print!("{}", prv::shell_hook::generate_zsh_hook());
            } else {
                // Auto-detect
                let shell = std::env::var("SHELL").unwrap_or_default();
                if shell.contains("bash") {
                    print!("{}", prv::shell_hook::generate_bash_hook());
                } else {
                    print!("{}", prv::shell_hook::generate_zsh_hook());
                }
            }
        }
        Commands::Record {
            command_string,
            exit_code,
        } => {
            let db = Database::open()?;
            let cmd = command_string.join(" ");
            prv::shell_hook::record_command(&cmd, exit_code, &db, &config)?;
        }
        Commands::Log { file } => {
            let db = Database::open()?;
            let path = resolve_file_path(&file);
            let entries = db.log_for_file(&path)?;
            if entries.is_empty() {
                println!("No provenance records for {}", path);
            } else {
                for (cmd, event) in &entries {
                    println!(
                        "{} {} {} ({})",
                        event.timestamp, event.event_type, cmd.command, event.path
                    );
                    if !cmd.args.is_empty() && cmd.args != "[]" {
                        println!("  args: {}", cmd.args);
                    }
                    if let Some(exit) = cmd.exit_code {
                        println!("  exit: {}", exit);
                    }
                }
            }
        }
        Commands::Trace { file } => {
            let db = Database::open()?;
            let path = resolve_file_path(&file);
            let entries = db.all_events_for_file(&path)?;
            if entries.is_empty() {
                println!("No trace records for {}", path);
            } else {
                for (cmd, events) in &entries {
                    println!("[{}] {} {}", cmd.timestamp, cmd.command, cmd.args);
                    println!("  cwd: {}", cmd.cwd);
                    for ev in events {
                        println!("  {} {}", ev.event_type, ev.path);
                    }
                }
            }
        }
        Commands::Deps { file } => {
            let db = Database::open()?;
            let path = resolve_file_path(&file);
            let deps = db.deps_for_file(&path)?;
            if deps.is_empty() {
                println!("No known dependencies for {}", path);
            } else {
                for dep in &deps {
                    println!("{}", dep);
                }
            }
        }
        Commands::Rdeps { file } => {
            let db = Database::open()?;
            let path = resolve_file_path(&file);
            let rdeps = db.rdeps_for_file(&path)?;
            if rdeps.is_empty() {
                println!("No known reverse dependencies for {}", path);
            } else {
                for rdep in &rdeps {
                    println!("{}", rdep);
                }
            }
        }
        Commands::Replay { file, dry_run } => {
            let db = Database::open()?;
            let path = resolve_file_path(&file);
            let steps = prv::replay::plan_replay(&db, &path)?;
            if steps.is_empty() {
                println!("No replay steps found for {}", path);
            } else {
                println!("Replay plan ({} steps):", steps.len());
                prv::replay::execute_replay(&steps, dry_run)?;
            }
        }
        Commands::Dot { file, mermaid } => {
            let db = Database::open()?;
            let path = resolve_file_path(&file);
            let (graph, _nodes) = prv::graph::build_graph(&db, &path)?;
            if mermaid {
                print!("{}", prv::graph::to_mermaid(&graph));
            } else {
                print!("{}", prv::graph::to_dot(&graph));
            }
        }
        Commands::Search { pattern } => {
            let db = Database::open()?;
            let results = db.search_commands(&pattern)?;
            if results.is_empty() {
                println!("No commands matching '{}'", pattern);
            } else {
                for cmd in &results {
                    println!(
                        "[{}] {} {} (exit: {})",
                        cmd.timestamp,
                        cmd.command,
                        cmd.args,
                        cmd.exit_code.map_or("?".into(), |c| c.to_string())
                    );
                }
            }
        }
        Commands::Gc { older_than } => {
            let duration = shared::human::parse_duration(&older_than)
                .map_err(|e| anyhow::anyhow!(e))?;
            let db = Database::open()?;
            let deleted = db.gc_older_than(duration)?;
            println!("Removed {} file event records", deleted);
        }
    }

    Ok(())
}

fn resolve_file_path(file: &str) -> String {
    let path = Path::new(file);
    if path.is_absolute() {
        file.to_string()
    } else {
        std::env::current_dir()
            .unwrap_or_default()
            .join(file)
            .to_string_lossy()
            .to_string()
    }
}
