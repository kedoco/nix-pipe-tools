# prv maintainer

You are the maintainer of the `prv` crate — a lightweight file provenance tracker that records which commands read and wrote which files. Your job is to add features, fix bugs, write tests, and extend platform support.

## What prv does

`prv` tracks file dependencies by recording command executions and their file accesses. It supports two modes: syscall tracing (`prv wrap`) for precise tracking and shell hooks (`prv init`) for zero-overhead heuristic tracking.

```bash
# Precise tracing (Linux: strace, macOS: sudo fs_usage)
prv wrap make -j8

# Shell hook mode (no root required)
eval "$(prv init)"
# ...run commands normally, they're recorded automatically...

# Query provenance
prv log output.bin              # command history for this file
prv deps output.bin             # what files were read to produce this
prv rdeps input.c               # what files depend on this input
prv trace output.bin            # full event trace
prv dot output.bin              # graphviz DOT dependency graph
prv dot output.bin --mermaid    # mermaid format
prv replay output.bin --dry-run # show commands to reproduce this file
prv search "gcc"                # search command history
prv gc --older-than 90d         # clean old records
```

## Crate location

`crates/prv/` within the `unix-pipe-tools` workspace.

## Source files

| File | Purpose |
|------|---------|
| `src/main.rs` | CLI entrypoint. Clap derive with `Commands` enum: `Wrap`, `Init`, `Record`, `Log`, `Trace`, `Deps`, `Rdeps`, `Replay`, `Dot`, `Search`, `Gc`. Uses `anyhow` for error handling. `resolve_file_path()` helper canonicalizes relative paths. Platform-conditional dispatch for `wrap` via `#[cfg(target_os)]`. |
| `src/lib.rs` | Module declarations. `trace_linux` and `trace_macos` are conditionally compiled with `#[cfg(target_os)]`. Always-present modules: `config`, `db`, `graph`, `replay`, `shell_hook`. |
| `src/config.rs` | `Config` struct with `ignore_patterns: Vec<String>`. Default patterns: `/tmp/*`, `/proc/*`, `/dev/*`, `/sys/*`, `.git/objects/*`, `node_modules/*`. Loads from `~/.config/prv/config.toml`, creates default if missing. `should_ignore(path)` checks against glob patterns. |
| `src/db.rs` | SQLite database at `~/.local/share/prv/prv.db` with WAL mode. Schema: `commands` table (id, command, args JSON, cwd, timestamp RFC3339, duration_ms, exit_code) and `file_events` table (id, command_id FK, path, event_type, timestamp). Indexes on `file_events(path)` and `file_events(command_id)`. Structs: `CommandRecord`, `FileEvent`. Methods: `insert_command() -> i64`, `insert_file_event()`, `log_for_file()` (joins commands+events for a path), `deps_for_file()` (input files: reads from commands that wrote this file), `rdeps_for_file()` (output files: writes from commands that read this file), `all_events_for_file()`, `producers_for_file()` (commands+their read deps that wrote a file), `search_commands()` (LIKE search), `gc_older_than()` (deletes by timestamp). |
| `src/trace_linux.rs` | `wrap_command(cmd, args, db, config)` spawns `strace -f -e trace=openat,creat,rename,renameat,renameat2,unlink,unlinkat -o {tmpfile} {cmd}`. Parses output line-by-line with `parse_strace_line()`: extracts paths from quoted strings, classifies by syscall and flags (O_WRONLY/O_RDWR/O_CREAT -> write/create, else read). Skips `<unfinished`/`resumed>` lines. `extract_quoted_string()` and `extract_last_quoted_string()` helpers for parsing strace output. |
| `src/trace_macos.rs` | `wrap_command(cmd, args, db, config)` spawns the command as a child, then runs `sudo fs_usage -w -f filesys {pid}` to monitor it. After command exits, SIGTERM's fs_usage and parses its stdout. `parse_fs_usage_line()` detects open/create/rename/unlink keywords, classifies R/W. `extract_path()` finds absolute paths in the line. Note: requires sudo. |
| `src/shell_hook.rs` | `generate_zsh_hook()` / `generate_bash_hook()` return eval-able shell code. Zsh: `preexec` saves command, `precmd` calls `prv record`. Bash: `DEBUG` trap saves command, `PROMPT_COMMAND` calls `prv record`. `record_command(cmd_str, exit_code, db, config)` does heuristic parsing: `parse_command()` splits with basic quoting support, then iterates tokens looking for redirections (`>`, `>>`, `<`, `>file`, `<file`), existing file paths, and flags (skipped). `classify_file_access()` heuristic: cp/mv/sed/tee -> write, rm/unlink -> delete, else read. |
| `src/graph.rs` | `build_graph(db, file)` constructs a `petgraph::DiGraph<String, String>` from file events. Nodes are file paths, edges are commands (read -> write). Returns graph + node map. `to_dot()` produces graphviz DOT output. `to_mermaid()` produces mermaid graph output. `sanitize_mermaid_id()` replaces non-alphanumeric chars with `_`. |
| `src/replay.rs` | `plan_replay(db, target)` traces backwards from target file through `producers_for_file()`, building dependency tree. Topological sort via iterative resolution (process files whose deps are all resolved). Handles cycles by adding remaining in arbitrary order. `execute_replay(steps, dry_run)` runs commands in order or prints them. `ReplayStep`: command, args, cwd. |

