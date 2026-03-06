use regex::Regex;

use crate::selector;

/// A parsed filter expression: column op value
pub struct Filter {
    column: ColumnRef,
    op: Op,
    value: String,
}

enum ColumnRef {
    Name(String),
    Index(usize),
}

enum Op {
    Eq,
    Ne,
    Lt,
    Gt,
    Le,
    Ge,
    Regex,
    NotRegex,
}

/// Parse a filter expression like "column > value" or "name ~ pattern"
pub fn parse_filter(expr: &str) -> Result<Filter, String> {
    // Try two-char operators first
    let ops = [
        ("!=", Op::Ne),
        ("<=", Op::Le),
        (">=", Op::Ge),
        ("!~", Op::NotRegex),
    ];
    for (token, op) in ops {
        if let Some(pos) = expr.find(token) {
            let col = expr[..pos].trim();
            let val = expr[pos + token.len()..].trim();
            return Ok(Filter {
                column: parse_col_ref(col)?,
                op,
                value: val.to_string(),
            });
        }
    }

    // Single-char operators
    let ops = [
        ("~", Op::Regex),
        ("=", Op::Eq),
        ("<", Op::Lt),
        (">", Op::Gt),
    ];
    for (token, op) in ops {
        if let Some(pos) = expr.find(token) {
            let col = expr[..pos].trim();
            let val = expr[pos + token.len()..].trim();
            return Ok(Filter {
                column: parse_col_ref(col)?,
                op,
                value: val.to_string(),
            });
        }
    }

    Err(format!("invalid filter expression: {}", expr))
}

fn parse_col_ref(s: &str) -> Result<ColumnRef, String> {
    if let Ok(n) = s.parse::<usize>() {
        if n == 0 {
            return Err("column indices are 1-based".to_string());
        }
        Ok(ColumnRef::Index(n))
    } else {
        Ok(ColumnRef::Name(selector::normalize(s)))
    }
}

/// Resolve filter column to a 0-based index
fn resolve_col(col: &ColumnRef, headers: &[String]) -> Result<usize, String> {
    match col {
        ColumnRef::Index(n) => Ok(n - 1),
        ColumnRef::Name(name) => {
            let normalized: Vec<String> = headers.iter().map(|h| selector::normalize(h)).collect();
            normalized
                .iter()
                .position(|h| h == name)
                .ok_or_else(|| format!("filter column '{}' not found", name))
        }
    }
}

/// Apply filters to rows, returning only rows that pass all filters
pub fn apply_filters(
    filters: &[Filter],
    headers: &[String],
    rows: Vec<Vec<String>>,
) -> Result<Vec<Vec<String>>, String> {
    // Pre-resolve column indices
    let resolved: Vec<(usize, &Op, &str)> = filters
        .iter()
        .map(|f| {
            let idx = resolve_col(&f.column, headers)?;
            Ok((idx, &f.op, f.value.as_str()))
        })
        .collect::<Result<Vec<_>, String>>()?;

    // Pre-compile regexes
    let mut regexes: Vec<Option<Regex>> = Vec::new();
    for (_, op, val) in &resolved {
        match op {
            Op::Regex | Op::NotRegex => {
                let re = Regex::new(val)
                    .map_err(|e| format!("invalid regex '{}': {}", val, e))?;
                regexes.push(Some(re));
            }
            _ => regexes.push(None),
        }
    }

    Ok(rows
        .into_iter()
        .filter(|row| {
            resolved.iter().zip(regexes.iter()).all(|(&(idx, op, val), regex)| {
                let cell = row.get(idx).map(String::as_str).unwrap_or("");
                match_filter(cell, op, val, regex.as_ref())
            })
        })
        .collect())
}

fn match_filter(cell: &str, op: &Op, value: &str, regex: Option<&Regex>) -> bool {
    match op {
        Op::Regex => regex.is_some_and(|r| r.is_match(cell)),
        Op::NotRegex => regex.is_none_or(|r| !r.is_match(cell)),
        _ => {
            // Try numeric comparison
            if let (Ok(a), Ok(b)) = (cell.parse::<f64>(), value.parse::<f64>()) {
                match op {
                    Op::Eq => a == b,
                    Op::Ne => a != b,
                    Op::Lt => a < b,
                    Op::Gt => a > b,
                    Op::Le => a <= b,
                    Op::Ge => a >= b,
                    _ => unreachable!(),
                }
            } else {
                match op {
                    Op::Eq => cell == value,
                    Op::Ne => cell != value,
                    Op::Lt => cell < value,
                    Op::Gt => cell > value,
                    Op::Le => cell <= value,
                    Op::Ge => cell >= value,
                    _ => unreachable!(),
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filter_numeric_gt() {
        let f = parse_filter("%cpu > 5.0").unwrap();
        let headers = vec!["%CPU".to_string(), "PID".to_string()];
        let rows = vec![
            vec!["10.5".to_string(), "123".to_string()],
            vec!["2.0".to_string(), "456".to_string()],
        ];
        let result = apply_filters(&[f], &headers, rows).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0][0], "10.5");
    }

    #[test]
    fn filter_regex() {
        let f = parse_filter("name ~ ^a").unwrap();
        let headers = vec!["name".to_string()];
        let rows = vec![
            vec!["alice".to_string()],
            vec!["bob".to_string()],
        ];
        let result = apply_filters(&[f], &headers, rows).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0][0], "alice");
    }
}
