use std::io::{self, Write};

use unicode_width::UnicodeWidthStr;

pub enum OutputFormat {
    Table,
    Csv,
    Tsv,
    Json,
    Plain,
    Markdown,
    Ascii,
    Box,
}

impl OutputFormat {
    pub fn parse(s: &str) -> Result<OutputFormat, String> {
        match s.to_lowercase().as_str() {
            "table" => Ok(OutputFormat::Table),
            "csv" => Ok(OutputFormat::Csv),
            "tsv" => Ok(OutputFormat::Tsv),
            "json" => Ok(OutputFormat::Json),
            "plain" => Ok(OutputFormat::Plain),
            "markdown" | "md" => Ok(OutputFormat::Markdown),
            "ascii" => Ok(OutputFormat::Ascii),
            "box" => Ok(OutputFormat::Box),
            _ => Err(format!("unknown output format: {}", s)),
        }
    }
}

pub fn write_output(
    headers: &[String],
    rows: &[Vec<String>],
    format: &OutputFormat,
    no_header: bool,
) -> Result<(), String> {
    let stdout = io::stdout();
    let mut out = stdout.lock();

    match format {
        OutputFormat::Table => write_table(&mut out, headers, rows, no_header),
        OutputFormat::Csv => write_csv(&mut out, headers, rows, no_header),
        OutputFormat::Tsv => write_tsv(&mut out, headers, rows, no_header),
        OutputFormat::Json => write_json(&mut out, headers, rows, no_header),
        OutputFormat::Plain => write_plain(&mut out, headers, rows, no_header),
        OutputFormat::Markdown => write_markdown(&mut out, headers, rows, no_header),
        OutputFormat::Ascii => write_ascii(&mut out, headers, rows, no_header),
        OutputFormat::Box => write_box(&mut out, headers, rows, no_header),
    }
}

fn write_table(
    out: &mut impl Write,
    headers: &[String],
    rows: &[Vec<String>],
    no_header: bool,
) -> Result<(), String> {
    let num_cols = headers.len().max(rows.first().map_or(0, |r| r.len()));
    if num_cols == 0 {
        return Ok(());
    }

    // Calculate column widths
    let mut widths = vec![0usize; num_cols];
    if !no_header {
        for (i, h) in headers.iter().enumerate() {
            widths[i] = widths[i].max(UnicodeWidthStr::width(h.as_str()));
        }
    }
    for row in rows {
        for (i, cell) in row.iter().enumerate() {
            if i < num_cols {
                widths[i] = widths[i].max(UnicodeWidthStr::width(cell.as_str()));
            }
        }
    }

    let write_row = |out: &mut dyn Write, cells: &[String]| -> Result<(), String> {
        let last = num_cols - 1;
        for (i, width) in widths.iter().enumerate().take(num_cols) {
            let cell = cells.get(i).map(|s| s.as_str()).unwrap_or("");
            if i == last {
                // No trailing padding on last column
                write!(out, "{}", cell).map_err(|e| e.to_string())?;
            } else {
                let cell_width = UnicodeWidthStr::width(cell);
                let padding = width.saturating_sub(cell_width);
                write!(out, "{}{}  ", cell, " ".repeat(padding))
                    .map_err(|e| e.to_string())?;
            }
        }
        writeln!(out).map_err(|e| e.to_string())
    };

    if !no_header && !headers.is_empty() {
        write_row(out, headers)?;
    }
    for row in rows {
        write_row(out, row)?;
    }
    Ok(())
}

fn write_csv(
    out: &mut impl Write,
    headers: &[String],
    rows: &[Vec<String>],
    no_header: bool,
) -> Result<(), String> {
    let write_csv_row = |out: &mut dyn Write, cells: &[String]| -> Result<(), String> {
        let formatted: Vec<String> = cells.iter().map(|c| csv_quote(c)).collect();
        writeln!(out, "{}", formatted.join(",")).map_err(|e| e.to_string())
    };

    if !no_header && !headers.is_empty() {
        write_csv_row(out, headers)?;
    }
    for row in rows {
        write_csv_row(out, row)?;
    }
    Ok(())
}

