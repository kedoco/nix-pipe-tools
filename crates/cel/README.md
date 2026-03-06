# cel

Extract columns from any tabular text. Auto-detects CSV, TSV, markdown,
ASCII-aligned, box-drawing, and whitespace-delimited tables.

## Install

```
cargo install --path crates/cel
```

## Usage

```
cel [COLUMNS] [OPTIONS]
```

Pipe any tabular text to `cel` and select columns by name or number.

```
docker ps | cel name,status
kubectl get pods | cel name,status,age
ps aux | cel pid,command -w '%cpu > 5.0'
cat data.csv | cel email,name -o json
cat README.md | cel 2,3
docker ps | cel name,status -o csv
ps aux | cel 1,11-
docker ps | cel -x container_id
docker ps | cel -l
cel -o box < data.csv
```

## Column Selectors

Columns can be selected by:

- **Name** — case-insensitive, spaces and hyphens normalized to underscores
- **1-based index** — `1`, `3`, `11`
- **Range** — `2-5` (columns 2 through 5), `3-` (column 3 to end)

## Options

| Option | Description |
|---|---|
| `-t FORMAT` | Input format override: csv, tsv, markdown, ascii, box, whitespace |
| `-o FORMAT` | Output format: table (default), csv, tsv, json, plain, markdown, ascii, box |
| `-x` | Exclude the selected columns instead of keeping them |
| `-w EXPR` | Row filter (see below) |
| `-l` | List detected columns and exit |
| `--no-header` | Treat first row as data, not headers |
| `--header NAMES` | Override header names (comma-separated) |
| `--case-sensitive` | Case-sensitive header matching |

## Filters

Filters (`-w`) apply before column extraction, so you can filter on columns
you don't select.

Syntax: `column op value`

Operators: `=`, `!=`, `<`, `>`, `<=`, `>=`, `~` (regex), `!~` (negated regex).

```
ps aux | cel pid,command -w '%cpu > 5.0'
docker ps | cel name -w 'status ~ Up'
```

## Supported Formats

Auto-detection examines the first 64 lines and applies this priority:

1. Markdown (pipe-delimited with separator row)
2. Box-drawing (Unicode box characters)
3. ASCII-aligned (column-aligned with separator lines)
4. TSV (tab-separated)
5. CSV (comma-separated)
6. Whitespace (runs of spaces)

Input and output format names match, so `cel` composes with itself:

```
docker ps | cel name,status -o csv | cel 1
```
