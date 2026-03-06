# memo

Content-addressed command memoization. Wraps any shell command and caches
stdout, stderr, and exit code. Replays from cache on hit.

## Install

```
cargo install --path crates/memo
```

## Usage

```
memo [OPTIONS] COMMAND [ARGS...]
```

```
memo sleep 1                         # first run: executes (slow)
memo sleep 1                         # second run: replays from cache (instant)
memo --verbose echo hello            # shows HIT/MISS on stderr
echo data | memo sort                # stdin is hashed into cache key
memo --ttl 1h curl -s https://...    # cached for 1 hour
memo --tag v2 make build             # tag differentiates cache entries
memo --watch config.yml make build   # invalidate when file changes
memo --env LANG echo hello           # include env var in cache key
```

## Options

    -v, --verbose          print HIT/MISS to stderr
    --ttl DURATION         time-to-live (e.g. "1h", "30m", "1d")
    --watch FILE,...       files to include in cache key
    --env VAR,...          env vars to include in cache key
    --tag TAG              extra tag for cache key differentiation

## Subcommands

    memo stats                    show hit/miss counters
    memo gc --max-size 1G         evict oldest entries until under size limit
    memo bust COMMAND [ARGS...]   invalidate cache for specific command
    memo purge                    clear entire cache
    memo show-key COMMAND [ARGS...]   print cache key without executing

## How It Works

The cache key is a SHA-256 hash of:

- command binary (resolved path + mtime + size)
- arguments
- stdin content hash
- selected environment variables (--env)
- watched file contents (--watch)
- tag (--tag)

Each cached entry is stored at `~/.cache/memo/blobs/{hash}/` and contains
stdout, stderr, exit code, and an interleave log that preserves the original
ordering of stdout and stderr on replay.

Cache writes are atomic: output goes to a temporary directory first, then is
renamed into place. Per-key file locking prevents duplicate concurrent
executions of the same command.

TTL is checked at lookup time. The `gc` subcommand performs LRU eviction,
sorting entries by last access time.
