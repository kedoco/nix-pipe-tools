# nix-pipe-tools

Small, composable Unix utilities for macOS and Linux. Each tool does one thing, they connect via pipes, and text is the universal interface.

## Tools

| Tool | Description |
|------|-------------|
| [memo](crates/memo/) | Content-addressed command memoization |
| [tap](crates/tap/) | Pipeline stage debugger / named snapshot capture |
| [prv](crates/prv/) | File provenance tracker via syscall tracing or shell hooks |
| [cel](crates/cel/) | Universal tabular text column extractor |
| [when](crates/when/) | Timestamp converter and time arithmetic |

## Quick start

```bash
# Build all tools
cargo build --workspace --release

# Install all tools to ~/.cargo/bin
make install

# Or install individually
cargo install --path crates/when
```

## Examples

```bash
# Cache a slow command
memo curl -s https://api.example.com/data

# Snapshot pipeline stages for debugging
cat data.csv | tap -n raw | sort -k2 | tap -n sorted | head -20

# Track which files a build reads and writes
prv wrap make -j8
prv deps output.bin

# Extract columns from any tabular text
docker ps | cel name,status -o csv
ps aux | cel pid,command -w '%cpu > 5.0'

# Convert timestamps and do time math
when 1709740800                    # epoch → 2024-03-06T16:00:00Z
when now + 90d -o "%Y-%m-%d"      # 90 days from now
when 2024-12-25 - now              # duration until date
```

## Build

```bash
cargo build --workspace           # debug build
cargo build --workspace --release # release build
cargo test --workspace            # run all tests
cargo clippy --workspace          # lint
```

## Install

```bash
make install     # copies release binaries to ~/.cargo/bin
make uninstall   # removes them
```

Or install individual tools:

```bash
cargo install --path crates/memo
cargo install --path crates/tap
cargo install --path crates/prv
cargo install --path crates/cel
cargo install --path crates/when
```

## License

MIT
