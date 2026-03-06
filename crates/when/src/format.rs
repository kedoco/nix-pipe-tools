use chrono::SecondsFormat;

use crate::expr::ExprResult;
use crate::parse::Timestamp;

pub enum OutputFormat {
    Rfc3339,
    Epoch,
    EpochMs,
    EpochUs,
    EpochNs,
    Relative,
    Custom(String),
}

pub fn parse_output_format(s: &str) -> Result<OutputFormat, String> {
    if s.contains('%') {
        return Ok(OutputFormat::Custom(s.to_string()));
    }
    match s.to_lowercase().as_str() {
        "rfc3339" | "rfc-3339" | "iso8601" | "iso-8601" => Ok(OutputFormat::Rfc3339),
        "epoch" | "epoch-s" | "unix" => Ok(OutputFormat::Epoch),
        "epoch-ms" | "ms" => Ok(OutputFormat::EpochMs),
        "epoch-us" | "us" => Ok(OutputFormat::EpochUs),
        "epoch-ns" | "ns" => Ok(OutputFormat::EpochNs),
        "relative" | "rel" | "ago" => Ok(OutputFormat::Relative),
        _ => Err(format!(
            "unknown output format: {}. Options: rfc3339, iso8601, epoch, epoch-ms, epoch-us, epoch-ns, relative, or strftime pattern",
            s
        )),
    }
}

pub fn format_result(result: &ExprResult, fmt: &OutputFormat) -> Result<String, String> {
    match result {
        ExprResult::Time(ts) => format_timestamp(*ts, fmt),
        ExprResult::Duration(nanos) => Ok(format_duration(*nanos, fmt)),
    }
}

fn format_timestamp(ts: Timestamp, fmt: &OutputFormat) -> Result<String, String> {
    match fmt {
        OutputFormat::Epoch => Ok(ts.epoch_secs().to_string()),
        OutputFormat::EpochMs => Ok(ts.epoch_millis().to_string()),
        OutputFormat::EpochUs => Ok(ts.epoch_micros().to_string()),
        OutputFormat::EpochNs => Ok(ts.epoch_nanos().to_string()),
        OutputFormat::Rfc3339 => {
            let dt = ts
                .to_datetime()
                .ok_or_else(|| "timestamp out of range".to_string())?;
            let nsecs = ts.0.rem_euclid(1_000_000_000);
            let sec_fmt = if nsecs == 0 {
                SecondsFormat::Secs
            } else if nsecs % 1_000_000 == 0 {
                SecondsFormat::Millis
            } else if nsecs % 1_000 == 0 {
                SecondsFormat::Micros
            } else {
                SecondsFormat::Nanos
            };
            Ok(dt.to_rfc3339_opts(sec_fmt, true))
        }
        OutputFormat::Relative => {
            let now = Timestamp::now();
            Ok(format_relative(now.0 - ts.0))
        }
        OutputFormat::Custom(pattern) => {
            let dt = ts
                .to_datetime()
                .ok_or_else(|| "timestamp out of range".to_string())?;
            Ok(dt.format(pattern).to_string())
        }
    }
}

fn format_duration(nanos: i64, fmt: &OutputFormat) -> String {
    match fmt {
        OutputFormat::Epoch => (nanos / 1_000_000_000).to_string(),
        OutputFormat::EpochMs => (nanos / 1_000_000).to_string(),
        OutputFormat::EpochUs => (nanos / 1_000).to_string(),
        OutputFormat::EpochNs => nanos.to_string(),
        _ => format_duration_human(nanos),
    }
}

