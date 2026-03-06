# when

Timestamp converter and time arithmetic for Unix pipelines.

## Install

```
cargo install --path crates/when
```

## Usage

```
when [EXPR...] [-o FORMAT]
```

With no arguments and a TTY, `when` prints the current time. With no
arguments and piped stdin, it reads timestamps line by line.

## Examples

### Convert timestamps

```
when 1709740800                          # 2024-03-06T16:00:00Z
when 1709740800000                       # epoch millis
when 1709740800000000000                 # epoch nanos
when 2024-03-06T12:00:00Z -o epoch      # 1709726400
when -o epoch-ms 1709740800             # epoch milliseconds
when -o relative 1709740800              # "1 year ago"
when -o "%Y-%m-%d" now                   # strftime pattern
```

### Arithmetic

```
when now + 90d                           # 90 days from now
when 1709740800 + 4d                     # epoch + duration
when 2024-12-25 - now                    # duration until Christmas
when 2024-12-25 - 2024-03-06            # duration between dates
when 2024-12-25 - 2024-03-06 + 42w      # chained math
when 2024-12-25 - 2024-03-06 + 42w - 2345678s
```

### Pipe mode

```
echo 1709740800 | when                   # reads from stdin
echo '"2024-03-06T12:00:00Z"' | when     # auto-unquotes JSON strings
cat timestamps.txt | when -o relative    # convert a whole file
```

## Options

| Flag | Description |
|------|-------------|
| `-o FORMAT` | Output format (default: `rfc3339`) |

## Supported input formats

| Format | Example |
|--------|---------|
| Epoch seconds | `1709740800` (10 digits) |
| Epoch milliseconds | `1709740800000` (13 digits) |
| Epoch microseconds | `1709740800000000` (16 digits) |
| Epoch nanoseconds | `1709740800000000000` (19 digits) |
| Float epoch | `1709740800.5` |
| RFC 3339 | `2024-03-06T16:00:00Z` |
| ISO 8601 | `2024-03-06T16:00:00+00:00` |
| Date only | `2024-03-06` |
| JSON-quoted string | `"1709740800"` |
| `now` | current time |

Epoch precision is auto-detected by digit count.

## Output formats

| Format | Description |
|--------|-------------|
| `rfc3339` | `2024-03-06T16:00:00Z` (default) |
| `epoch` | seconds since Unix epoch |
| `epoch-ms` | milliseconds since Unix epoch |
| `epoch-us` | microseconds since Unix epoch |
| `epoch-ns` | nanoseconds since Unix epoch |
| `relative` | human-readable ("3 hours ago", "in 5 days", "500ms ago") |
| `%...` | strftime pattern (e.g. `"%Y-%m-%d"`) |

## Duration units

| Unit | Meaning |
|------|---------|
| `ns` | nanoseconds |
| `us`, `µs` | microseconds |
| `ms` | milliseconds |
| `s` | seconds |
| `m` | minutes |
| `h` | hours |
| `d` | days |
| `w` | weeks |
