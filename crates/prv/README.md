# prv

File provenance tracker. Records which commands read and wrote which files,
via syscall tracing or shell hooks.

## Install

```
cargo install --path crates/prv
```

## Usage

### Tracking commands

**Syscall tracing** -- precise, captures all file I/O. Requires `strace` on
Linux or `sudo fs_usage` on macOS.

```
prv wrap make -j8
prv wrap gcc -o output input.c
```

**Shell hooks** -- zero overhead, heuristic, no root required. Works on any
platform with bash or zsh.

```
eval "$(prv init)"
# run commands normally; prv records them on each prompt
```

Force a specific shell with `prv init --bash` or `prv init --zsh`.
Without flags, prv auto-detects from `$SHELL`.

### Querying provenance

```
prv log output.bin              # command history for a file
prv deps output.bin             # input files that produced this
prv rdeps input.c               # files that depend on this input
prv trace output.bin            # full event trace with reads/writes
prv search "gcc"                # search command history by pattern
```

### Graphs and replay

```
prv dot output.bin              # graphviz DOT dependency graph
prv dot output.bin --mermaid    # mermaid format
prv dot output.bin | dot -Tpng -o deps.png

prv replay output.bin --dry-run # show commands to reproduce a file
prv replay output.bin           # execute them (topologically sorted)
```

### Maintenance

```
prv gc --older-than 90d         # remove records older than 90 days
prv gc --older-than 24h         # or 24 hours
```

## Subcommands

| Command | Description |
|---|---|
| `wrap COMMAND [ARGS...]` | Trace command via strace/fs_usage |
| `init [--zsh\|--bash]` | Output shell hook code for eval |
| `record COMMAND... --exit-code N` | Record a command from shell hook (internal) |
| `log FILE` | Show command history for a file |
| `trace FILE` | Show full event trace |
| `deps FILE` | Show input dependencies |
| `rdeps FILE` | Show reverse dependencies |
| `replay FILE [--dry-run]` | Replay commands to reproduce a file |
| `dot FILE [--mermaid]` | Dependency graph in DOT or Mermaid format |
| `search PATTERN` | Search command history |
| `gc --older-than DURATION` | Clean old records |

## Configuration

Config file: `~/.config/prv/config.toml`

Use ignore patterns to skip files you don't care about (temporary files,
build artifacts, etc.).

## Storage

SQLite database with WAL mode at `~/.local/share/prv/prv.db`.

## Platform support

- **Linux**: `strace` for `wrap`, shell hooks everywhere
- **macOS**: `fs_usage` (requires sudo) for `wrap`, shell hooks everywhere
