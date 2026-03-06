/// Format byte count in human-readable form.
pub fn format_bytes(bytes: u64) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB"];
    if bytes == 0 {
        return "0 B".to_string();
    }
    let mut size = bytes as f64;
    for unit in UNITS {
        if size < 1024.0 {
            return if size.fract() < 0.05 {
                format!("{:.0} {}", size, unit)
            } else {
                format!("{:.1} {}", size, unit)
            };
        }
        size /= 1024.0;
    }
    format!("{:.1} PB", size)
}

/// Format a duration in human-readable form.
pub fn format_duration(secs: f64) -> String {
    if secs < 0.001 {
        format!("{:.0}µs", secs * 1_000_000.0)
    } else if secs < 1.0 {
        format!("{:.0}ms", secs * 1000.0)
    } else if secs < 60.0 {
        format!("{:.1}s", secs)
    } else if secs < 3600.0 {
        let m = (secs / 60.0).floor() as u64;
        let s = (secs % 60.0).floor() as u64;
        format!("{}m{}s", m, s)
    } else {
        let h = (secs / 3600.0).floor() as u64;
        let m = ((secs % 3600.0) / 60.0).floor() as u64;
        format!("{}h{}m", h, m)
    }
}

/// Parse a human-readable duration string (e.g. "1h", "30m", "1d", "2h30m").
pub fn parse_duration(s: &str) -> Result<std::time::Duration, String> {
    let s = s.trim();
    let mut total_secs: u64 = 0;
    let mut current_num = String::new();

    for c in s.chars() {
        if c.is_ascii_digit() {
            current_num.push(c);
        } else {
            let num: u64 = current_num
                .parse()
                .map_err(|_| format!("invalid duration: {}", s))?;
            current_num.clear();
            match c {
                's' => total_secs += num,
                'm' => total_secs += num * 60,
                'h' => total_secs += num * 3600,
                'd' => total_secs += num * 86400,
                _ => return Err(format!("unknown duration unit: {}", c)),
            }
        }
    }

    if !current_num.is_empty() {
        // bare number defaults to seconds
        let num: u64 = current_num
            .parse()
            .map_err(|_| format!("invalid duration: {}", s))?;
        total_secs += num;
    }

    if total_secs == 0 {
        return Err(format!("invalid duration: {}", s));
    }

    Ok(std::time::Duration::from_secs(total_secs))
}

/// Parse a human-readable byte size (e.g. "1G", "500M", "1024K").
pub fn parse_size(s: &str) -> Result<u64, String> {
    let s = s.trim();
    let (num_str, unit) = if s.ends_with(|c: char| c.is_ascii_alphabetic()) {
        let split = s.len() - 1;
        // Handle "GB", "MB" etc
        let split = if s.len() >= 2
            && s.as_bytes()[s.len() - 1].eq_ignore_ascii_case(&b'B')
            && s.as_bytes()[s.len() - 2].is_ascii_alphabetic()
        {
            s.len() - 2
        } else {
            split
        };
        (&s[..split], &s[split..])
    } else {
        (s, "")
    };

    let num: f64 = num_str.parse().map_err(|_| format!("invalid size: {}", s))?;
    let multiplier: u64 = match unit.to_uppercase().as_str() {
        "" | "B" => 1,
        "K" | "KB" => 1024,
        "M" | "MB" => 1024 * 1024,
        "G" | "GB" => 1024 * 1024 * 1024,
        "T" | "TB" => 1024 * 1024 * 1024 * 1024,
        _ => return Err(format!("unknown size unit: {}", unit)),
    };

    Ok((num * multiplier as f64) as u64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_bytes() {
        assert_eq!(format_bytes(0), "0 B");
        assert_eq!(format_bytes(100), "100 B");
        assert_eq!(format_bytes(1024), "1 KB");
        assert_eq!(format_bytes(1536), "1.5 KB");
        assert_eq!(format_bytes(1048576), "1 MB");
    }

    #[test]
    fn test_format_duration() {
        assert_eq!(format_duration(0.5), "500ms");
        assert_eq!(format_duration(1.0), "1.0s");
        assert_eq!(format_duration(90.0), "1m30s");
        assert_eq!(format_duration(3661.0), "1h1m");
    }

    #[test]
    fn test_parse_duration() {
        assert_eq!(parse_duration("1h").unwrap().as_secs(), 3600);
        assert_eq!(parse_duration("30m").unwrap().as_secs(), 1800);
        assert_eq!(parse_duration("1d").unwrap().as_secs(), 86400);
        assert_eq!(parse_duration("2h30m").unwrap().as_secs(), 9000);
    }

    #[test]
    fn test_parse_size() {
        assert_eq!(parse_size("1G").unwrap(), 1024 * 1024 * 1024);
        assert_eq!(parse_size("500M").unwrap(), 500 * 1024 * 1024);
        assert_eq!(parse_size("1GB").unwrap(), 1024 * 1024 * 1024);
        assert_eq!(parse_size("1024").unwrap(), 1024);
    }
}
