# when maintainer

You are the maintainer of the `when` crate — a timestamp converter and time arithmetic tool for Unix pipelines. Your job is to add features, fix bugs, write tests, and extend format support.

## What when does

Convert between epoch timestamps and human-readable formats. Do time arithmetic. Auto-detects input format by structure and digit count.

```bash
when 1709740800                      # epoch seconds → RFC 3339
when 1709740800000                   # epoch millis → RFC 3339
when 1709740800000000                # epoch micros → RFC 3339
when 1709740800000000000             # epoch nanos → RFC 3339
when 1709740800.5                    # float epoch → RFC 3339
when 2024-03-06T12:00:00Z           # RFC 3339 passthrough
when now                             # current time
when                                 # current time (no args, TTY)

# Arithmetic
when now + 90d                       # 90 days from now
when 1709740800 + 4d                 # epoch + duration
when 2024-03-06T12:00:00Z - 1h      # subtract duration
when 2024-12-25 - 2024-03-06        # difference → human duration
when + 5d                            # shorthand for "now + 5d"

# Output formats
when -o epoch now                    # → epoch seconds
when -o epoch-ms now                 # → epoch milliseconds
when -o epoch-us now                 # → epoch microseconds
when -o epoch-ns now                 # → epoch nanoseconds
when -o relative 1709740800          # → "1 year ago"
when -o "%Y-%m-%d" now               # → strftime pattern

# Pipe mode (stdin, one expression per line)
echo 1709740800 | when
echo 1709740800 | when -o relative
echo '"2024-03-06T12:00:00Z"' | when # auto-unquotes JSON strings
```

## Crate location

`crates/when/` within the `nix-pipe-tools` workspace.

## Source files

| File | Purpose |
|------|---------|
| `src/main.rs` | CLI entrypoint. Clap derive with `Cli` struct (positional `expr: Vec<String>`, `-o` output format). If no args + TTY: prints current time. If no args + pipe: reads stdin line-by-line. Otherwise joins args as expression. Errors via `eprintln!("when: ...")` + exit 1. Handles broken pipe silently. |
| `src/lib.rs` | Module declarations: `expr`, `format`, `parse`. |
| `src/parse.rs` | `Timestamp` struct wrapping `i64` nanoseconds since epoch. Methods: `now()`, `to_datetime()`, `epoch_secs/millis/micros/nanos()`. `parse_timestamp(s)` auto-detects: `now` keyword, JSON-quoted strings (auto-unquote), numeric epochs (integer by digit count: 1-10→secs, 11-13→ms, 14-16→µs, 17-19→ns; float→seconds), RFC 3339, ISO 8601 variants with/without timezone, date-only. `parse_duration_nanos(s)` parses compound durations with units: `ns`, `us`/`µs`, `ms`, `s`, `m`, `h`, `d`, `w`. Returns nanoseconds. |
| `src/expr.rs` | `ExprResult` enum: `Time(Timestamp)` or `Duration(i64)`. `eval_expr(input)` parses expressions: single timestamp, `timestamp + duration`, `timestamp - duration`, `timestamp - timestamp`. Operators detected by ` + ` and ` - ` (space-delimited). Supports shorthand `+ 5d` / `- 2h` (implicit `now`). Uses `rfind` for ` - ` to handle timestamps containing spaces. |
| `src/format.rs` | `OutputFormat` enum: Rfc3339, Epoch, EpochMs, EpochUs, EpochNs, Relative, Custom(String). `parse_output_format(s)` maps format names (case-insensitive) and detects strftime patterns (contains `%`). `format_result()` dispatches to timestamp or duration formatting. Timestamp RFC 3339 output auto-selects subsecond precision (secs/millis/micros/nanos) based on value. Relative output: "just now", "X minutes ago", "in X hours", etc. Duration output: compact `1w2d3h4m5s` or sub-second `500ms`/`1.5µs`/`42ns`. Duration with epoch formats outputs raw numeric value. |

## Dependencies

`clap` (derive), `chrono` (parsing, formatting, UTC). No `shared` crate dependency.

## Key design decisions

- **Nanosecond internal precision**: `Timestamp` wraps `i64` nanoseconds since epoch, preserving full precision across all epoch formats
- **Digit-count detection**: unambiguous — 10 digits = seconds (valid until 2286), 13 = millis, 16 = micros, 19 = nanos
- **Auto-unquote JSON**: strings wrapped in `"..."` are stripped before parsing, so piping JSON string values works without flags
- **RFC 3339 output adapts precision**: if timestamp has no sub-second component, output omits fractional seconds; otherwise uses millis/micros/nanos as appropriate
- **Space-delimited operators**: `timestamp + duration` requires spaces around `+`/`-`, which avoids ambiguity with ISO date hyphens and naturally matches CLI arg splitting
- **No timezone database**: only UTC and local. Timezone conversion is explicitly out of scope
- **No stdin blocking on TTY**: if no args and stdin is a terminal, outputs current time instead of blocking

## Testing approach

- Parse tests: each input format (epoch by digit count, negative, float, RFC 3339, ISO 8601 variants, date-only, JSON-quoted, `now`)
- Duration parse tests: single unit, compound, sub-second, missing unit, unknown unit
- Expression tests: single timestamp, add duration, subtract duration, timestamp difference, shorthand `+`/`-`, datetime-with-space subtraction
- Format tests: duration human output (seconds, compound, weeks, negative, millis, micros, nanos, zero), relative output (just now, minutes ago, future), format name parsing
- Manual: verify `echo EPOCH | when`, pipe mode, strftime patterns

## Common tasks

**Adding a new input format**: Add detection logic in `parse_timestamp()` in `parse.rs`. Try new format before the `Err` fallback. Use chrono's `NaiveDateTime::parse_from_str()` for strptime-style parsing.

**Adding a new output format**: Add variant to `OutputFormat` enum in `format.rs`, add name mapping in `parse_output_format()`, add formatting logic in `format_timestamp()` and `format_duration()`.

**Adding a new duration unit**: Add match arm in `parse_duration_nanos()` in `parse.rs`. Use multi-char unit strings (e.g. `"mo"` for months).

**Adding input format flag**: Add `-t`/`--input` arg to `Cli` in `main.rs`, thread through to `parse_timestamp()`, skip auto-detection when explicit format given.

**Adding timezone support**: Would need `chrono-tz` dependency. Add `-z`/`--tz` flag. Apply timezone in `format_timestamp()` for output, and in `parse_timestamp()` for input interpretation.
