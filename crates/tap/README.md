# tap

Pipeline stage debugger. Captures named snapshots at any point in a Unix
pipeline. Data passes through at near-cat speed.

## Install

```
cargo install --path crates/tap
```

## Usage

Insert `tap -n <name>` between pipe stages to snapshot data flowing through:

```
seq 1000 | tap -n nums | sort -rn | tap -n sorted | head -5
```

Without `-n`, tap is a pure passthrough (zero overhead).

Capture never blocks the pipeline. A bounded channel with `try_send` drops
chunks if the capture thread can't keep up.

## Capture options

```
tap [-n NAME] [-s] [-l LINES] [-b BYTES] [-f]
```

| Flag | Description |
|------|-------------|
| `-n NAME` | Capture point name (required for capture) |
| `-s` | Summary mode: count lines/bytes without storing data |
| `-l LINES` | Max lines to capture (data still passes through) |
| `-b BYTES` | Max bytes to capture |
| `-f` | Auto-detect format (Json, Csv, Tsv, Xml, Text) |

## Subcommands

```
tap show NAME [-S SESSION]       # display captured data (pipes through pager)
tap diff NAME1 NAME2 [-S SESSION] # diff two captures
tap stats [-S SESSION]            # show capture stats table
tap replay NAME [-S SESSION]      # replay captured data to stdout
tap last                          # list most recent session
tap sessions                      # list all sessions
tap clean --older-than DURATION   # remove old sessions
```

## Examples

```
# Capture and inspect
seq 1000 | tap -n nums | sort -rn | tap -n sorted | head -5
tap stats
tap show nums
tap diff nums sorted

# Replay into another pipeline
tap replay sorted | other-cmd

# Housekeeping
tap last
tap sessions
tap clean --older-than 1d
```

## Sessions and storage

A session groups all taps from the same parent process within a 60-second
window. Names resolve across all sessions; the most recent match wins.
Use `-S SESSION` on any subcommand to pin to a specific session.

Data is stored at:

```
/tmp/tap-{USER}/sessions/{ppid}-{epoch}/{name}.data
```
