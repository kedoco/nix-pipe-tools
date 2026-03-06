# cel maintainer

You are the maintainer of the `cel` crate — a universal tabular text column extractor that auto-detects input format. Your job is to add features, fix bugs, write tests, and extend format support.

## What cel does

Pipe any tabular text through `cel`, select columns by name or number, get clean output. It auto-detects CSV, TSV, markdown, ASCII-aligned tables, and box-drawing tables.

```bash
docker ps | cel name,status                   # auto-detect ASCII table, extract by header
kubectl get pods | cel name,status,age         # case-insensitive header matching
ps aux | cel pid,command -w '%cpu > 5.0'       # filter rows
cat data.csv | cel email,name -o json          # CSV in, JSON out
cat README.md | cel 2,3                        # markdown table
docker ps | cel name,status -o csv             # ASCII table in, CSV out
ps aux | cel 1,11-                             # ranges: column 1 and 11 to end
docker ps | cel -x container_id                # exclude columns
docker ps | cel -l                             # list detected columns
cel -o box < data.csv                          # CSV in, box-drawing table out
```

## Crate location

`crates/cel/` within the `unix-pipe-tools` workspace.

## Source files

| File | Purpose |
|------|---------|
| `src/main.rs` | CLI entrypoint. Clap derive with `Cli` struct (flags: `-t` input format, `-o` output format, `-x` exclude, `-w` filter, `-l` list, `--no-header`, `--header`, `--case-sensitive`). `run()` orchestrates: read stdin, detect format, parse, apply filters, apply column selection, write output. Errors via `eprintln!("cel: ...")` + exit 1. |
| `src/lib.rs` | Module declarations: `detect`, `parse`, `selector`, `extract`, `filter`, `output`. |
| `src/detect.rs` | `Format` enum: Markdown, BoxDrawing, Ascii, Tsv, Csv, Whitespace. `detect(lines)` checks first 64 lines in priority order: markdown (pipes + separator line) → box-drawing (Unicode borders) → ASCII-aligned (header gaps validated against data rows) → TSV (consistent tab count) → CSV (consistent comma count, accounting for quotes) → whitespace fallback. `from_str_opt()` parses `-t` flag values. |
| `src/parse.rs` | Per-format parsers all producing `Table { headers: Vec<String>, rows: Vec<Vec<String>> }`. CSV parser: hand-rolled state machine handling quoted fields and escaped quotes. TSV: split on tabs. Markdown: strip pipes, skip separator lines. Box-drawing: strip `│`/`|` delimiters, skip border lines. ASCII-aligned: detect column start positions from 2+ space gaps in header, validated against data rows (60% threshold), last column extends to EOL. Whitespace: split on whitespace runs. First row always becomes headers. |
| `src/selector.rs` | Parses comma-separated column selectors. `Selector` enum: `Name(String)`, `Index(usize)`, `Range(usize, Option<usize>)`. Names are normalized (lowercase, spaces/hyphens → underscores). `resolve()` maps selectors to 0-based column indices; with `exclude=true`, returns all columns NOT matching. |
| `src/extract.rs` | `extract(table, selectors, exclude)` resolves selectors against headers and produces an `Extracted { headers, rows }` with only the selected columns. |
| `src/filter.rs` | `-w` expression parser. Syntax: `column op value`. Operators: `=`, `!=`, `<`, `>`, `<=`, `>=`, `~` (regex), `!~` (regex negation). Column can be a name or 1-based index. Numeric values compared as f64; otherwise string comparison. Regexes pre-compiled. `apply_filters()` takes multiple filters (AND logic) and returns matching rows. |
| `src/output.rs` | `OutputFormat` enum: Table, Csv, Tsv, Json, Plain, Markdown, Ascii, Box. Table: whitespace-aligned padded columns (like `column -t`). CSV: RFC 4180 with quoting. TSV: tab-separated. JSON: array of objects (with headers) or array of arrays (without). Plain: single space between columns. Markdown: pipe-delimited with separator line. Ascii: `+---+`/`|` borders. Box: Unicode box-drawing (`┌┬┐├┼┤└┴┘│─`) with row separators. Column widths computed with `unicode-width`. |

## Dependencies

`clap` (derive), `serde`/`serde_json`, `regex` (for `-w` regex filtering), `unicode-width` (correct alignment with wide chars). No `shared` crate dependency. No external CSV library.

## Key design decisions

- **No CSV parsing library** — hand-rolled state machine handles messy real-world output better than strict RFC 4180 parsers
- **ASCII-aligned detection** uses column start positions from header gaps (2+ consecutive spaces), validated against data rows with a 60% threshold
- **Header normalization**: lowercase, replace spaces/hyphens with underscores — so `CONTAINER ID` matches selector `container_id`
- **Filters apply before column extraction** so you can filter on columns you don't select (e.g. `cel name -w 'age > 30'`)
- **Input/output format names match** (csv, tsv, markdown, ascii, box, table, plain) so cel is composable with itself
- **Reads all stdin** before processing — not a streaming tool, but tabular data is rarely huge

## Testing approach

- Unit tests in each module (15 total): selector parsing, format detection, CSV/TSV/markdown/ASCII/whitespace parsing, filter evaluation, header normalization, exclude mode
- Integration: pipe test data through the binary and verify output
- Round-trip: `cel -o <format>` piped back into `cel -t <format>` should preserve data

## Common tasks

**Adding a new input format**: Add variant to `Format` enum in `detect.rs`, add detection logic in `detect()` (respect priority order), add `from_str_opt()` mapping, add parser function in `parse.rs`, add match arm in `parse()`.

**Adding a new output format**: Add variant to `OutputFormat` enum in `output.rs`, add `parse()` mapping, add `write_*` function, add match arm in `write_output()`. Update help text for `-o` in `main.rs`.

**Adding a new filter operator**: Add variant to `Op` enum in `filter.rs`, add parsing in `parse_filter()` (two-char ops checked before single-char), add evaluation in `match_filter()`.

**Adding a new selector syntax**: Add variant to `Selector` enum in `selector.rs`, add parsing in `parse_one()`, add resolution in `resolve()`.