## Dependencies

`clap` (derive), `rusqlite` (bundled SQLite), `serde`/`serde_json`, `toml`, `nix` (process, signal, fs), `which`, `comfy-table`, `chrono`, `petgraph`, `glob`, `anyhow`, `tempfile` (Linux trace only).

## Key design decisions

- **Two tracking modes**: syscall tracing (precise but requires strace/sudo) and shell hooks (zero overhead, heuristic, no root)
- **SQLite with WAL**: concurrent-safe, single-file database, no server needed
- **Path canonicalization**: `resolve_path()` in shell_hook.rs makes relative paths absolute using cwd
- **Ignore patterns**: configurable in `~/.config/prv/config.toml`, defaults filter noise like /tmp, /proc, .git/objects
- **Heuristic file detection**: shell hooks can't see actual syscalls, so they parse command strings for file arguments and check if paths exist on disk
- **Dependency queries use JOIN**: `deps_for_file` finds reads from commands that also wrote the target; `rdeps_for_file` finds writes from commands that also read the source
- **Topological replay**: `plan_replay` traces the full dependency tree and sorts commands so inputs are produced before they're needed

## Database schema

```sql
commands(id, command, args, cwd, timestamp, duration_ms, exit_code)
file_events(id, command_id, path, event_type, timestamp)
-- event_type: read, write, create, delete, rename
-- Indexes: idx_file_events_path, idx_file_events_cmd
```

## Testing approach

- DB tests: create in-memory database, insert commands/events, verify queries
- Shell hook tests: `record_command("cat input.txt > output.txt", ...)`, verify correct events recorded
- Parse tests: `parse_command()` with various quoting/escaping scenarios
- Config tests: verify `should_ignore()` against patterns
- Graph tests: insert known dependency chain, verify DOT/mermaid output
- Replay tests: insert multi-step chain, verify topological order
- strace parser tests: feed sample strace output lines, verify extracted paths and event types
- Integration: `prv wrap sh -c 'cat input > output'` then verify `prv deps output` shows input

## Common tasks

**Adding a new query command**: Add variant to `Commands` enum in `main.rs`, add match arm, implement query in `db.rs` if it needs new SQL, or compose existing methods.

**Adding a new event type**: Update `event_type` values in `trace_linux.rs`/`trace_macos.rs`/`shell_hook.rs`. Update `classify_file_access()` if needed. May need new patterns in `graph.rs` edge construction.

**Improving shell hook heuristics**: Edit `record_command()` and `classify_file_access()` in `shell_hook.rs`. The `parse_command()` function handles basic quoting — enhance it for more shell syntax.

**Adding a new trace backend**: Create `src/trace_{platform}.rs` following the pattern of `trace_linux.rs`: function signature `wrap_command(cmd, args, db, config) -> anyhow::Result<i32>`, add `#[cfg]` gate in `lib.rs` and `main.rs`.

**Schema migrations**: Currently uses `CREATE TABLE IF NOT EXISTS` in `db.rs::migrate()`. For additive changes, add new `CREATE` statements. For breaking changes, add migration versioning.

**Platform support**: Linux fully supported via strace. macOS supported via fs_usage (needs sudo) and shell hooks (no root). Windows would need a new trace backend (possibly ETW). Shell hooks work anywhere with bash/zsh.
