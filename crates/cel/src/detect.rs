/// Detected input format
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Format {
    Markdown,
    BoxDrawing,
    Ascii,
    Tsv,
    Csv,
    Whitespace,
}

impl Format {
    pub fn from_str_opt(s: &str) -> Result<Format, String> {
        match s.to_lowercase().as_str() {
            "markdown" | "md" => Ok(Format::Markdown),
            "box" => Ok(Format::BoxDrawing),
            "ascii" | "table" => Ok(Format::Ascii),
            "tsv" => Ok(Format::Tsv),
            "csv" => Ok(Format::Csv),
            "plain" => Ok(Format::Ascii),
            _ => Err(format!("unknown format: {}", s)),
        }
    }
}

/// Auto-detect the format from input lines.
pub fn detect(lines: &[&str]) -> Format {
    let non_empty: Vec<&str> = lines.iter().copied().filter(|l| !l.is_empty()).collect();
    if non_empty.is_empty() {
        return Format::Whitespace;
    }

    if is_markdown(&non_empty) {
        return Format::Markdown;
    }
    if is_box_drawing(&non_empty) {
        return Format::BoxDrawing;
    }
    if is_ascii_aligned(&non_empty) {
        return Format::Ascii;
    }
    if is_tsv(&non_empty) {
        return Format::Tsv;
    }
    if is_csv(&non_empty) {
        return Format::Csv;
    }
    Format::Whitespace
}

fn is_markdown(lines: &[&str]) -> bool {
    // Need at least 2 lines, one with | delimiters and a separator line
    if lines.len() < 2 {
        return false;
    }
    let has_pipe_lines = lines.iter().any(|l| l.contains('|'));
    if !has_pipe_lines {
        return false;
    }
    // Look for separator line: |---|---|  or  | --- | --- |  etc.
    lines.iter().any(|l| {
        let trimmed = l.trim();
        if !trimmed.contains('-') {
            return false;
        }
        // Remove pipes and check remaining is only dashes, colons, spaces
        let without_pipes = trimmed.replace('|', "");
        without_pipes.trim().chars().all(|c| c == '-' || c == ':' || c == ' ')
            && without_pipes.contains('-')
    })
}

fn is_box_drawing(lines: &[&str]) -> bool {
    lines.iter().any(|l| {
        (l.contains('+') && l.contains('-') && l.matches('+').count() >= 2)
            || l.contains('├')
            || l.contains('┌')
            || l.contains('└')
    }) && lines.iter().any(|l| l.contains('│') || l.contains('|'))
}

fn is_ascii_aligned(lines: &[&str]) -> bool {
    if lines.len() < 2 {
        return false;
    }
    let header = lines[0];
    let data_lines = &lines[1..];
    if data_lines.is_empty() {
        return false;
    }

    // Try 2-space gap detection first
    let (col_starts, gap_starts) = find_wide_column_starts(header);
    if col_starts.len() >= 2 {
        let threshold = (data_lines.len() as f64 * 0.6).ceil() as usize;
        let mut valid = true;
        for idx in 1..col_starts.len() {
            let gap_begin = gap_starts[idx];
            let gap_end = col_starts[idx];
            let count = data_lines
                .iter()
                .filter(|l| {
                    let b = l.as_bytes();
                    (gap_begin..gap_end).any(|p| p < b.len() && b[p] == b' ')
                })
                .count();
            if count < threshold {
                valid = false;
                break;
            }
        }
        if valid {
            return true;
        }
    }

    // Try gutter-based detection for 1-space separated columns (like lsof)
    let header_words = header.split_whitespace().count();
    if header_words >= 2 {
        let gutter_cols = find_gutter_columns(header, data_lines);
        if gutter_cols.len() >= 2 {
            return true;
        }
    }

    false
}

/// Find column start positions using 2+ space gaps in the header.
/// Returns (col_starts, gap_starts) where gap_starts[i] is the start of the gap before col_starts[i].
fn find_wide_column_starts(line: &str) -> (Vec<usize>, Vec<usize>) {
    let bytes = line.as_bytes();
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
    (starts, gap_starts)
}

/// Find column boundaries using the gutter approach: positions where all (or nearly all)
/// lines have a space character. Column boundaries are gutter→non-gutter transitions.
fn find_gutter_columns(header: &str, data_lines: &[&str]) -> Vec<usize> {
    let header_len = header.len();
    let all_lines: Vec<&[u8]> = std::iter::once(header.as_bytes())
        .chain(data_lines.iter().map(|l| l.as_bytes()))
        .collect();
    if header_len == 0 {
        return Vec::new();
    }

    let total = all_lines.len();
    let threshold = (total as f64 * 0.9).ceil() as usize;
    let mut is_gutter = vec![false; header_len];
    for pos in 0..header_len {
        let count = all_lines
            .iter()
            .filter(|l| pos >= l.len() || l[pos] == b' ')
            .count();
        is_gutter[pos] = count >= threshold;
    }

    // Find gutter→non-gutter transitions
    let mut col_starts = Vec::new();
    for pos in 0..header_len {
        if !is_gutter[pos] && (pos == 0 || is_gutter[pos - 1]) {
            col_starts.push(pos);
        }
    }

    col_starts
}

fn is_tsv(lines: &[&str]) -> bool {
    if lines.len() < 2 {
        return lines.len() == 1 && lines[0].contains('\t');
    }
    let counts: Vec<usize> = lines.iter().map(|l| l.matches('\t').count()).collect();
    let first = counts[0];
    first > 0 && counts.iter().all(|&c| c == first)
}

fn is_csv(lines: &[&str]) -> bool {
    if lines.len() < 2 {
        return lines.len() == 1 && lines[0].contains(',');
    }
    let counts: Vec<usize> = lines.iter().map(|l| count_csv_commas(l)).collect();
    let first = counts[0];
    first > 0 && counts.iter().all(|&c| c == first)
}

/// Count commas not inside quoted fields
fn count_csv_commas(line: &str) -> usize {
    let mut count = 0;
    let mut in_quote = false;
    for c in line.chars() {
        match c {
            '"' => in_quote = !in_quote,
            ',' if !in_quote => count += 1,
            _ => {}
        }
    }
    count
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_csv() {
        let lines = vec!["name,age", "alice,30", "bob,25"];
        assert_eq!(detect(&lines), Format::Csv);
    }

    #[test]
    fn detect_tsv() {
        let lines = vec!["name\tage", "alice\t30", "bob\t25"];
        assert_eq!(detect(&lines), Format::Tsv);
    }

    #[test]
    fn detect_markdown() {
        let lines = vec!["| Name | Age |", "|------|-----|", "| Alice | 30 |"];
        assert_eq!(detect(&lines), Format::Markdown);
    }

    #[test]
    fn detect_ascii() {
        let lines = vec![
            "NAME          STATUS    AGE",
            "my-pod        Running   5d",
            "other-pod     Pending   1d",
        ];
        assert_eq!(detect(&lines), Format::Ascii);
    }

    #[test]
    fn detect_ascii_one_space_gaps() {
        // lsof-style: 1-space separators with pre-calculated max column widths
        let lines = vec![
            "COMMAND   PID USER   FD TYPE DEVICE SIZE/OFF NODE NAME",
            "Google   6984 kevin cwd DIR    1,18      640    2 /",
            "node    18429 kevin 14u IPv4 0x1234      0t0  TCP *:8080",
        ];
        assert_eq!(detect(&lines), Format::Ascii);
    }
}