fn csv_quote(s: &str) -> String {
    if s.contains(',') || s.contains('"') || s.contains('\n') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

fn write_tsv(
    out: &mut impl Write,
    headers: &[String],
    rows: &[Vec<String>],
    no_header: bool,
) -> Result<(), String> {
    if !no_header && !headers.is_empty() {
        writeln!(out, "{}", headers.join("\t")).map_err(|e| e.to_string())?;
    }
    for row in rows {
        writeln!(out, "{}", row.join("\t")).map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn write_json(
    out: &mut impl Write,
    headers: &[String],
    rows: &[Vec<String>],
    no_header: bool,
) -> Result<(), String> {
    if no_header || headers.is_empty() {
        // Array of arrays
        let data: Vec<&Vec<String>> = rows.iter().collect();
        let json = serde_json::to_string_pretty(&data).map_err(|e| e.to_string())?;
        writeln!(out, "{}", json).map_err(|e| e.to_string())
    } else {
        // Array of objects
        let data: Vec<serde_json::Map<String, serde_json::Value>> = rows
            .iter()
            .map(|row| {
                let mut map = serde_json::Map::new();
                for (i, h) in headers.iter().enumerate() {
                    let val = row.get(i).cloned().unwrap_or_default();
                    map.insert(h.clone(), serde_json::Value::String(val));
                }
                map
            })
            .collect();
        let json = serde_json::to_string_pretty(&data).map_err(|e| e.to_string())?;
        writeln!(out, "{}", json).map_err(|e| e.to_string())
    }
}

fn write_plain(
    out: &mut impl Write,
    headers: &[String],
    rows: &[Vec<String>],
    no_header: bool,
) -> Result<(), String> {
    if !no_header && !headers.is_empty() {
        writeln!(out, "{}", headers.join(" ")).map_err(|e| e.to_string())?;
    }
    for row in rows {
        writeln!(out, "{}", row.join(" ")).map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn write_markdown(
    out: &mut impl Write,
    headers: &[String],
    rows: &[Vec<String>],
    no_header: bool,
) -> Result<(), String> {
    let num_cols = headers.len().max(rows.first().map_or(0, |r| r.len()));
    if num_cols == 0 {
        return Ok(());
    }

    let widths = column_widths(headers, rows, num_cols, no_header);

    let write_md_row = |out: &mut dyn Write, cells: &[String]| -> Result<(), String> {
        write!(out, "|").map_err(|e| e.to_string())?;
        for (i, width) in widths.iter().enumerate().take(num_cols) {
            let cell = cells.get(i).map(|s| s.as_str()).unwrap_or("");
            let cell_width = UnicodeWidthStr::width(cell);
            let padding = width.saturating_sub(cell_width);
            write!(out, " {}{} |", cell, " ".repeat(padding)).map_err(|e| e.to_string())?;
        }
        writeln!(out).map_err(|e| e.to_string())
    };

    if !no_header && !headers.is_empty() {
        write_md_row(out, headers)?;
        // Separator line
        write!(out, "|").map_err(|e| e.to_string())?;
        for width in widths.iter().take(num_cols) {
            write!(out, " {} |", "-".repeat(*width)).map_err(|e| e.to_string())?;
        }
        writeln!(out).map_err(|e| e.to_string())?;
    }
    for row in rows {
        write_md_row(out, row)?;
    }
    Ok(())
}

fn write_ascii(
    out: &mut impl Write,
    headers: &[String],
    rows: &[Vec<String>],
    no_header: bool,
) -> Result<(), String> {
    let num_cols = headers.len().max(rows.first().map_or(0, |r| r.len()));
    if num_cols == 0 {
        return Ok(());
    }

    let widths = column_widths(headers, rows, num_cols, no_header);

    let write_border = |out: &mut dyn Write| -> Result<(), String> {
        write!(out, "+").map_err(|e| e.to_string())?;
        for (i, width) in widths.iter().enumerate().take(num_cols) {
            write!(out, "{}", "-".repeat(width + 2)).map_err(|e| e.to_string())?;
            if i + 1 < num_cols {
                write!(out, "+").map_err(|e| e.to_string())?;
            }
        }
        writeln!(out, "+").map_err(|e| e.to_string())
    };

    let write_data_row = |out: &mut dyn Write, cells: &[String]| -> Result<(), String> {
        write!(out, "|").map_err(|e| e.to_string())?;
        for (i, width) in widths.iter().enumerate().take(num_cols) {
            let cell = cells.get(i).map(|s| s.as_str()).unwrap_or("");
            let cell_width = UnicodeWidthStr::width(cell);
            let padding = width.saturating_sub(cell_width);
            write!(out, " {}{} ", cell, " ".repeat(padding)).map_err(|e| e.to_string())?;
            if i + 1 < num_cols {
                write!(out, "|").map_err(|e| e.to_string())?;
            }
        }
        writeln!(out, "|").map_err(|e| e.to_string())
    };

    write_border(out)?;
    if !no_header && !headers.is_empty() {
        write_data_row(out, headers)?;
        write_border(out)?;
    }
    for row in rows {
        write_data_row(out, row)?;
    }
    write_border(out)?;
    Ok(())
}

fn write_box(
    out: &mut impl Write,
    headers: &[String],
    rows: &[Vec<String>],
    no_header: bool,
) -> Result<(), String> {
    let num_cols = headers.len().max(rows.first().map_or(0, |r| r.len()));
    if num_cols == 0 {
        return Ok(());
    }

    let widths = column_widths(headers, rows, num_cols, no_header);

    let write_border = |out: &mut dyn Write, left: &str, mid: &str, right: &str| -> Result<(), String> {
        write!(out, "{}", left).map_err(|e| e.to_string())?;
        for (i, width) in widths.iter().enumerate().take(num_cols) {
            write!(out, "{}", "─".repeat(width + 2)).map_err(|e| e.to_string())?;
            if i + 1 < num_cols {
                write!(out, "{}", mid).map_err(|e| e.to_string())?;
            }
        }
        writeln!(out, "{}", right).map_err(|e| e.to_string())
    };

    let write_data_row = |out: &mut dyn Write, cells: &[String]| -> Result<(), String> {
        write!(out, "│").map_err(|e| e.to_string())?;
        for (i, width) in widths.iter().enumerate().take(num_cols) {
            let cell = cells.get(i).map(|s| s.as_str()).unwrap_or("");
            let cell_width = UnicodeWidthStr::width(cell);
            let padding = width.saturating_sub(cell_width);
            write!(out, " {}{} ", cell, " ".repeat(padding)).map_err(|e| e.to_string())?;
            if i + 1 < num_cols {
                write!(out, "│").map_err(|e| e.to_string())?;
            }
        }
        writeln!(out, "│").map_err(|e| e.to_string())
    };

    write_border(out, "┌", "┬", "┐")?;
    if !no_header && !headers.is_empty() {
        write_data_row(out, headers)?;
        write_border(out, "├", "┼", "┤")?;
    }
    for (i, row) in rows.iter().enumerate() {
        write_data_row(out, row)?;
        if i + 1 < rows.len() {
            write_border(out, "├", "┼", "┤")?;
        }
    }
    write_border(out, "└", "┴", "┘")?;
    Ok(())
}

fn column_widths(
    headers: &[String],
    rows: &[Vec<String>],
    num_cols: usize,
    no_header: bool,
) -> Vec<usize> {
    let mut widths = vec![0usize; num_cols];
    if !no_header {
        for (i, h) in headers.iter().enumerate() {
            widths[i] = widths[i].max(UnicodeWidthStr::width(h.as_str()));
        }
    }
    for row in rows {
        for (i, cell) in row.iter().enumerate() {
            if i < num_cols {
                widths[i] = widths[i].max(UnicodeWidthStr::width(cell.as_str()));
            }
        }
    }
    widths
}
