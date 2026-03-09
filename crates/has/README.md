# has

Find what process has a file, port, or resource open.

## Install

```
cargo install --path crates/has
```

## Usage

```
has [RESOURCE...] [OPTIONS]
```

Give `has` a file path, port, IP address, or hostname and it tells you what process is using it. Pass multiple resources as arguments, or pipe them via stdin — one per line.

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

### Find connections to an address

```
$ has 192.168.1.1
PID     PROCESS    USER     FD    MODE
99854   chrome     kevin    26    u

$ has api.example.com
PID     PROCESS    USER     FD    MODE
1492    curl       kevin    5     u
```

### Multiple resources at once

```
$ has :8080 :3000 ./data.db
PID     PROCESS    USER     FD    MODE
18429   node       kevin    14    u
5521    rails      kevin    9     u
1234    python     kevin    3     rw
```

### Pipe resources from stdin

```
# Find who has ports 80 and 443
echo -e ":80\n:443" | has

# Compose with other tools
some-command | has
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
| IPv4/IPv6 address | Network address | `has 192.168.1.1`, `has ::1` |
| `:N` | Port number | `has :8080` |
| `host.name` | Hostname | `has api.example.com` |
| Anything else | File path | `has ./data.db` |

## Output

All queries produce the same table format:

| Column | Description |
|--------|-------------|
| PID | Process ID |
| PROCESS | Command name |
| USER | Login name of process owner |
| FD | File descriptor number |
| MODE | Access mode: `r` read, `w` write, `u` read/write |

## Options

| Option | Description |
|--------|-------------|
| `-H`, `--no-header` | Suppress the header row (useful for piping) |

## Notes

- Uses `lsof` on macOS, native `/proc` on Linux (no external dependencies)
- Some processes may require root to inspect
- No output and exit 0 when no results are found (silence is golden)
- When some resources succeed and others fail, results go to stdout and errors go to stderr
