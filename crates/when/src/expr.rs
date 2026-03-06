use crate::parse::{self, Timestamp};

pub enum ExprResult {
    Time(Timestamp),
    Duration(i64), // nanoseconds, signed
}

struct Segment {
    op: Option<char>,
    operand: String,
}

fn tokenize(input: &str) -> Vec<Segment> {
    let tokens: Vec<&str> = input.split_whitespace().collect();
    let mut segments = Vec::new();
    let mut current_op: Option<char> = None;
    let mut parts: Vec<&str> = Vec::new();

    for token in tokens {
        if token == "+" || token == "-" {
            if !parts.is_empty() {
                segments.push(Segment {
                    op: current_op,
                    operand: parts.join(" "),
                });
                parts.clear();
            }
            current_op = Some(token.as_bytes()[0] as char);
        } else {
            parts.push(token);
        }
    }

    if !parts.is_empty() {
        segments.push(Segment {
            op: current_op,
            operand: parts.join(" "),
        });
    }

    segments
}

pub fn eval_expr(input: &str) -> Result<ExprResult, String> {
    let input = input.trim();

    // Shorthand: "+ 5d" means "now + 5d", "- 2h" means "now - 2h"
    if input.starts_with("+ ") || input.starts_with("- ") {
        return eval_expr(&format!("now {}", input));
    }

    let segments = tokenize(input);
    if segments.is_empty() {
        return Err("empty expression".to_string());
    }

    // First segment: must be a timestamp (no operator)
    let mut result = ExprResult::Time(parse::parse_timestamp(&segments[0].operand)?);

    for seg in &segments[1..] {
        let op = seg.op.ok_or_else(|| "missing operator".to_string())?;
        let dur = parse::parse_duration_nanos(&seg.operand).ok();
        let ts = parse::parse_timestamp(&seg.operand).ok();

        result = match (&result, op) {
            (ExprResult::Time(t), '+') => {
                let d = dur.ok_or_else(|| {
                    format!("expected duration after '+': {}", seg.operand)
                })?;
                ExprResult::Time(Timestamp(t.0 + d))
            }
            (ExprResult::Time(t), '-') => {
                if let Some(d) = dur {
                    ExprResult::Time(Timestamp(t.0 - d))
                } else if let Some(t2) = ts {
                    ExprResult::Duration(t.0 - t2.0)
                } else {
                    return Err(format!("invalid operand: {}", seg.operand));
                }
            }
            (ExprResult::Duration(d), '+') => {
                let d2 = dur.ok_or_else(|| {
                    format!("expected duration after '+': {}", seg.operand)
                })?;
                ExprResult::Duration(d + d2)
            }
            (ExprResult::Duration(d), '-') => {
                let d2 = dur.ok_or_else(|| {
                    format!("expected duration after '-': {}", seg.operand)
                })?;
                ExprResult::Duration(d - d2)
            }
            _ => return Err(format!("invalid operator: {}", op)),
        };
    }

    Ok(result)
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

    #[test]
    fn test_chained_arithmetic() {
        // 2024-03-06 - 2024-03-05 = 1d, + 12h = 1d12h, - 30m = 1d11h30m
        match eval_expr("2024-03-06 - 2024-03-05 + 12h - 30m").unwrap() {
            ExprResult::Duration(d) => {
                let secs = d / 1_000_000_000;
                assert_eq!(secs, 86400 + 43200 - 1800);
            }
            ExprResult::Time(_) => panic!("expected Duration"),
        }
    }

    #[test]
    fn test_chained_time_add() {
        match eval_expr("1709740800 + 1d + 2h + 30m").unwrap() {
            ExprResult::Time(ts) => {
                assert_eq!(ts.epoch_secs(), 1709740800 + 86400 + 7200 + 1800);
            }
            ExprResult::Duration(_) => panic!("expected Time"),
        }
    }

    #[test]
    fn test_chained_mixed_ops() {
        match eval_expr("1709740800 + 2d - 1h").unwrap() {
            ExprResult::Time(ts) => {
                assert_eq!(ts.epoch_secs(), 1709740800 + 2 * 86400 - 3600);
            }
            ExprResult::Duration(_) => panic!("expected Time"),
        }
    }
}
