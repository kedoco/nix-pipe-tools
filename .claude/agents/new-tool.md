# new tool

You are creating a new Unix pipe tool in the `unix-pipe-tools` workspace. Follow the established patterns exactly.

## Workspace structure

```
unix-pipe-tools/
  Cargo.toml          # workspace root — members list, [workspace.dependencies]
  CLAUDE.md           # architecture overview — update when adding a tool
  crates/
    shared/           # common utilities (hash, fileident, human-readable formatting)
    memo/             # command memoization
    tap/              # pipeline stage debugger
    prv/              # file provenance tracker
    cel/              # tabular text column extractor
    <your-tool>/      # new tool goes here
  .claude/agents/
    memo-maintainer.md
    tap-maintainer.md
    prv-maintainer.md
    cel-maintainer.md
    <your-tool>-maintainer.md  # create one for the new tool
```

## Step-by-step process

### 1. Create the crate

```
crates/<name>/
  Cargo.toml
  src/
    main.rs   # CLI entrypoint
    lib.rs    # pub mod declarations
    ...       # one file per concern
```

### 2. Cargo.toml pattern

```toml
[package]
name = "<name>"
version.workspace = true
edition.workspace = true

[[bin]]
name = "<name>"
path = "src/main.rs"

[dependencies]
clap = { workspace = true }
# add only what you need from workspace deps
# add new deps to workspace root first
```

### 3. Register in workspace

In root `Cargo.toml`:
- Add `"crates/<name>"` to `[workspace] members`
- Add any new dependencies to `[workspace.dependencies]`

### 4. Update CLAUDE.md

- Update the crate count in the opening line
- Add a bullet to the Architecture section

### 5. Create maintainer skill

Create `.claude/agents/<name>-maintainer.md` following the pattern of existing ones. Include:
- What the tool does with usage examples
- Crate location
- Source files table with detailed descriptions of each file
- Dependencies list
- Key design decisions
- Testing approach
- Common tasks (adding features, extending behavior)

## Conventions to follow

### CLI style
- `clap` derive macros for argument parsing
- Short flags for common operations (e.g. `-o`, `-t`, `-w`, `-l`)
- Long flags with `--kebab-case`
- Help text documents all accepted values inline (e.g. `-o` lists all output formats)

### Error handling
- `Result<(), String>` with `eprintln!("<tool>: {}", e)` and `std::process::exit(1)` in `main()`
- No `unwrap()` on fallible operations in production paths
- `prv` uses `anyhow` — either style is fine, but be consistent within a crate

### I/O philosophy
- Actual data output goes to stdout
- All user-facing messages (errors, status, diagnostics) go to stderr
- Silence is golden — don't print unless there's something to say
- Tools compose via pipes: stdin in, stdout out

### Code style
- Rust 2021 edition
- One file per concern (don't stuff everything in main.rs)
- `lib.rs` is just `pub mod` declarations
- Keep `main.rs` thin — parse args, dispatch to library functions
- Platform-specific code uses `#[cfg(target_os = "...")]`
- Run `cargo clippy` and fix all warnings before committing
- Run `cargo test` — write unit tests in each module with `#[cfg(test)] mod tests`

### Dependencies
- Prefer workspace dependencies — add to root `Cargo.toml` first
- Use `shared` crate if you need hashing, file identity, or human-readable formatting
- Minimize external deps — hand-roll simple parsers rather than pulling in a library for one use case
- No `shared` dependency required — only use it if it genuinely helps

### What NOT to do
- Don't add `unwrap()` in production code paths
- Don't print to stdout for diagnostics — only data output
- Don't add dependencies you don't need
- Don't create a README.md — CLAUDE.md and the maintainer skill are the docs
- Don't over-engineer — solve the immediate problem simply

## Verification checklist

```bash
cargo build -p <name>
cargo test -p <name>
cargo clippy -p <name>       # must be clean (zero warnings)
cargo build --workspace       # make sure nothing else broke
cargo test --workspace
```

Test the tool manually with representative inputs before committing.
