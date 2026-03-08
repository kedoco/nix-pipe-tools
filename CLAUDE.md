# Unix Pipe Tools

Cargo workspace with seven crates: `shared`, `memo`, `tap`, `prv`, `cel`, `when`, `has`.

## Build & Test

```bash
cargo build --workspace
cargo test --workspace
cargo clippy --workspace
```

## Architecture

- **shared** (`crates/shared/`) — common utilities: SHA-256 hashing (`hash.rs`), file identity (`fileident.rs`), human-readable formatting/parsing (`human.rs`)
- **memo** (`crates/memo/`) — content-addressed command memoization
- **tap** (`crates/tap/`) — pipeline stage debugger / named snapshot capture
- **prv** (`crates/prv/`) — file provenance tracker via syscall tracing or shell hooks
- **cel** (`crates/cel/`) — universal tabular text column extractor (auto-detects CSV, TSV, markdown, ASCII-aligned tables)
- **when** (`crates/when/`) — timestamp converter and time arithmetic (epoch ↔ human, duration math)
- **has** (`crates/has/`) — resource-to-process lookup: find what process has a file, port, or resource open

All three tools follow Unix philosophy: each does one thing, they compose via pipes, text is the universal interface, silence is golden.

## Conventions

- Rust 2021 edition, workspace dependencies in root `Cargo.toml`
- CLI parsing via `clap` derive macros
- Errors: `memo` and `tap` use `Box<dyn Error>` / `Result<(), String>`, `prv` uses `anyhow`
- No `unwrap()` on fallible operations in production paths
- Platform-specific code uses `#[cfg(target_os = "...")]`
- All user-facing output to stderr except actual data output (stdout)
