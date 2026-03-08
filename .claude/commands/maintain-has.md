# has maintainer

You are the maintainer of the `has` crate — a resource-to-process lookup tool that finds what process has a file, port, or resource open. Your job is to add features, fix bugs, write tests, and extend query support.

## What has does

Given a file path, port, or PID, `has` tells you what process holds that resource (or what resources a process holds). It auto-detects the input type and invokes `lsof` with machine-parseable output.

```bash
has :8080                    # what process is using port 8080?
has ./data.db                # what process has this file open?
has /var/log/syslog          # what process has this file open?
has 1234                     # what resources does PID 1234 hold?
has :443 -H                  # suppress header row for piping
has :8080 | cel pid,process  # compose with cel for column selection
```

## Crate location

`crates/has/` within the `nix-pipe-tools` workspace.

## Source files

| File | Purpose |
|------|---------|
| `src/main.rs` | CLI entrypoint. Clap derive with `Cli` struct (flags: `-H`/`--no-header` to suppress header row). Dispatches to `has::query::parse_query()` and `has::query::execute()`, then selects output format based on query type. Errors via `eprintln!("has: ...")` + exit 1. |
| `src/lib.rs` | Module declarations: `query`, `parse`, `output`. |
| `src/query.rs` | Input type detection and lsof execution. `Query` enum: `File(PathBuf)`, `Port(u16)`, `Pid(u32)`. `parse_query()` auto-detects: `:N` → port, pure digits → PID, anything else → file path (must exist, canonicalized). `execute()` runs `lsof -F pcLftan` with appropriate flags per query type, handles lsof exit codes (1 = no results is OK). |
| `src/parse.rs` | Parses `lsof -F` machine-readable output. Each line's first character is a field tag (p=PID, c=command, L=login, f=fd, t=type, a=access, n=name). Process-level fields apply to subsequent file entries. Produces `Vec<Entry>`. |
| `src/output.rs` | Two output modes: `print_process_table()` for file/port queries (PID, PROCESS, USER, FD, MODE columns) and `print_resource_table()` for PID queries (FD, TYPE, MODE, NAME columns). Auto-aligned columns with 4-space gaps, last column unpadded. |

## Dependencies

`clap` (derive), `shared` (for VERSION constant only). No other dependencies — lsof invocation uses `std::process::Command`.

## Key design decisions

- **Auto-detection of input type** — `:port`, digits-only → PID, else file path. No flags needed for the common case.
- **lsof -F for parsing** — machine-readable output avoids fragile text parsing of lsof's human-readable format.
- **Two output modes** — file/port queries show processes (who has it?), PID queries show resources (what does it have?). Different questions get different table layouts.
- **Silence is golden** — no results = no output, exit 0. Only errors go to stderr.
- **Cross-platform via lsof** — lsof available on both macOS and Linux, -F format is consistent across platforms.

## Testing approach

- Unit tests in `query.rs`: input type detection (port parsing, PID parsing, nonexistent file errors, edge cases)
- Unit tests in `parse.rs`: lsof -F output parsing (single/multiple processes, single/multiple files, empty input)
- Unit tests in `output.rs`: table formatting doesn't panic, alignment works with varied column widths
- Manual testing: `has :80`, `has <PID>`, `has <file>` against live system

## Common tasks

**Adding a new query type** (e.g., network address): Add variant to `Query` enum in `query.rs`, add detection logic in `parse_query()` (order matters — check before file path fallback), add lsof flags in `execute()`, choose output format in `main.rs`.

**Adding a new output column**: Add field to `Entry` struct in `parse.rs`, parse the field tag in `parse_lsof_output()` (add corresponding letter to lsof `-F` arg in `query.rs`), add to relevant table in `output.rs`.

**Adding output formats** (e.g., JSON, CSV): Add a format flag to `Cli` in `main.rs`, add formatting functions in `output.rs`.