fn format_relative(diff_nanos: i64) -> String {
    let abs = diff_nanos.unsigned_abs();
    let secs = abs / 1_000_000_000;

    let text = if secs < 1 {
        return "just now".to_string();
    } else if secs < 60 {
        format!("{} seconds", secs)
    } else if secs < 3600 {
        let m = secs / 60;
        if m == 1 {
            "1 minute".to_string()
        } else {
            format!("{} minutes", m)
        }
    } else if secs < 86400 {
        let h = secs / 3600;
        if h == 1 {
            "1 hour".to_string()
        } else {
            format!("{} hours", h)
        }
    } else if secs < 86400 * 30 {
        let d = secs / 86400;
        if d == 1 {
            "1 day".to_string()
        } else {
            format!("{} days", d)
        }
    } else if secs < 86400 * 365 {
        let m = secs / (86400 * 30);
        if m == 1 {
            "1 month".to_string()
        } else {
            format!("{} months", m)
        }
    } else {
        let y = secs / (86400 * 365);
        if y == 1 {
            "1 year".to_string()
        } else {
            format!("{} years", y)
        }
    };

    if diff_nanos >= 0 {
        format!("{} ago", text)
    } else {
        format!("in {}", text)
    }
}

fn format_duration_human(nanos: i64) -> String {
    let negative = nanos < 0;
    let abs = nanos.unsigned_abs();
    let prefix = if negative { "-" } else { "" };

    if abs == 0 {
        return "0s".to_string();
    }

    if abs < 1_000 {
        return format!("{}{}ns", prefix, abs);
    }
    if abs < 1_000_000 {
        return format!("{}{}µs", prefix, format_frac(abs as f64 / 1_000.0));
    }
    if abs < 1_000_000_000 {
        return format!("{}{}ms", prefix, format_frac(abs as f64 / 1_000_000.0));
    }

    let total_secs = abs / 1_000_000_000;
    let mut parts = Vec::new();
    let mut rem = total_secs;

    for (unit, divisor) in [("w", 604800), ("d", 86400), ("h", 3600), ("m", 60), ("s", 1)] {
        let count = rem / divisor;
        if count > 0 {
            parts.push(format!("{}{}", count, unit));
            rem %= divisor;
        }
    }

    format!("{}{}", prefix, parts.join(""))
}

fn format_frac(n: f64) -> String {
    if (n - n.round()).abs() < 0.05 {
        format!("{:.0}", n)
    } else {
        format!("{:.1}", n)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_duration_human_seconds() {
        assert_eq!(format_duration_human(5_000_000_000), "5s");
    }

    #[test]
    fn test_duration_human_compound() {
        let nanos = (86400 + 3600 + 120 + 5) * 1_000_000_000_i64;
        assert_eq!(format_duration_human(nanos), "1d1h2m5s");
    }

    #[test]
    fn test_duration_human_weeks() {
        let nanos = 8 * 86400 * 1_000_000_000_i64;
        assert_eq!(format_duration_human(nanos), "1w1d");
    }

    #[test]
    fn test_duration_human_negative() {
        assert_eq!(format_duration_human(-3_600_000_000_000), "-1h");
    }

    #[test]
    fn test_duration_human_millis() {
        assert_eq!(format_duration_human(500_000_000), "500ms");
    }

    #[test]
    fn test_duration_human_micros() {
        assert_eq!(format_duration_human(1_500), "1.5µs");
    }

    #[test]
    fn test_duration_human_nanos() {
        assert_eq!(format_duration_human(42), "42ns");
    }

    #[test]
    fn test_duration_human_zero() {
        assert_eq!(format_duration_human(0), "0s");
    }

    #[test]
    fn test_relative_just_now() {
        assert_eq!(format_relative(500_000_000), "just now");
    }

    #[test]
    fn test_relative_minutes_ago() {
        assert_eq!(format_relative(180_000_000_000), "3 minutes ago");
    }

    #[test]
    fn test_relative_future() {
        assert_eq!(format_relative(-7200_000_000_000), "in 2 hours");
    }

    #[test]
    fn test_parse_output_format() {
        assert!(parse_output_format("rfc3339").is_ok());
        assert!(parse_output_format("epoch").is_ok());
        assert!(parse_output_format("epoch-ms").is_ok());
        assert!(parse_output_format("relative").is_ok());
        assert!(parse_output_format("%Y-%m-%d").is_ok());
        assert!(parse_output_format("garbage").is_err());
    }
}
