use crate::detect::Format;

/// Parsed table: optional headers + rows of cells
pub struct Table {
    pub headers: Vec<String>,
    pub rows: Vec<Vec<String>>,
}

pub fn parse(input: &str, format: Format) -> Result<Table, String> {
    match format {
        Format::Csv => parse_csv(input),
        Format::Tsv => parse_tsv(input),
        Format::Markdown => parse_markdown(input),
        Format::BoxDrawing => parse_box_drawing(input),
        Format::Ascii => parse_ascii(input),
        Format::Whitespace => parse_whitespace(input),
    }
}

fn parse_csv(input: &str) -> Result<Table, String> {
    let mut rows: Vec<Vec<String>> = Vec::new();
    for line in input.lines() {
        rows.push(parse_csv_line(line));
    }
    split_header(rows)
}

fn parse_csv_line(line: &str) -> Vec<String> {
    let mut fields = Vec::new();
    let mut current = String::new();
    let mut in_quote = false;
    let mut chars = line.chars().peekable();

    while let Some(c) = chars.next() {
        match c {
            '"' if in_quote => {
                if chars.peek() == Some(&'"') {
                    chars.next();
                    current.push('"');
                } else {
                    in_quote = false;
                }
            }
            '"' if !in_quote && current.is_empty() => {
                in_quote = true;
            }
            ',' if !in_quote => {
                fields.push(current.clone());
                current.clear();
            }
            _ => current.push(c),
        }
    }
    fields.push(current);
    fields
}

fn parse_tsv(input: &str) -> Result<Table, String> {
    let mut rows: Vec<Vec<String>> = Vec::new();
    for line in input.lines() {
        rows.push(line.split('\t').map(|s| s.to_string()).collect());
    }
    split_header(rows)
}

fn parse_markdown(input: &str) -> Result<Table, String> {
    let mut rows: Vec<Vec<String>> = Vec::new();
    for line in input.lines() {
        let trimmed = line.trim();
        // Skip separator lines
        if is_md_separator(trimmed) {
            continue;
        }
        if !trimmed.contains('|') {
            continue;
        }
        let cells: Vec<String> = trimmed
            .split('|')
            .map(|s| s.trim().to_string())
            .collect::<Vec<_>>();
        // Remove empty first/last from leading/trailing pipes
        let cells = strip_empty_edges(cells);
        rows.push(cells);
    }
    split_header(rows)
}

fn is_md_separator(line: &str) -> bool {
    if !line.contains('-') {
        return false;
    }
    let without_pipes = line.replace('|', "");
    without_pipes
        .trim()
        .chars()
        .all(|c| c == '-' || c == ':' || c == ' ')
        && without_pipes.contains('-')
}

fn parse_box_drawing(input: &str) -> Result<Table, String> {
    let mut rows: Vec<Vec<String>> = Vec::new();
    for line in input.lines() {
        let trimmed = line.trim();
        // Skip border lines
        if is_box_border(trimmed) {
            continue;
        }
        // Split on | or │
        let cells: Vec<String> = split_box_line(trimmed)
            .into_iter()
            .map(|s| s.trim().to_string())
            .collect();
        let cells = strip_empty_edges(cells);
        if !cells.is_empty() {
            rows.push(cells);
        }
    }
    split_header(rows)
}

fn is_box_border(line: &str) -> bool {
    line.chars().all(|c| matches!(c, '+' | '-' | '=' | '├' | '┤' | '┬' | '┴' | '┼' | '─' | '┌' | '┐' | '└' | '┘' | '│' | ' '))
        && (line.contains('-') || line.contains('─') || line.contains('='))
}

fn split_box_line(line: &str) -> Vec<String> {
    let mut result = Vec::new();
    let mut current = String::new();
    for c in line.chars() {
        if c == '|' || c == '│' {
            result.push(current.clone());
            current.clear();
        } else {
            current.push(c);
        }
    }
    result.push(current);
    result
}

fn parse_ascii(input: &str) -> Result<Table, String> {
    let lines: Vec<&str> = input.lines().collect();
    if lines.is_empty() {
        return Ok(Table {
            headers: Vec::new(),
            rows: Vec::new(),
        });
    }

    let header_line = lines[0];
    let col_starts = find_column_starts(header_line, &lines[1..]);

    let extract_cells = |line: &str| -> Vec<String> {
        let mut cells = Vec::new();
        for (i, &start) in col_starts.iter().enumerate() {
            let end = if i + 1 < col_starts.len() {
                col_starts[i + 1]
            } else {
                line.len()
            };
            let cell = if start < line.len() {
                line[start..end.min(line.len())].trim().to_string()
            } else {
                String::new()
            };
            cells.push(cell);
        }
        cells
    };

    let headers = extract_cells(header_line);
    let rows: Vec<Vec<String>> = lines[1..]
        .iter()
        .filter(|l| !l.trim().is_empty())
        .map(|l| extract_cells(l))
        .collect();

    Ok(Table { headers, rows })
}

