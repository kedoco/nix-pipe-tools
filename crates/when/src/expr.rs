use crate::parse::{self, Timestamp};

pub enum ExprResult {
    Time(Timestamp),
    Duration(i64), // nanoseconds, signed
}

pub fn eval_expr(input: &str) -> Result<ExprResult, String> {
    let input = input.trim();

    // Shorthand: "+ 5d" means "now + 5d", "- 2h" means "now - 2h"
    if input.starts_with("+ ") || input.starts_with("- ") {
        return eval_expr(&format!("now {}", input));
    }

    // Addition: timestamp + duration
    if let Some(pos) = input.find(" + ") {
        let left = &input[..pos];
        let right = &input[pos + 3..];
        let ts = parse::parse_timestamp(left)?;
        let dur = parse::parse_duration_nanos(right)?;
        return Ok(ExprResult::Time(Timestamp(ts.0 + dur)));
    }

    // Subtraction: timestamp - duration, or timestamp - timestamp
    // Use rfind to handle timestamps with spaces (e.g. "2024-03-06 12:00:00 - now")
    if let Some(pos) = input.rfind(" - ") {
        let left = &input[..pos];
        let right = &input[pos + 3..];
        let ts = parse::parse_timestamp(left)?;
        if let Ok(dur) = parse::parse_duration_nanos(right) {
            return Ok(ExprResult::Time(Timestamp(ts.0 - dur)));
        }
        let ts2 = parse::parse_timestamp(right)?;
        return Ok(ExprResult::Duration(ts.0 - ts2.0));
    }

    // Single timestamp
    let ts = parse::parse_timestamp(input)?;
    Ok(ExprResult::Time(ts))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_single_timestamp() {
        match eval_expr("1709740800").unwrap() {
            ExprResult::Time(ts) => assert_eq!(ts.epoch_secs(), 1709740800),
            ExprResult::Duration(_) => panic!("expected Time"),
        }
    }

    #[test]
    fn test_add_duration() {
        match eval_expr("1709740800 + 1d").unwrap() {
            ExprResult::Time(ts) => assert_eq!(ts.epoch_secs(), 1709740800 + 86400),
            ExprResult::Duration(_) => panic!("expected Time"),
        }
    }

    #[test]
    fn test_subtract_duration() {
        match eval_expr("1709740800 - 1h").unwrap() {
            ExprResult::Time(ts) => assert_eq!(ts.epoch_secs(), 1709740800 - 3600),
            ExprResult::Duration(_) => panic!("expected Time"),
        }
    }

    #[test]
    fn test_timestamp_diff() {
        match eval_expr("1709740800 - 1709654400").unwrap() {
            ExprResult::Duration(d) => assert_eq!(d / 1_000_000_000, 86400),
            ExprResult::Time(_) => panic!("expected Duration"),
        }
    }

    #[test]
    fn test_shorthand_plus() {
        match eval_expr("+ 1d").unwrap() {
            ExprResult::Time(ts) => {
                let now = crate::parse::Timestamp::now().epoch_secs();
                let diff = (ts.epoch_secs() - now - 86400).abs();
                assert!(diff <= 1);
            }
            ExprResult::Duration(_) => panic!("expected Time"),
        }
    }

    #[test]
    fn test_rfc3339_subtract_duration() {
        match eval_expr("2024-03-06T12:00:00Z - 1d").unwrap() {
            ExprResult::Time(ts) => assert_eq!(ts.epoch_secs(), 1709726400 - 86400),
            ExprResult::Duration(_) => panic!("expected Time"),
        }
    }

    #[test]
    fn test_datetime_with_space_subtract() {
        match eval_expr("2024-03-06 12:00:00 - 1h").unwrap() {
            ExprResult::Time(ts) => assert_eq!(ts.epoch_secs(), 1709726400 - 3600),
            ExprResult::Duration(_) => panic!("expected Time"),
        }
    }
}
