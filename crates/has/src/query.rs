use std::path::PathBuf;

use crate::types::Entry;

/// What kind of resource to look up.
pub enum Query {
    /// A file path — find processes that have it open.
    File(PathBuf),
    /// A port number — find processes listening on or connected to it.
    Port(u16),
    /// A PID — find resources held by that process.
    Pid(u32),
}

/// Auto-detect query type from user input.
///
/// - `:8080` → Port
/// - Pure digits → PID
/// - Anything else → file path
pub fn parse_query(input: &str) -> Result<Query, String> {
    if let Some(port_str) = input.strip_prefix(':') {
        let port = port_str
            .parse::<u16>()
            .map_err(|_| format!("invalid port: {}", port_str))?;
        return Ok(Query::Port(port));
    }

    if input.chars().all(|c| c.is_ascii_digit()) && !input.is_empty() {
        let pid = input
            .parse::<u32>()
            .map_err(|_| format!("invalid PID: {}", input))?;
        return Ok(Query::Pid(pid));
    }

    let path = PathBuf::from(input);
    if !path.exists() {
        return Err(format!("no such file: {}", input));
    }
    let canonical = path
        .canonicalize()
        .map_err(|e| format!("cannot resolve path {}: {}", input, e))?;
    Ok(Query::File(canonical))
}

/// Execute the query using the platform-native backend.
pub fn execute(query: &Query) -> Result<Vec<Entry>, String> {
    execute_platform(query)
}

/// Linux: native /proc filesystem backend.
#[cfg(target_os = "linux")]
fn execute_platform(query: &Query) -> Result<Vec<Entry>, String> {
    match query {
        Query::File(path) => crate::procfs::query_file(path),
        Query::Port(port) => crate::procfs::query_port(*port),
        Query::Pid(pid) => crate::procfs::query_pid(*pid),
    }
}

/// macOS (and other platforms): lsof backend.
#[cfg(not(target_os = "linux"))]
fn execute_platform(query: &Query) -> Result<Vec<Entry>, String> {
    match query {
        Query::File(path) => crate::lsof::query_file(path),
        Query::Port(port) => crate::lsof::query_port(*port),
        Query::Pid(pid) => crate::lsof::query_pid(*pid),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- Port parsing ---

    #[test]
    fn parse_port_standard() {
        match parse_query(":8080").unwrap() {
            Query::Port(8080) => {}
            _ => panic!("expected Port(8080)"),
        }
    }

    #[test]
    fn parse_port_zero() {
        match parse_query(":0").unwrap() {
            Query::Port(0) => {}
            _ => panic!("expected Port(0)"),
        }
    }

    #[test]
    fn parse_port_max() {
        match parse_query(":65535").unwrap() {
            Query::Port(65535) => {}
            _ => panic!("expected Port(65535)"),
        }
    }

    #[test]
    fn parse_port_overflow() {
        assert!(parse_query(":65536").is_err());
        assert!(parse_query(":99999").is_err());
    }

    #[test]
    fn parse_port_not_a_number() {
        assert!(parse_query(":abc").is_err());
    }

    #[test]
    fn parse_port_empty_after_colon() {
        assert!(parse_query(":").is_err());
    }

    #[test]
    fn parse_port_negative() {
        // ":-1" — strip_prefix gives "-1", parse::<u16> fails
        assert!(parse_query(":-1").is_err());
    }

    #[test]
    fn parse_port_with_spaces() {
        assert!(parse_query(": 8080").is_err());
    }

    // --- PID parsing ---

    #[test]
    fn parse_pid_standard() {
        match parse_query("1234").unwrap() {
            Query::Pid(1234) => {}
            _ => panic!("expected Pid(1234)"),
        }
    }

    #[test]
    fn parse_pid_zero() {
        match parse_query("0").unwrap() {
            Query::Pid(0) => {}
            _ => panic!("expected Pid(0)"),
        }
    }

    #[test]
    fn parse_pid_max_u32() {
        match parse_query("4294967295").unwrap() {
            Query::Pid(4294967295) => {}
            _ => panic!("expected Pid(u32::MAX)"),
        }
    }

    #[test]
    fn parse_pid_overflow_u32() {
        // 4294967296 = u32::MAX + 1
        assert!(parse_query("4294967296").is_err());
    }

    #[test]
    fn parse_pid_leading_zeros() {
        // "007" is all digits → PID 7
        match parse_query("007").unwrap() {
            Query::Pid(7) => {}
            _ => panic!("expected Pid(7)"),
        }
    }

    // --- File path parsing ---

    #[test]
    fn parse_file_nonexistent() {
        assert!(parse_query("/tmp/has_test_nonexistent_file_xyz").is_err());
    }

    #[test]
    fn parse_file_exists() {
        // /dev/null always exists
        match parse_query("/dev/null").unwrap() {
            Query::File(p) => assert_eq!(p.to_str().unwrap(), "/dev/null"),
            _ => panic!("expected File"),
        }
    }

    #[test]
    fn parse_file_relative_path() {
        // "." always exists and canonicalizes to an absolute path
        match parse_query(".").unwrap() {
            Query::File(p) => assert!(p.is_absolute()),
            _ => panic!("expected File"),
        }
    }

    #[test]
    fn parse_file_with_dotdot() {
        // "../" should resolve and canonicalize
        match parse_query("..").unwrap() {
            Query::File(p) => assert!(p.is_absolute()),
            _ => panic!("expected File"),
        }
    }

    // --- Ambiguity / edge cases ---

    #[test]
    fn parse_empty_is_not_pid() {
        assert!(parse_query("").is_err());
    }

    #[test]
    fn colon_prefix_always_wins_over_file() {
        // Even if a file named ":8080" existed, colon prefix triggers port parsing
        match parse_query(":8080").unwrap() {
            Query::Port(8080) => {}
            _ => panic!("colon prefix should always parse as port"),
        }
    }

    #[test]
    fn digits_always_win_over_file() {
        // Pure digits → PID, even if a file with that name exists
        // (user should use "./1234" to force file interpretation)
        match parse_query("1234").unwrap() {
            Query::Pid(1234) => {}
            _ => panic!("pure digits should always parse as PID"),
        }
    }

    #[test]
    fn mixed_chars_become_file_path() {
        // "abc" is not all digits, no colon prefix → file path (but doesn't exist)
        assert!(parse_query("abc123").is_err());
    }

    #[test]
    fn negative_number_becomes_file_path() {
        // "-1" has a dash, not all digits → file path (doesn't exist → error)
        assert!(parse_query("-1").is_err());
    }

    #[test]
    fn path_with_digits_only_name() {
        // "./0" has a non-digit '.', so it's a file path, not PID
        // Will fail because ./0 likely doesn't exist, but it should try as file
        let result = parse_query("./0");
        // Either succeeds as File or fails with "no such file" — never a PID
        match result {
            Ok(Query::File(_)) => {}
            Err(e) => assert!(e.contains("no such file"), "unexpected error: {}", e),
            _ => panic!("./0 should parse as file path, not PID"),
        }
    }
}
