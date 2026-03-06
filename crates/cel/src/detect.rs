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
            "plain" => Ok(Format::Whitespace),
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
    // Find column start positions from header (positions where non-space starts after 2+ spaces)
    let col_starts = find_column_starts(header);
    if col_starts.len() < 2 {
        return false;
    }
    // The gaps between columns: position just before each column start (except first)
    // Check that data rows have spaces in the gap regions
    let data_lines = &lines[1..];
    if data_lines.is_empty() {
        return false;
    }
    let threshold = (data_lines.len() as f64 * 0.6).ceil() as usize;
    // For each column start (except 0), check the char just before it is a space
    for &start in &col_starts[1..] {
        if start == 0 {
            continue;
        }
        let check_pos = start - 1;
        let count = data_lines
            .iter()
            .filter(|l| l.len() > check_pos && l.as_bytes()[check_pos] == b' ')
            .count();
        if count < threshold {
            return false;
        }
    }
    true
}

/// Find column start positions: position 0 + positions where non-space begins after 2+ spaces
fn find_column_starts(line: &str) -> Vec<usize> {
    let bytes = line.as_bytes();
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
    starts
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
}
