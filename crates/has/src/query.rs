use std::net::IpAddr;
use std::path::PathBuf;

use crate::types::Entry;

/// What kind of resource to look up.
pub enum Query {
    /// A file path — find processes that have it open.
    File(PathBuf),
    /// A port number — find processes listening on or connected to it.
    Port(u16),
    /// An IP address or hostname — find processes with connections to it.
    Address(String),
}

/// Auto-detect query type from user input.
///
/// - IPv4/IPv6 address → Address
/// - `:8080` → Port
/// - hostname (e.g. `example.com`) → Address
/// - Anything else → file path
pub fn parse_query(input: &str) -> Result<Query, String> {
    // IP address (v4 or v6) — checked early because IPv6 like "::1" starts with ':'
    if input.parse::<IpAddr>().is_ok() {
        return Ok(Query::Address(input.to_string()));
    }

    if let Some(port_str) = input.strip_prefix(':') {
        let port = port_str
            .parse::<u16>()
            .map_err(|_| format!("invalid port: {}", port_str))?;
        return Ok(Query::Port(port));
    }

    // Hostname: contains a dot, no path separators, valid DNS characters
    if looks_like_hostname(input) {
        return Ok(Query::Address(input.to_string()));
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

/// Check if input looks like a DNS hostname.
///
/// Must contain at least one dot, no path separators, and only
/// characters valid in DNS names (alphanumeric, hyphens, dots).
/// Each label (part between dots) must be non-empty.
fn looks_like_hostname(input: &str) -> bool {
    if input.contains('/') {
        return false;
    }
    if !input
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '.')
    {
        return false;
    }
    let labels: Vec<&str> = input.split('.').collect();
    // Need at least two labels (e.g., "example.com")
    if labels.len() < 2 {
        return false;
    }
    // Every label must be non-empty (no leading/trailing/consecutive dots)
    labels.iter().all(|l| !l.is_empty())
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
        Query::Address(addr) => crate::procfs::query_address(addr),
    }
}

/// macOS (and other platforms): lsof backend.
#[cfg(not(target_os = "linux"))]
fn execute_platform(query: &Query) -> Result<Vec<Entry>, String> {
    match query {
        Query::File(path) => crate::lsof::query_file(path),
        Query::Port(port) => crate::lsof::query_port(*port),
        Query::Address(addr) => crate::lsof::query_address(addr),
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

    // --- Pure digits → file path ---

    #[test]
    fn pure_digits_become_file_path() {
        // "1234" is not a port or IP → falls through to file path (doesn't exist → error)
        assert!(parse_query("1234").is_err());
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
    fn digits_become_file_path() {
        // Pure digits → file path (no longer treated as PID)
        // "1234" doesn't exist as a file → error
        assert!(parse_query("1234").is_err());
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

    // --- Address parsing ---

    #[test]
    fn parse_ipv4_address() {
        match parse_query("192.168.1.1").unwrap() {
            Query::Address(a) => assert_eq!(a, "192.168.1.1"),
            _ => panic!("expected Address"),
        }
    }

    #[test]
    fn parse_ipv4_localhost() {
        match parse_query("127.0.0.1").unwrap() {
            Query::Address(a) => assert_eq!(a, "127.0.0.1"),
            _ => panic!("expected Address"),
        }
    }

    #[test]
    fn parse_ipv6_address() {
        match parse_query("::1").unwrap() {
            Query::Address(a) => assert_eq!(a, "::1"),
            _ => panic!("expected Address"),
        }
    }

    #[test]
    fn parse_ipv6_full() {
        match parse_query("fe80::1").unwrap() {
            Query::Address(a) => assert_eq!(a, "fe80::1"),
            _ => panic!("expected Address"),
        }
    }

    #[test]
    fn parse_hostname() {
        match parse_query("example.com").unwrap() {
            Query::Address(a) => assert_eq!(a, "example.com"),
            _ => panic!("expected Address"),
        }
    }

    #[test]
    fn parse_hostname_subdomain() {
        match parse_query("api.example.com").unwrap() {
            Query::Address(a) => assert_eq!(a, "api.example.com"),
            _ => panic!("expected Address"),
        }
    }

    #[test]
    fn parse_hostname_with_hyphen() {
        match parse_query("my-host.example.com").unwrap() {
            Query::Address(a) => assert_eq!(a, "my-host.example.com"),
            _ => panic!("expected Address"),
        }
    }

    #[test]
    fn parse_single_label_not_hostname() {
        // "localhost" has no dot — falls through to file path (doesn't exist → error)
        assert!(parse_query("localhost").is_err());
    }

    #[test]
    fn parse_leading_dot_not_hostname() {
        // ".hidden" has an empty first label
        assert!(parse_query(".hidden").is_err());
    }

    #[test]
    fn parse_trailing_dot_not_hostname() {
        // "example." has an empty last label
        assert!(parse_query("example.").is_err());
    }

    #[test]
    fn parse_path_with_slash_not_hostname() {
        // "./foo.bar" has a slash → file path
        let result = parse_query("./foo.bar");
        match result {
            Ok(Query::File(_)) => {}
            Err(e) => assert!(e.contains("no such file"), "unexpected error: {}", e),
            _ => panic!("./foo.bar should be a file path"),
        }
    }

    #[test]
    fn path_with_digits_only_name() {
        // "./0" has a slash → file path
        let result = parse_query("./0");
        match result {
            Ok(Query::File(_)) => {}
            Err(e) => assert!(e.contains("no such file"), "unexpected error: {}", e),
            _ => panic!("./0 should parse as file path"),
        }
    }
}
