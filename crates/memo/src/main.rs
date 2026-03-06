use clap::{Parser, Subcommand};
use memo::cache::{Cache, CacheMeta};
use memo::exec;
use memo::gc;
use memo::hasher::{self, CacheKeyInputs, ResolvedCommand};
use memo::replay;
use memo::stats::Stats;
use shared::human;
use std::path::PathBuf;
use std::process;

#[derive(Parser)]
#[command(name = "memo", about = "Command memoization for Unix pipelines")]
struct Cli {
    #[command(subcommand)]
    subcmd: Option<SubCmd>,

    /// Time-to-live for cached result (e.g. "1h", "30m", "1d")
    #[arg(long)]
    ttl: Option<String>,

    /// Files to watch for changes (comma-separated)
    #[arg(long, value_delimiter = ',')]
    watch: Vec<PathBuf>,

    /// Environment variables to include in cache key (comma-separated)
    #[arg(long, value_delimiter = ',')]
    env: Vec<String>,

    /// Extra tag to include in cache key
    #[arg(long)]
    tag: Option<String>,

    /// Print HIT/MISS status to stderr
    #[arg(long, short)]
    verbose: bool,

    /// Command and arguments to memoize
    #[arg(trailing_var_arg = true)]
    command: Vec<String>,
}

#[derive(Subcommand)]
enum SubCmd {
    /// Garbage collect old cache entries
    Gc {
        /// Maximum total cache size (e.g. "1G", "500M")
        #[arg(long)]
        max_size: String,
    },
    /// Show hit/miss statistics
    Stats,
    /// Invalidate cache for a specific command
    Bust {
        /// Command and arguments
        #[arg(trailing_var_arg = true, required = true)]
        command: Vec<String>,
    },
    /// Clear entire cache
    Purge,
    /// Show the cache key that would be used
    ShowKey {
        /// Command and arguments
        #[arg(trailing_var_arg = true, required = true)]
        command: Vec<String>,
    },
}

fn main() {
    let cli = Cli::parse();

    let result = match cli.subcmd {
        Some(SubCmd::Gc { max_size }) => cmd_gc(&max_size),
        Some(SubCmd::Stats) => cmd_stats(),
        Some(SubCmd::Bust { command }) => cmd_bust(&command, &cli.env, &cli.watch, cli.tag.as_deref()),
        Some(SubCmd::Purge) => cmd_purge(),
        Some(SubCmd::ShowKey { command }) => {
            cmd_show_key(&command, &cli.env, &cli.watch, cli.tag.as_deref())
        }
        None => {
            if cli.command.is_empty() {
                eprintln!("memo: no command specified");
                process::exit(1);
            }
            cmd_run(&cli)
        }
    };

    if let Err(e) = result {
        eprintln!("memo: {}", e);
        process::exit(1);
    }
}

fn cmd_run(cli: &Cli) -> Result<(), Box<dyn std::error::Error>> {
    let command_name = &cli.command[0];
    let args: Vec<String> = cli.command[1..].to_vec();

    let cache = Cache::new()?;
    let resolved = ResolvedCommand::resolve(command_name)?;

    // Parse TTL
    let ttl_duration = cli
        .ttl
        .as_ref()
        .map(|s| human::parse_duration(s))
        .transpose()
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidInput, e))?;

    // Check if stdin has data to hash.
    // We use a non-blocking poll: if stdin is a TTY, skip. If it's a pipe,
    // peek to see if there's data before committing to reading all of it.
    use std::io::IsTerminal;
    let (stdin_hash, stdin_file) = if !std::io::stdin().is_terminal() && stdin_has_data() {
        let (hash, tmp) = hasher::hash_stdin_to_file(cache.root())?;
        (Some(hash), Some(tmp))
    } else {
        (None, None)
    };

    // Gather env vars
    let env_vars: Vec<(String, String)> = cli
        .env
        .iter()
        .filter_map(|k| std::env::var(k).ok().map(|v| (k.clone(), v)))
        .collect();

    let inputs = CacheKeyInputs {
        resolved: &resolved,
        args: &args,
        stdin_hash: stdin_hash.as_deref(),
        env_vars: &env_vars,
        watched_files: &cli.watch,
        tag: cli.tag.as_deref(),
    };

    let key = hasher::compute_key(&inputs)?;

    // Acquire lock for dedup
    let _lock = cache.lock_key(&key)?;

    // Check cache
    if let Some(meta) = cache.lookup(&key) {
        if cli.verbose {
            eprintln!("memo: HIT (cached {}ms ago)", meta.duration_ms);
        }
        let _ = Stats::record_hit();

        let stdout = cache.read_stdout(&key)?;
        let stderr = cache.read_stderr(&key)?;
        let interleave = cache.read_interleave(&key)?;
        replay::replay(&stdout, &stderr, &interleave)?;
        process::exit(meta.exit_code);
    }

    if cli.verbose {
        eprintln!("memo: MISS");
    }
    let _ = Stats::record_miss();

    // Execute the command
    let stdin_path = stdin_file.as_ref().map(|f| f.path());
    let result = exec::run_command(&resolved.path, &args, stdin_path)?;

    // Build metadata
    let now = humantime::format_rfc3339(std::time::SystemTime::now()).to_string();
    let meta = CacheMeta {
        exit_code: result.exit_code,
        duration_ms: result.duration_ms,
        created_at: now,
        ttl_secs: ttl_duration.map(|d| d.as_secs()),
        command: command_name.clone(),
        args: args.clone(),
        stdin_hash,
        watched_files: cli.watch.iter().map(|p| p.to_string_lossy().to_string()).collect(),
    };

    // Store in cache
    cache.store(
        &key,
        &meta,
        &result.stdout,
        &result.stderr,
        &result.interleave_log,
    )?;

    // Replay output
    replay::replay(&result.stdout, &result.stderr, &result.interleave_log)?;
    process::exit(result.exit_code);
}

