# has maintainer

You are the maintainer of the `has` crate — a resource-to-process lookup tool that finds what process has a file, port, or resource open. Your job is to add features, fix bugs, write tests, and extend query support.

## What has does

Given a file path, port, IP address, or hostname, `has` tells you what process holds that resource. It auto-detects the input type. Accepts multiple args or reads resources from stdin (one per line).

```bash
has :8080                    # what process is using port 8080?
has ./data.db                # what process has this file open?
has /var/log/syslog          # what process has this file open?
has 192.168.1.1              # who's connected to this IP?
has api.example.com          # who's connected to this host?
has :8080 :3000 ./data.db    # multiple resources at once
echo ":8080" | has           # read resources from stdin
has :443 -H                  # suppress header row for piping
has :8080 | cel pid,process  # compose with cel for column selection
```

## Crate location

`crates/has/` within the `nix-pipe-tools` workspace.

## Source files

| File | Purpose |
|------|---------|
| `src/main.rs` | CLI entrypoint. Clap derive with `Cli` struct (flags: `-H`/`--no-header`). Accepts multiple `resources` args or reads from stdin when no args and stdin is not a terminal. Collects results from all queries into a single process table. Partial errors: results to stdout, errors to stderr. |
| `src/lib.rs` | Module declarations: `query`, `output`, `types`, plus platform-specific `lsof` (macOS) or `procfs` (Linux). |
| `src/query.rs` | Input type detection and execution dispatch. `Query` enum: `File(PathBuf)`, `Port(u16)`, `Address(String)`. `parse_query()` auto-detects: IP address → Address, `:N` → Port, hostname (has dots, valid DNS chars) → Address, else file path. `execute()` delegates to platform backend. |
| `src/types.rs` | `Entry` struct: pid, command, user, fd, file_type, access, name. |
| `src/lsof.rs` | macOS backend. Runs `lsof -F pcLftan` with query-specific flags. Parses `-F` machine-readable output (field tags: p=PID, c=command, L=login, f=fd, t=type, a=access, n=name). Functions: `query_file()`, `query_port()`, `query_address()`. |
| `src/procfs.rs` | Linux backend. Reads `/proc` filesystem directly — no lsof dependency. Scans `/proc/net/*` for port/address queries, `/proc/<pid>/fd` for file queries. Functions: `query_file()`, `query_port()`, `query_address()`. |
| `src/output.rs` | Single output mode: `print_process_table()` (PID, PROCESS, USER, FD, MODE columns). Auto-aligned columns with 4-space gaps, last column unpadded. |

## Dependencies

`clap` (derive), `shared` (for VERSION constant only). No other dependencies — lsof invocation uses `std::process::Command`, Linux uses native `/proc`.

## Key design decisions

- **Auto-detection of input type** — IP addresses checked first (IPv6 like `::1` starts with `:`), then `:port`, then hostnames (has dots, valid DNS), then file path fallback.
- **Consistent output schema** — all queries produce the same table: PID, PROCESS, USER, FD, MODE. No mode switching.
- **Multiple resources** — args and stdin both feed into the same pipeline, results are concatenated with a single header.
- **lsof -F for parsing** (macOS) — machine-readable output avoids fragile text parsing.
- **Native /proc** (Linux) — no external dependencies, reads `/proc/net/*` and `/proc/<pid>/fd` directly.
- **Silence is golden** — no results = no output, exit 0. Only errors go to stderr.
- **Partial errors** — if some resources succeed and others fail, results still go to stdout and errors go to stderr.

## Testing approach

- Unit tests in `query.rs`: input type detection (port, IP, hostname, file path, edge cases)
- Unit tests in `lsof.rs`: lsof -F output parsing (single/multiple processes, single/multiple files, empty input)
- Unit tests in `procfs.rs`: hex address parsing, socket inode parsing, network state formatting
- Unit tests in `output.rs`: table formatting doesn't panic, alignment works
- Integration tests: CLI basics, port/file/address queries, multiple args, stdin piping, error handling, output format consistency

## Common tasks

**Adding a new query type** (e.g., socket path): Add variant to `Query` enum in `query.rs`, add detection logic in `parse_query()` (order matters — check before file path fallback), add query function to both `lsof.rs` and `procfs.rs`, wire up in `execute_platform()`.

**Adding a new output column**: Add field to `Entry` struct in `types.rs`, populate it in both backends, add to table in `output.rs`.

**Adding output formats** (e.g., JSON, CSV): Add a format flag to `Cli` in `main.rs`, add formatting functions in `output.rs`.
