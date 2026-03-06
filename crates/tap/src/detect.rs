/// Detected data format.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Format {
    Json,
    Csv,
    Tsv,
    Xml,
    Text,
}

impl std::fmt::Display for Format {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Format::Json => write!(f, "json"),
            Format::Csv => write!(f, "csv"),
            Format::Tsv => write!(f, "tsv"),
            Format::Xml => write!(f, "xml"),
            Format::Text => write!(f, "text"),
        }
    }
}

/// Detect format from a sample of data (first ~8KB).
pub fn detect_format(sample: &[u8]) -> Format {
    let s = match std::str::from_utf8(sample) {
        Ok(s) => s,
        Err(_) => return Format::Text,
    };

    let trimmed = s.trim_start();

    if (trimmed.starts_with('{') || trimmed.starts_with('['))
        && (serde_json::from_str::<serde_json::Value>(trimmed).is_ok()
            || serde_json::from_str::<serde_json::Value>(&format!("{}]", trimmed)).is_ok()
            || trimmed.len() >= sample.len().saturating_sub(16))
    {
        return Format::Json;
    }

    if trimmed.starts_with('<') {
        return Format::Xml;
    }

    // Check for CSV/TSV by looking at first few lines
    let lines: Vec<&str> = trimmed.lines().take(5).collect();
    if lines.len() >= 2 {
        // TSV check
        let tab_counts: Vec<usize> = lines.iter().map(|l| l.matches('\t').count()).collect();
        if tab_counts[0] > 0 && tab_counts.iter().all(|&c| c == tab_counts[0]) {
            return Format::Tsv;
        }

        // CSV check
        let comma_counts: Vec<usize> = lines.iter().map(|l| l.matches(',').count()).collect();
        if comma_counts[0] > 0 && comma_counts.iter().all(|&c| c == comma_counts[0]) {
            return Format::Csv;
        }
    }

    Format::Text
}
