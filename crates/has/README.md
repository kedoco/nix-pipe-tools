# has

Find what process has a file, port, or resource open.

## Install

```
cargo install --path crates/has
```

## Usage

```
has <RESOURCE> [OPTIONS]
```

Give `has` a file path, port, or PID and it tells you what's using it.

## Examples

### Find what's using a port

```
$ has :8080
PID     PROCESS    USER     FD    MODE
18429   node       kevin    14    u

$ has :443
PID     PROCESS    USER     FD    MODE
8219    nginx      root     6     u
8220    nginx      www      7     u
```

### Find what has a file open

```
$ has ./data.db
PID     PROCESS    USER     FD    MODE
1234    python     kevin    3     rw
5678    sqlite3    kevin    5     r
```

### See what resources a process holds

```
$ has 47543
FD     TYPE      MODE    NAME
cwd    DIR               /Users/kevin/dev/myproject
txt    REG               /usr/local/bin/node
0      unix      u       ->(none)
3      KQUEUE    u       count=0, state=0xa
13     IPv4      u       localhost:9230
14     IPv4      u       localhost:8788
16     IPv4      u       localhost:9230->localhost:58176
```

### Compose with other tools

```
# Just the PIDs
has :8080 -H | awk '{print $1}'

# Use with cel for column selection
has :8080 | cel pid,process

# Kill whatever's hogging a port
has :3000 -H | awk '{print $1}' | xargs kill
```

## Input auto-detection

| Input | Detected as | Example |
|-------|-------------|---------|
| `:N` | Port number | `has :8080` |
| Digits only | PID | `has 1234` |
| Anything else | File path | `has ./data.db` |

To look up a file whose name is purely numeric, use a path prefix: `has ./1234`.

## Output

**File or port query** — shows processes holding that resource:

| Column | Description |
|--------|-------------|
| PID | Process ID |
| PROCESS | Command name |
| USER | Login name of process owner |
| FD | File descriptor number |
| MODE | Access mode: `r` read, `w` write, `u` read/write |

**PID query** — shows resources held by that process:

| Column | Description |
|--------|-------------|
| FD | File descriptor number (or `cwd`, `txt`, etc.) |
| TYPE | Resource type: REG (file), DIR, IPv4, IPv6, unix, KQUEUE, etc. |
| MODE | Access mode |
| NAME | File path, socket address, or resource description |

## Options

| Option | Description |
|--------|-------------|
| `-H`, `--no-header` | Suppress the header row (useful for piping) |

## Notes

- Requires `lsof` (pre-installed on macOS and most Linux distributions)
- Some processes may require root to inspect
- No output and exit 0 when no results are found (silence is golden)
