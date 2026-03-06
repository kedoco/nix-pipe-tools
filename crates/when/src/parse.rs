use chrono::{DateTime, NaiveDate, NaiveDateTime, Utc};

/// Internal timestamp: nanoseconds since Unix epoch.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Timestamp(pub i64);

impl Timestamp {
    pub fn now() -> Self {
        let dt = Utc::now();
        let secs = dt.timestamp();
        let nsecs = dt.timestamp_subsec_nanos() as i64;
        Timestamp(secs * 1_000_000_000 + nsecs)
    }

    pub fn to_datetime(self) -> Option<DateTime<Utc>> {
        let secs = self.0.div_euclid(1_000_000_000);
        let nsecs = self.0.rem_euclid(1_000_000_000) as u32;
        DateTime::from_timestamp(secs, nsecs)
    }

    pub fn epoch_secs(self) -> i64 {
        self.0 / 1_000_000_000
    }

    pub fn epoch_millis(self) -> i64 {
        self.0 / 1_000_000
    }

    pub fn epoch_micros(self) -> i64 {
        self.0 / 1_000
    }

    pub fn epoch_nanos(self) -> i64 {
        self.0
    }
}

fn maybe_unquote(s: &str) -> &str {
    if s.len() >= 2 && s.starts_with('"') && s.ends_with('"') {
        &s[1..s.len() - 1]
    } else {
        s
    }
}

pub fn parse_timestamp(s: &str) -> Result<Timestamp, String> {
    let s = maybe_unquote(s.trim());

    if s.eq_ignore_ascii_case("now") {
        return Ok(Timestamp::now());
    }

    // Numeric: integer or float epoch
    if let Some(ts) = try_parse_numeric(s)? {
        return Ok(ts);
    }

    // RFC 3339 / ISO 8601 with timezone
    if let Ok(dt) = DateTime::parse_from_rfc3339(s) {
        return Ok(dt_to_timestamp(dt.with_timezone(&Utc)));
    }

    // ISO 8601 variants without timezone (assume UTC)
    for fmt in &[
        "%Y-%m-%dT%H:%M:%S%.f",
        "%Y-%m-%dT%H:%M:%S",
        "%Y-%m-%d %H:%M:%S%.f",
        "%Y-%m-%d %H:%M:%S",
    ] {
        if let Ok(ndt) = NaiveDateTime::parse_from_str(s, fmt) {
            return Ok(dt_to_timestamp(ndt.and_utc()));
        }
    }

    // Date only
    if let Ok(d) = NaiveDate::parse_from_str(s, "%Y-%m-%d") {
        if let Some(dt) = d.and_hms_opt(0, 0, 0) {
            return Ok(dt_to_timestamp(dt.and_utc()));
        }
    }

    Err(format!("unrecognized time format: {}", s))
}

fn try_parse_numeric(s: &str) -> Result<Option<Timestamp>, String> {
    let (negative, digits) = if let Some(rest) = s.strip_prefix('-') {
        (true, rest)
    } else {
        (false, s)
    };

    // Float epoch (seconds with fractional part)
    if digits.contains('.') {
        let parts: Vec<&str> = digits.splitn(2, '.').collect();
        if parts.len() == 2
            && !parts[0].is_empty()
            && parts[0].chars().all(|c| c.is_ascii_digit())
            && parts[1].chars().all(|c| c.is_ascii_digit())
        {
            let f: f64 = s.parse().map_err(|_| format!("invalid number: {}", s))?;
            return Ok(Some(Timestamp((f * 1_000_000_000.0) as i64)));
        }
        return Ok(None);
    }

    // Integer epoch
    if !digits.is_empty() && digits.chars().all(|c| c.is_ascii_digit()) {
        let n: i64 = s.parse().map_err(|_| format!("number out of range: {}", s))?;
        let len = digits.len();
        let ts = match len {
            1..=10 => Timestamp(n * 1_000_000_000),
            11..=13 => Timestamp(n * 1_000_000),
            14..=16 => Timestamp(n * 1_000),
            17..=19 => Timestamp(n),
            _ => {
                let sign = if negative { "-" } else { "" };
                return Err(format!(
                    "ambiguous epoch value ({} digits): {}{}",
                    len, sign, digits
                ));
            }
        };
        return Ok(Some(ts));
    }

    Ok(None)
}

fn dt_to_timestamp(dt: DateTime<Utc>) -> Timestamp {
    let secs = dt.timestamp();
    let nsecs = dt.timestamp_subsec_nanos() as i64;
    Timestamp(secs * 1_000_000_000 + nsecs)
}