/// Check if stdin has data available using poll(2).
fn stdin_has_data() -> bool {
    use nix::poll::{poll, PollFd, PollFlags, PollTimeout};
    use std::os::unix::io::AsRawFd;
    let stdin_fd = std::io::stdin().as_raw_fd();
    // Safety: we're borrowing the fd only for the poll call
    let fd = unsafe { std::os::unix::io::BorrowedFd::borrow_raw(stdin_fd) };
    let mut fds = [PollFd::new(fd, PollFlags::POLLIN)];
    match poll(&mut fds, PollTimeout::from(0u8)) {
        Ok(n) => n > 0,
        Err(_) => false,
    }
}

fn cmd_gc(max_size: &str) -> Result<(), Box<dyn std::error::Error>> {
    let max_bytes =
        human::parse_size(max_size).map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidInput, e))?;
    let cache = Cache::new()?;
    let result = gc::run_gc(&cache, max_bytes)?;
    eprintln!(
        "memo gc: removed {} entries, freed {}, {} -> {}",
        result.removed,
        human::format_bytes(result.freed),
        human::format_bytes(result.total_before),
        human::format_bytes(result.total_after),
    );
    Ok(())
}

fn cmd_stats() -> Result<(), Box<dyn std::error::Error>> {
    let stats = Stats::load()?;
    let total = stats.hits + stats.misses;
    let rate = if total > 0 {
        (stats.hits as f64 / total as f64) * 100.0
    } else {
        0.0
    };
    println!("hits:    {}", stats.hits);
    println!("misses:  {}", stats.misses);
    println!("total:   {}", total);
    println!("hit rate: {:.1}%", rate);
    Ok(())
}

fn cmd_bust(
    command: &[String],
    env_keys: &[String],
    watched_files: &[PathBuf],
    tag: Option<&str>,
) -> Result<(), Box<dyn std::error::Error>> {
    let command_name = &command[0];
    let args = &command[1..];
    let key = hasher::compute_key_for_command(command_name, args, env_keys, watched_files, tag)?;
    let cache = Cache::new()?;
    if cache.remove(&key)? {
        eprintln!("memo: busted cache for key {}", &key[..12]);
    } else {
        eprintln!("memo: no cache entry found");
    }
    Ok(())
}

fn cmd_purge() -> Result<(), Box<dyn std::error::Error>> {
    let cache = Cache::new()?;
    cache.purge()?;
    eprintln!("memo: cache purged");
    Ok(())
}

fn cmd_show_key(
    command: &[String],
    env_keys: &[String],
    watched_files: &[PathBuf],
    tag: Option<&str>,
) -> Result<(), Box<dyn std::error::Error>> {
    let command_name = &command[0];
    let args = &command[1..];
    let key = hasher::compute_key_for_command(command_name, args, env_keys, watched_files, tag)?;
    println!("{}", key);
    Ok(())
}
