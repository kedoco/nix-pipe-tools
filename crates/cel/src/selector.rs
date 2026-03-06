/// Parsed column selector
#[derive(Debug, Clone)]
pub enum Selector {
    /// Column by name (normalized: lowercase, underscores)
    Name(String),
    /// Column by 1-based index
    Index(usize),
    /// Inclusive range of 1-based indices
    Range(usize, Option<usize>),
}

/// Normalize a header name: lowercase, replace spaces/hyphens with underscores
pub fn normalize(name: &str) -> String {
    name.to_lowercase()
        .replace([' ', '-'], "_")
}

/// Parse a column selector string like "name,3,status,2-5,7-"
pub fn parse(input: &str) -> Result<Vec<Selector>, String> {
    let mut selectors = Vec::new();
    for part in input.split(',') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        selectors.push(parse_one(part)?);
    }
    if selectors.is_empty() {
        return Err("empty column selector".to_string());
    }
    Ok(selectors)
}

fn parse_one(s: &str) -> Result<Selector, String> {
    // Try range: "2-5" or "3-"
    if let Some(dash_pos) = s.find('-') {
        // Only treat as range if the part before dash is numeric
        let before = &s[..dash_pos];
        let after = &s[dash_pos + 1..];

        if let Ok(start) = before.parse::<usize>() {
            if start == 0 {
                return Err("column indices are 1-based".to_string());
            }
            if after.is_empty() {
                return Ok(Selector::Range(start, None));
            }
            if let Ok(end) = after.parse::<usize>() {
                if end == 0 {
                    return Err("column indices are 1-based".to_string());
                }
                if end < start {
                    return Err(format!("invalid range: {}-{}", start, end));
                }
                return Ok(Selector::Range(start, Some(end)));
            }
        }
        // Not a numeric range — treat as a name containing a hyphen
    }

    // Try single number
    if let Ok(n) = s.parse::<usize>() {
        if n == 0 {
            return Err("column indices are 1-based".to_string());
        }
        return Ok(Selector::Index(n));
    }

    // It's a name
    Ok(Selector::Name(normalize(s)))
}

/// Resolve selectors to 0-based column indices given headers.
/// If `exclude` is true, return all columns NOT matching the selectors.
pub fn resolve(
    selectors: &[Selector],
    headers: &[String],
    num_cols: usize,
    exclude: bool,
) -> Result<Vec<usize>, String> {
    let normalized_headers: Vec<String> = headers.iter().map(|h| normalize(h)).collect();

    let mut selected = Vec::new();
    for sel in selectors {
        match sel {
            Selector::Name(name) => {
                let idx = normalized_headers
                    .iter()
                    .position(|h| h == name)
                    .ok_or_else(|| format!("column '{}' not found", name))?;
                if !selected.contains(&idx) {
                    selected.push(idx);
                }
            }
            Selector::Index(n) => {
                let idx = n - 1;
                if idx >= num_cols {
                    return Err(format!("column {} out of range (have {})", n, num_cols));
                }
                if !selected.contains(&idx) {
                    selected.push(idx);
                }
            }
            Selector::Range(start, end) => {
                let s = start - 1;
                let e = end.map(|e| e - 1).unwrap_or(num_cols - 1);
                let e = e.min(num_cols - 1);
                for idx in s..=e {
                    if !selected.contains(&idx) {
                        selected.push(idx);
                    }
                }
            }
        }
    }

    if exclude {
        let all: Vec<usize> = (0..num_cols).filter(|i| !selected.contains(i)).collect();
        Ok(all)
    } else {
        Ok(selected)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_names_and_numbers() {
        let sels = parse("name,3,status").unwrap();
        assert_eq!(sels.len(), 3);
        assert!(matches!(&sels[0], Selector::Name(n) if n == "name"));
        assert!(matches!(&sels[1], Selector::Index(3)));
        assert!(matches!(&sels[2], Selector::Name(n) if n == "status"));
    }

    #[test]
    fn parse_ranges() {
        let sels = parse("2-5,7-").unwrap();
        assert!(matches!(&sels[0], Selector::Range(2, Some(5))));
        assert!(matches!(&sels[1], Selector::Range(7, None)));
    }

    #[test]
    fn normalize_headers() {
        assert_eq!(normalize("CONTAINER ID"), "container_id");
        assert_eq!(normalize("container-id"), "container_id");
    }

    #[test]
    fn resolve_exclude() {
        let headers: Vec<String> = vec!["a".into(), "b".into(), "c".into()];
        let sels = parse("b").unwrap();
        let cols = resolve(&sels, &headers, 3, true).unwrap();
        assert_eq!(cols, vec![0, 2]);
    }
}