/// Parse a duration string like "5d", "2h30m", "500ms", "1w2d".
/// Returns nanoseconds.
pub fn parse_duration_nanos(s: &str) -> Result<i64, String> {
    let s = s.trim();
    if s.is_empty() {
        return Err("empty duration".to_string());
    }

    let mut total: i64 = 0;
    let mut chars = s.chars().peekable();
    let mut found_unit = false;

    while chars.peek().is_some() {
        let mut num_str = String::new();
        while let Some(&c) = chars.peek() {
            if c.is_ascii_digit() || c == '.' {
                num_str.push(c);
                chars.next();
            } else {
                break;
            }
        }

        if num_str.is_empty() {
            return Err(format!("invalid duration: {}", s));
        }

        let num: f64 = num_str
            .parse()
            .map_err(|_| format!("invalid number in duration: {}", s))?;

        let mut unit = String::new();
        while let Some(&c) = chars.peek() {
            if c.is_ascii_alphabetic() || c == 'µ' {
                unit.push(c);
                chars.next();
            } else {
                break;
            }
        }

        if unit.is_empty() {
            return Err(format!("missing unit in duration: {}", s));
        }

        found_unit = true;
        let nanos_per: f64 = match unit.as_str() {
            "ns" => 1.0,
            "us" | "µs" => 1_000.0,
            "ms" => 1_000_000.0,
            "s" => 1_000_000_000.0,
            "m" => 60.0 * 1_000_000_000.0,
            "h" => 3_600.0 * 1_000_000_000.0,
            "d" => 86_400.0 * 1_000_000_000.0,
            "w" => 7.0 * 86_400.0 * 1_000_000_000.0,
            _ => return Err(format!("unknown duration unit: {}", unit)),
        };

        total += (num * nanos_per) as i64;
    }

    if !found_unit {
        return Err(format!("invalid duration: {}", s));
    }

    Ok(total)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_epoch_seconds() {
        let ts = parse_timestamp("1709740800").unwrap();
        assert_eq!(ts.epoch_secs(), 1709740800);
    }

    #[test]
    fn test_epoch_millis() {
        let ts = parse_timestamp("1709740800000").unwrap();
        assert_eq!(ts.epoch_secs(), 1709740800);
        assert_eq!(ts.epoch_millis(), 1709740800000);
    }

    #[test]
    fn test_epoch_micros() {
        let ts = parse_timestamp("1709740800000000").unwrap();
        assert_eq!(ts.epoch_secs(), 1709740800);
        assert_eq!(ts.epoch_micros(), 1709740800000000);
    }

    #[test]
    fn test_epoch_nanos() {
        let ts = parse_timestamp("1709740800000000000").unwrap();
        assert_eq!(ts.epoch_secs(), 1709740800);
        assert_eq!(ts.epoch_nanos(), 1709740800000000000);
    }

    #[test]
    fn test_negative_epoch() {
        let ts = parse_timestamp("-86400").unwrap();
        assert_eq!(ts.epoch_secs(), -86400);
    }

    #[test]
    fn test_float_epoch() {
        let ts = parse_timestamp("1709740800.5").unwrap();
        assert_eq!(ts.epoch_secs(), 1709740800);
        assert_eq!(ts.epoch_millis(), 1709740800500);
    }

    #[test]
    fn test_rfc3339() {
        let ts = parse_timestamp("2024-03-06T12:00:00Z").unwrap();
        assert_eq!(ts.epoch_secs(), 1709726400);
    }

    #[test]
    fn test_rfc3339_subsec() {
        let ts = parse_timestamp("2024-03-06T12:00:00.123456Z").unwrap();
        assert_eq!(ts.epoch_micros(), 1709726400123456);
    }

    #[test]
    fn test_iso8601_no_tz() {
        let ts = parse_timestamp("2024-03-06T12:00:00").unwrap();
        assert_eq!(ts.epoch_secs(), 1709726400);
    }

    #[test]
    fn test_datetime_space() {
        let ts = parse_timestamp("2024-03-06 12:00:00").unwrap();
        assert_eq!(ts.epoch_secs(), 1709726400);
    }

    #[test]
    fn test_date_only() {
        let ts = parse_timestamp("2024-03-06").unwrap();
        assert_eq!(ts.epoch_secs(), 1709683200);
    }

    #[test]
    fn test_json_quoted() {
        let ts = parse_timestamp("\"2024-03-06T12:00:00Z\"").unwrap();
        assert_eq!(ts.epoch_secs(), 1709726400);
    }

    #[test]
    fn test_json_quoted_epoch() {
        let ts = parse_timestamp("\"1709740800\"").unwrap();
        assert_eq!(ts.epoch_secs(), 1709740800);
    }

    #[test]
    fn test_now() {
        let ts = parse_timestamp("now").unwrap();
        let diff = (Timestamp::now().0 - ts.0).abs();
        assert!(diff < 1_000_000_000); // within 1 second
    }

    #[test]
    fn test_small_epoch() {
        let ts = parse_timestamp("0").unwrap();
        assert_eq!(ts.epoch_secs(), 0);
    }

    #[test]
    fn test_duration_simple() {
        assert_eq!(parse_duration_nanos("5s").unwrap(), 5_000_000_000);
        assert_eq!(parse_duration_nanos("2m").unwrap(), 120_000_000_000);
        assert_eq!(parse_duration_nanos("1h").unwrap(), 3_600_000_000_000);
        assert_eq!(parse_duration_nanos("3d").unwrap(), 3 * 86_400_000_000_000);
        assert_eq!(
            parse_duration_nanos("1w").unwrap(),
            7 * 86_400_000_000_000
        );
    }

    #[test]
    fn test_duration_compound() {
        assert_eq!(
            parse_duration_nanos("1d12h").unwrap(),
            (86400 + 43200) * 1_000_000_000
        );
    }

    #[test]
    fn test_duration_subsecond() {
        assert_eq!(parse_duration_nanos("500ms").unwrap(), 500_000_000);
        assert_eq!(parse_duration_nanos("100us").unwrap(), 100_000);
        assert_eq!(parse_duration_nanos("50ns").unwrap(), 50);
    }

    #[test]
    fn test_duration_missing_unit() {
        assert!(parse_duration_nanos("42").is_err());
    }

    #[test]
    fn test_duration_unknown_unit() {
        assert!(parse_duration_nanos("5x").is_err());
    }
}
