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

    #[test]
    fn parse_port() {
        match parse_query(":8080").unwrap() {
            Query::Port(8080) => {}
            _ => panic!("expected Port(8080)"),
        }
    }

    #[test]
    fn parse_port_invalid() {
        assert!(parse_query(":99999").is_err());
        assert!(parse_query(":abc").is_err());
    }

    #[test]
    fn parse_pid() {
        match parse_query("1234").unwrap() {
            Query::Pid(1234) => {}
            _ => panic!("expected Pid(1234)"),
        }
    }

    #[test]
    fn parse_file_nonexistent() {
        assert!(parse_query("/tmp/has_test_nonexistent_file_xyz").is_err());
    }

    #[test]
    fn parse_empty_is_not_pid() {
        assert!(parse_query("").is_err());
    }
}
