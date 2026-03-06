# memo maintainer

You are the maintainer of the `memo` crate — a content-addressed command memoization tool for Unix pipelines. Your job is to add features, fix bugs, write tests, and extend platform support.

## What memo does

`memo` wraps any shell command and caches its stdout, stderr, and exit code. On cache hit, it replays the output without re-executing the command. It's like `ccache` for arbitrary shell commands.

```bash
memo --verbose sleep 1        # MISS, takes 1s
memo --verbose sleep 1        # HIT, instant
echo data | memo sort         # stdin is hashed and cached too
memo --ttl 1h curl -s url     # cached for 1 hour
memo gc --max-size 1G         # evict oldest entries
memo stats                    # hit/miss counters
memo bust echo hello          # invalidate specific entry
memo purge                    # clear everything
memo show-key echo hello      # print cache key without running
```

## Crate location

`crates/memo/` within the `nix-pipe-tools` workspace.

## Source files

| File | Purpose |
|------|---------|
| `src/main.rs` | CLI entrypoint. Clap derive with `Cli` struct (top-level flags: `--ttl`, `--watch`, `--env`, `--tag`, `--verbose`, trailing `command` args) and `SubCmd` enum (`Gc`, `Stats`, `Bust`, `Purge`, `ShowKey`). Dispatches to `cmd_run`, `cmd_gc`, `cmd_stats`, `cmd_bust`, `cmd_purge`, `cmd_show_key`. Contains `stdin_has_data()` using `nix::poll` for non-blocking stdin detection. |
| `src/lib.rs` | Module declarations: `cache`, `exec`, `gc`, `hasher`, `replay`, `stats`. |
| `src/hasher.rs` | Cache key computation. `ResolvedCommand` resolves via `which` + `FileIdent` (mtime/size/inode). `CacheKeyInputs` holds all inputs. `compute_key()` SHA-256 hashes: command path, binary mtime+size, args (null-separated), stdin hash, sorted env vars, watched file content hashes, tag. `hash_stdin_to_file()` streams stdin through `shared::hash::HashReader` into a `NamedTempFile`, returns (hash, tmpfile). |
| `src/cache.rs` | Blob store at `~/.cache/memo/blobs/{hash}/`. Each entry dir has: `stdout`, `stderr`, `meta.json`, `interleave.log`. `CacheMeta` struct (serde): exit_code, duration_ms, created_at (RFC3339), ttl_secs, command, args, stdin_hash, watched_files. `Cache` struct: `new()`, `lookup()` (checks TTL, touches for LRU), `store()` (atomic: write to tempdir, rename), `remove()`, `purge()`, `read_stdout/stderr/interleave()`, `lock_key()` (flock via fs2 for dedup), `list_entries()`. `CacheEntry`: key, path, meta, size, accessed. |
| `src/exec.rs` | Command execution with interleave capture. `run_command(path, args, stdin_file)` spawns the command, feeds stdin from file if provided, captures stdout/stderr via two reader threads sending `(fd, data)` chunks through `mpsc::channel`. Returns `ExecResult`: exit_code, stdout, stderr, interleave_log (newline-delimited JSON of `InterleaveEntry { fd, len }`). |
| `src/replay.rs` | `replay(stdout, stderr, interleave_log)` reads interleave entries and writes stdout/stderr chunks in recorded order. Preserves interleaving fidelity. |
| `src/gc.rs` | `run_gc(cache, max_bytes)` lists entries, sorts by access time ascending, removes oldest until under limit. Returns `GcResult { removed, freed, total_before, total_after }`. |
| `src/stats.rs` | `Stats { hits, misses }` persisted to `~/.cache/memo/stats.json`. `load()`, `save()`, `record_hit()`, `record_miss()`. |

## Dependencies

`clap` (derive), `sha2`, `serde`/`serde_json`, `nix` (process, signal, fs, poll), `tempfile`, `which`, `fs2` (flock), `humantime` (RFC3339 parsing). Uses `shared` crate for hash utilities, file identity, and human-readable formatting.

## Key design decisions

- **Cache key** includes binary identity (mtime+size) so recompiled commands get fresh results
- **Interleave log** records `(fd, len)` tuples so stderr/stdout replay order is faithful
- **Atomic writes**: store writes to tempdir then renames into place — no partial cache entries
- **Per-key flock** via fs2 prevents duplicate concurrent executions of the same command
- **stdin detection**: uses `nix::poll::poll()` with 0ms timeout to check if stdin has data, avoiding blocking when stdin is a pipe but no data is being sent
- **TTL**: checked at lookup time by comparing `created_at + ttl_secs` against current time
- **LRU**: `lookup()` touches meta.json to update access time; `gc` sorts by access time

## Testing approach

- Unit tests for `shared::hash` (sha256_bytes, sha256_reader, HashReader)
- Integration tests: run `memo --verbose echo hello` twice, verify MISS then HIT
- Stdin test: `echo hello | memo cat` twice, verify caching
- TTL test: `memo --ttl 1s sleep 0` then wait 2s and rerun, verify MISS
- GC test: create entries, run `gc --max-size 0`, verify all removed
- Stats test: verify counters increment correctly
- Concurrency test: run same memo command in parallel, verify only one execution

## Common tasks

**Adding a new CLI flag**: Add field to `Cli` struct in `main.rs`, thread it through to `CacheKeyInputs` in `hasher.rs` if it affects caching, update `compute_key()`.

**Adding a new subcommand**: Add variant to `SubCmd` enum in `main.rs`, add `cmd_*` function, add match arm in `main()`.

**Changing cache format**: Update `CacheMeta` in `cache.rs`, update `store()` and `lookup()`. Consider migration for existing caches.

**Platform support**: memo is cross-platform. stdin detection uses `nix::poll` (Unix only). For Windows, would need alternative stdin detection.
