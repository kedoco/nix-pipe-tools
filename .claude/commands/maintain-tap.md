# tap maintainer

You are the maintainer of the `tap` crate — a pipeline stage debugger that captures named snapshots at any point in a Unix pipeline. Your job is to add features, fix bugs, write tests, and extend platform support.

## What tap does

Insert `tap -n <name>` between pipe stages to capture a snapshot of the data flowing through. Data passes through at near-`cat` speed. Query captured data after the pipeline finishes.

```bash
seq 1000 | tap -n nums | sort -rn | tap -n sorted | head -5
tap stats                           # table of all captures
tap show nums                       # display captured data
tap diff nums sorted                # diff two captures
tap replay sorted | other-cmd       # replay into new pipeline
tap last                            # list most recent session
tap sessions                        # list all sessions
tap clean --older-than 1d           # remove old sessions
```

## Crate location

`crates/tap/` within the `nix-pipe-tools` workspace.

## Source files

| File | Purpose |
|------|---------|
| `src/main.rs` | CLI entrypoint. Clap derive with `Cli` struct (flags: `-n name`, `-s` summary, `-l lines`, `-b bytes`, `-f` format detect) and `Cmd` enum (`Show`, `Diff`, `Stats`, `Replay`, `Last`, `Sessions`, `Clean`). Capture mode in `run_capture()`: derives session ID, sets up crossbeam channel, spawns capture thread, runs passthrough relay, writes meta.json, prints summary to stderr. |
| `src/lib.rs` | Module declarations: `capture`, `detect`, `passthrough`, `query`, `session`. |
| `src/passthrough.rs` | `relay(sender: Option<&Sender<Vec<u8>>>)` — reads stdin in 64KB chunks, writes to stdout, optionally sends copies via crossbeam channel. Returns `(total_bytes, total_lines)`. Uses `try_send()` so full channel drops chunks rather than blocking the pipe. `bytecount()` helper counts newlines. |
| `src/capture.rs` | `capture_thread(rx, opts)` — background thread receiving chunks from crossbeam bounded channel (capacity 256). Writes to `{name}.data` file. Respects `max_lines`/`max_bytes` limits (stops writing but keeps draining channel). Collects first 8KB as `sample` for format detection. `CaptureOpts`: data_path, summary_only, max_lines, max_bytes. `CaptureResult`: bytes_written, lines_written, truncated, sample. |
| `src/session.rs` | Session ID = `{PPID}-{epoch_seconds}`. Reuses existing session from same PPID within 60-second window (so multiple tap instances in the same pipeline or quick re-runs share a session). Storage: `/tmp/tap-{USER}/sessions/{session_id}/{name}.data` + `{name}.meta.json`. `Meta` struct (serde): name, session_id, timestamp (RFC3339), bytes, lines, duration_secs, format (Format enum), truncated. |
| `src/detect.rs` | `Format` enum: Json, Csv, Tsv, Xml, Text. `detect_format(sample)` heuristic from first 8KB: JSON (starts with `{`/`[` and parses), XML (starts with `<`), TSV (consistent tab counts across first 5 lines), CSV (consistent comma counts), else Text. |
| `src/query.rs` | Query subcommand implementations. `latest_session()` finds most recent session by directory mtime. `resolve_session_for_name(session, name)` searches all sessions for the most recent capture with a given name. `read_metas(session_id)` reads all `.meta.json` files in a session dir. `all_metas_latest_per_name()` aggregates the most recent capture per name across all sessions. Commands: `show` (pipes through `$PAGER`/`less` if TTY), `diff` (delegates to `diff(1)`), `stats` (comfy-table; without `-S` shows all names across sessions, with `-S` scopes to one session), `replay` (cats data to stdout), `last` (lists latest session), `sessions` (lists all), `clean` (removes by mtime). |

## Dependencies

`clap` (derive), `serde`/`serde_json`, `nix` (process), `chrono`, `comfy-table`, `crossbeam-channel`. Uses `shared` crate for `format_bytes`, `format_duration`, `parse_duration`.

## Key design decisions

- **Never blocks the pipeline**: passthrough uses `try_send()` on bounded channel. If capture thread can't keep up, chunks are dropped and capture is marked truncated
- **Session reuse**: `session_id()` scans existing session dirs for same PPID within 60s window before creating a new one. This groups captures from the same pipeline or rapid re-runs
- **Name-based resolution**: `show`, `replay`, `diff` all search across sessions for captures by name (most recent wins), rather than requiring a session ID
- **Stats aggregation**: `tap stats` (no `-S`) shows the most recent capture per name across all sessions, so you always see a complete picture
- **Zero-overhead passthrough**: bare `tap` (no `-n`) just relays stdin to stdout with no capture, as fast as `cat`
- **Summary mode** (`-s`): counts lines/bytes without storing data (for huge streams)
- **Capture limits** (`-l`/`-b`): stop writing to disk but continue passing through data

## Storage layout

```
/tmp/tap-{USER}/sessions/
  {ppid}-{epoch}/
    {name}.data           # raw captured bytes
    {name}.meta.json      # Meta struct as JSON
```

## Testing approach

- Passthrough test: `echo hello | tap` should output `hello` and nothing else
- Capture test: `seq 10 | tap -n test` then verify `.data` file has correct content
- Session reuse: run two taps from same parent quickly, verify same session dir
- Stats: run pipeline with multiple taps, verify `tap stats` shows all captures
- Diff: capture two stages, run `tap diff`, verify output matches `diff(1)`
- Limits: `seq 1000 | tap -n x -l 5`, verify only 5 lines captured but all 1000 pass through
- Format detection: pipe JSON/CSV/TSV/XML, verify `-f` detects correctly
- Clean: create sessions, `tap clean --older-than 0s`, verify removed

## Common tasks

**Adding a new query command**: Add variant to `Cmd` enum in `main.rs`, add match arm dispatching to `query::new_cmd()`, implement in `query.rs`.

**Adding a new format**: Add variant to `Format` enum in `detect.rs`, add detection logic in `detect_format()`, update `Display` impl.

**Changing storage layout**: Update path functions in `session.rs` (`data_path`, `meta_path`, `session_dir`). Update `read_metas()` and `all_metas_latest_per_name()` in `query.rs`.

**Adding capture metadata**: Add field to `Meta` struct in `session.rs`, populate in `run_capture()` in `main.rs`, display in `stats()` table in `query.rs`.

**Platform support**: tap is cross-platform. Uses `/tmp` on all Unixes. For Windows, would need to change `base_dir()` in `session.rs` to use `%TEMP%`.
