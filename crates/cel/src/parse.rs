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

    // Fall back to whitespace splitting if no clear column boundaries found
    if col_starts.len() < 2 {
        return parse_whitespace(input);
    }

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

/// Find column start positions from header, validated against data rows.
///
/// Two-pass approach:
/// 1. Find columns separated by 2+ spaces in the header (handles most tabular output).
/// 2. If that finds fewer columns than header words, try 1-space gaps validated by
///    requiring nearly ALL data rows to have a space at that position (the "gutter"
///    approach). This handles tools like lsof that pre-calculate max column widths
///    and use exactly 1 space between columns.
fn find_column_starts(header: &str, data_lines: &[&str]) -> Vec<usize> {
    let bytes = header.as_bytes();

    // Pass 1: find gaps of 2+ spaces in the header
    let mut starts = Vec::new();
    let mut gap_starts = Vec::new();
    let mut i = 0;
    while i < bytes.len() && bytes[i] == b' ' {
        i += 1;
    }
    if i < bytes.len() {
        starts.push(i);
        gap_starts.push(0);
    }
    while i < bytes.len() {
        if bytes[i] == b' ' {
            let gap_start = i;
            while i < bytes.len() && bytes[i] == b' ' {
                i += 1;
            }
            if i - gap_start >= 2 && i < bytes.len() {
                starts.push(i);
                gap_starts.push(gap_start);
            }
        } else {
            i += 1;
        }
    }

    if data_lines.is_empty() || starts.len() < 2 {
        // Not enough columns from 2-space pass; try gutter approach directly
        let header_words = count_header_words(header);
        if header_words >= 2 && !data_lines.is_empty() {
            let gutter = find_gutter_columns(header, data_lines);
            if gutter.len() >= 2 {
                return gutter;
            }
        }
        return starts;
    }

    // Validate 2-space columns: check that data rows have at least one space
    // anywhere within the header's gap region.
    let threshold = (data_lines.len() as f64 * 0.6).ceil() as usize;
    let mut validated = vec![starts[0]];
    for idx in 1..starts.len() {
        let gap_begin = gap_starts[idx];
        let gap_end = starts[idx];
        let count = data_lines
            .iter()
            .filter(|l| {
                let b = l.as_bytes();
                (gap_begin..gap_end).any(|p| p < b.len() && b[p] == b' ')
            })
            .count();
        if count >= threshold {
            validated.push(starts[idx]);
        }
    }

    // Pass 2: if we found fewer columns than header words, try the gutter approach
    let header_words = count_header_words(header);
    if validated.len() < header_words {
        let gutter = find_gutter_columns(header, data_lines);
        if gutter.len() > validated.len() {
            return gutter;
        }
    }

    validated
}

/// Count whitespace-separated words in the header line.
fn count_header_words(header: &str) -> usize {
    header.split_whitespace().count()
}

/// Find column boundaries using the "gutter" approach: positions where ALL (or nearly
/// all) lines have a space character. Column boundaries are gutter→non-gutter transitions.
///
/// This handles tools like lsof that pre-calculate max column widths and use exactly
/// 1 space between columns, with a mix of left/right alignment.
fn find_gutter_columns(header: &str, data_lines: &[&str]) -> Vec<usize> {
    let header_len = header.len();
    let all_lines: Vec<&[u8]> = std::iter::once(header.as_bytes())
        .chain(data_lines.iter().map(|l| l.as_bytes()))
        .collect();
    if header_len == 0 {
        return Vec::new();
    }

    let total = all_lines.len();
    // Only search within the header's extent — positions beyond the header
    // are in the last column's data and can't define column boundaries.
    let mut is_gutter = vec![false; header_len];
    for pos in 0..header_len {
        let count = all_lines
            .iter()
            .filter(|l| pos >= l.len() || l[pos] == b' ')
            .count();
        is_gutter[pos] = count == total;
    }

    let col_starts = gutter_transitions(&is_gutter, header_len);

    // If strict gutter finds enough columns, use it
    let header_words = count_header_words(header);
    if col_starts.len() >= header_words {
        return col_starts;
    }

    // Relax to 90% threshold
    if total >= 4 {
        let threshold = (total as f64 * 0.9).ceil() as usize;
        for pos in 0..header_len {
            let count = all_lines
                .iter()
                .filter(|l| pos >= l.len() || l[pos] == b' ')
                .count();
            is_gutter[pos] = count >= threshold;
        }
        let relaxed = gutter_transitions(&is_gutter, header_len);
        if relaxed.len() > col_starts.len() {
            return relaxed;
        }
    }

    col_starts
}

/// Find column starts from gutter→non-gutter transitions.
fn gutter_transitions(is_gutter: &[bool], len: usize) -> Vec<usize> {
    let mut starts = Vec::new();
    for pos in 0..len {
        if !is_gutter[pos] && (pos == 0 || is_gutter[pos - 1]) {
            starts.push(pos);
        }
    }
    starts
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
    fn ascii_right_aligned_columns() {
        // Simulates lsof-style output with right-aligned numbers and spaces in last column
        let input = "\
COMMAND     PID  USER   FD    NAME
Google     6984 kevin  cwd    /
Google     6984 kevin  txt    /Applications/Google Chrome.app/Contents/MacOS/Google Chrome
Google     6984 kevin  3      /Users/kevin/Library/Application Support/Google/data";
        let t = parse(input, Format::Ascii).unwrap();
        assert_eq!(t.headers, vec!["COMMAND", "PID", "USER", "FD", "NAME"]);
        assert_eq!(t.rows[1][4], "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome");
        assert_eq!(t.rows[2][4], "/Users/kevin/Library/Application Support/Google/data");
    }

    #[test]
    fn ascii_lsof_one_space_gaps() {
        // lsof uses exactly 1 space between columns with pre-calculated max widths.
        // The gutter approach detects column boundaries via positions where ALL lines
        // have a space character (gutter→non-gutter transitions).
        //
        // Columns: COMMAND(7,L) PID(5,R) USER(5,L) FD(3,R) TYPE(4,L) DEVICE(6,R) SIZE/OFF(8,R) NODE(4,R) NAME(last)
        // Each column padded to its max width, 1 space separator between columns.
        let input = "\
COMMAND   PID USER   FD TYPE DEVICE SIZE/OFF NODE NAME
Google   6984 kevin cwd DIR    1,18      640    2 /
Google   6984 kevin txt REG    1,18   215040  123 /Applications/Google Chrome.app/Contents/MacOS/Google Chrome
node    18429 kevin 14u IPv4 0x1234      0t0  TCP *:8080 (LISTEN)";
        let t = parse(input, Format::Ascii).unwrap();
        assert_eq!(
            t.headers,
            vec!["COMMAND", "PID", "USER", "FD", "TYPE", "DEVICE", "SIZE/OFF", "NODE", "NAME"]
        );
        assert_eq!(t.rows[0][8], "/");
        assert_eq!(
            t.rows[1][8],
            "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome"
        );
        assert_eq!(t.rows[2][8], "*:8080 (LISTEN)");
    }

    #[test]
    fn whitespace_delimited() {
        let t = parse("hello world foo\na b c", Format::Whitespace).unwrap();
        assert_eq!(t.headers, vec!["hello", "world", "foo"]);
        assert_eq!(t.rows[0], vec!["a", "b", "c"]);
    }
}
