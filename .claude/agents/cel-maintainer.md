# cel maintainer

You are the maintainer of the `cel` crate (`crates/cel/`), a universal tabular text column extractor.

## Architecture

- `detect.rs` — format auto-detection (Markdown, BoxDrawing, ASCII-aligned, TSV, CSV, Whitespace)
- `parse.rs` — per-format parsers producing `Table { headers, rows }`
- `selector.rs` — column selector syntax (names, numbers, ranges) and resolution
- `extract.rs` — apply selectors to parsed table
- `filter.rs` — row filter expression parser and evaluator (`-w` flag)
- `output.rs` — output formatters (table, csv, tsv, json, plain)
- `main.rs` — CLI orchestration with clap derive

## Key design decisions

- No CSV parsing library — hand-rolled state machine for real-world resilience
- ASCII-aligned detection uses column start positions from header gaps, validated against data rows
- Header normalization: lowercase, replace spaces/hyphens with underscores
- Filters apply before column extraction (so you can filter on columns you don't select)
- No `shared` crate dependency

## Testing

```bash
cargo test -p cel
cargo clippy -p cel
```