/// Find column start positions from header: position where non-space starts after 2+ spaces.
/// Validated against data rows.
fn find_column_starts(header: &str, data_lines: &[&str]) -> Vec<usize> {
    let bytes = header.as_bytes();
    let mut starts = Vec::new();
    let mut i = 0;
    // Skip leading spaces
    while i < bytes.len() && bytes[i] == b' ' {
        i += 1;
    }
    if i < bytes.len() {
        starts.push(i);
    }
    while i < bytes.len() {
        if bytes[i] == b' ' {
            let gap_start = i;
            while i < bytes.len() && bytes[i] == b' ' {
                i += 1;
            }
            if i - gap_start >= 2 && i < bytes.len() {
                starts.push(i);
            }
        } else {
            i += 1;
        }
    }

    if data_lines.is_empty() || starts.len() < 2 {
        return starts;
    }

    // Validate: for each start (except first), the char before it should be a space
    // in 60%+ of data rows
    let threshold = (data_lines.len() as f64 * 0.6).ceil() as usize;
    let mut validated = vec![starts[0]];
    for &start in &starts[1..] {
        let check_pos = start - 1;
        let count = data_lines
            .iter()
            .filter(|l| l.len() > check_pos && l.as_bytes()[check_pos] == b' ')
            .count();
        if count >= threshold {
            validated.push(start);
        }
    }
    validated
}

fn parse_whitespace(input: &str) -> Result<Table, String> {
    let mut rows: Vec<Vec<String>> = Vec::new();
    for line in input.lines() {
        if line.trim().is_empty() {
            continue;
        }
        let cells: Vec<String> = line.split_whitespace().map(|s| s.to_string()).collect();
        rows.push(cells);
    }
    split_header(rows)
}

fn strip_empty_edges(mut cells: Vec<String>) -> Vec<String> {
    if cells.first().is_some_and(|s| s.is_empty()) {
        cells.remove(0);
    }
    if cells.last().is_some_and(|s| s.is_empty()) {
        cells.pop();
    }
    cells
}

fn split_header(rows: Vec<Vec<String>>) -> Result<Table, String> {
    if rows.is_empty() {
        return Ok(Table {
            headers: Vec::new(),
            rows: Vec::new(),
        });
    }
    let mut iter = rows.into_iter();
    let headers = iter.next().unwrap();
    let rows: Vec<Vec<String>> = iter.collect();
    Ok(Table { headers, rows })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn csv_basic() {
        let t = parse("name,age\nalice,30\nbob,25", Format::Csv).unwrap();
        assert_eq!(t.headers, vec!["name", "age"]);
        assert_eq!(t.rows.len(), 2);
        assert_eq!(t.rows[0], vec!["alice", "30"]);
    }

    #[test]
    fn csv_quoted() {
        let t = parse("name,bio\nalice,\"hello, world\"\nbob,simple", Format::Csv).unwrap();
        assert_eq!(t.rows[0][1], "hello, world");
    }

    #[test]
    fn markdown_table() {
        let input = "| Name | Age |\n|------|-----|\n| Alice | 30 |\n| Bob | 25 |";
        let t = parse(input, Format::Markdown).unwrap();
        assert_eq!(t.headers, vec!["Name", "Age"]);
        assert_eq!(t.rows.len(), 2);
    }

    #[test]
    fn ascii_aligned() {
        let input = "NAME          STATUS    AGE\nmy-pod        Running   5d\nother-pod     Pending   1d";
        let t = parse(input, Format::Ascii).unwrap();
        assert_eq!(t.headers, vec!["NAME", "STATUS", "AGE"]);
        assert_eq!(t.rows[0], vec!["my-pod", "Running", "5d"]);
    }

    #[test]
    fn whitespace_delimited() {
        let t = parse("hello world foo\na b c", Format::Whitespace).unwrap();
        assert_eq!(t.headers, vec!["hello", "world", "foo"]);
        assert_eq!(t.rows[0], vec!["a", "b", "c"]);
    }
}
