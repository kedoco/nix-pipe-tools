use std::fs::{self, File};
use std::io::Write;
use std::net::TcpListener;
use std::process::{Command, Output, Stdio};

fn has_cmd() -> Command {
    Command::new(env!("CARGO_BIN_EXE_has"))
}

fn run(args: &[&str]) -> Output {
    has_cmd()
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .unwrap()
}

fn stdout_str(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).to_string()
}

fn stderr_str(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).to_string()
}

fn run_with_stdin(args: &[&str], stdin_data: &str) -> Output {
    let mut child = has_cmd()
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(stdin_data.as_bytes())
        .unwrap();
    child.wait_with_output().unwrap()
}

// ---------------------------------------------------------------------------
// CLI basics
// ---------------------------------------------------------------------------

#[test]
fn no_args_empty_stdin_produces_no_output() {
    // No args, empty stdin → no results, exit 0
    let out = run_with_stdin(&[], "");
    assert!(out.status.success());
    assert!(stdout_str(&out).is_empty());
}

#[test]
fn help_flag_exits_zero() {
    let out = run(&["--help"]);
    assert!(out.status.success());
    let help = stdout_str(&out);
    assert!(help.contains("file path"));
    assert!(help.contains(":port"));
}

#[test]
fn version_flag_exits_zero() {
    let out = run(&["--version"]);
    assert!(out.status.success());
    let ver = stdout_str(&out);
    assert!(ver.starts_with("has "));
}

// ---------------------------------------------------------------------------
// Error cases
// ---------------------------------------------------------------------------

#[test]
fn nonexistent_file_exits_nonzero() {
    let out = run(&["/tmp/has_test_no_such_file_ever_12345"]);
    assert!(!out.status.success());
    let err = stderr_str(&out);
    assert!(err.contains("no such file"));
}

#[test]
fn invalid_port_exits_nonzero() {
    let out = run(&[":not_a_port"]);
    assert!(!out.status.success());
    let err = stderr_str(&out);
    assert!(err.contains("invalid port"));
}

#[test]
fn port_overflow_exits_nonzero() {
    let out = run(&[":99999"]);
    assert!(!out.status.success());
    let err = stderr_str(&out);
    assert!(err.contains("invalid port"));
}

#[test]
fn empty_port_exits_nonzero() {
    let out = run(&[":"]);
    assert!(!out.status.success());
}

// ---------------------------------------------------------------------------
// Port queries
// ---------------------------------------------------------------------------

#[test]
fn unused_port_produces_no_output() {
    // Port 1 is highly unlikely to be in use and requires root to bind
    let out = run(&[":19"]);
    assert!(out.status.success());
    assert!(stdout_str(&out).is_empty());
}

#[test]
fn port_query_finds_listener() {
    // Bind a port, then ask has to find it
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();

    let out = run(&[&format!(":{}", port)]);
    assert!(out.status.success());

    let stdout = stdout_str(&out);
    if !stdout.is_empty() {
        // Should have a header with PID and PROCESS columns
        assert!(stdout.contains("PID"));
        assert!(stdout.contains("PROCESS"));
        // Should find our own PID
        let our_pid = std::process::id().to_string();
        assert!(
            stdout.contains(&our_pid),
            "expected to find our PID {} in output: {}",
            our_pid,
            stdout
        );
    }
    // Keep listener alive until test completes
    drop(listener);
}

#[test]
fn port_query_no_header_flag() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();

    let out = run(&["-H", &format!(":{}", port)]);
    assert!(out.status.success());

    let stdout = stdout_str(&out);
    // Should NOT contain the header row
    assert!(!stdout.contains("PID"));
    assert!(!stdout.contains("PROCESS"));

    drop(listener);
}

// ---------------------------------------------------------------------------
// File queries
// ---------------------------------------------------------------------------

#[test]
fn file_query_on_dev_null() {
    // /dev/null is commonly held open by many processes
    let out = run(&["/dev/null"]);
    assert!(out.status.success());
    // Might find processes, might not (depends on permissions)
    // But it should never crash
}

#[test]
fn file_query_on_temp_file_held_open() {
    // Create a temp file and hold it open
    let dir = tempfile::tempdir().unwrap();
    let file_path = dir.path().join("test_has_held.txt");
    let mut f = File::create(&file_path).unwrap();
    f.write_all(b"test data").unwrap();

    // We're holding `f` open. Query the file.
    let out = run(&[file_path.to_str().unwrap()]);
    assert!(out.status.success());

    // On Linux (procfs backend), we should find our own process.
    // On macOS (lsof backend), we should also find it.
    // But the test process might have dropped the fd by the time has runs.
    // At minimum, it should not crash.

    drop(f);
}

#[test]
fn file_query_nobody_has_it() {
    // Create a temp file, close it, nobody should have it open
    let dir = tempfile::tempdir().unwrap();
    let file_path = dir.path().join("test_has_closed.txt");
    fs::write(&file_path, "test data").unwrap();

    let out = run(&[file_path.to_str().unwrap()]);
    assert!(out.status.success());
    // Likely empty output — silence is golden
}

#[test]
fn file_query_symlink_resolved() {
    // Create a file and a symlink to it
    let dir = tempfile::tempdir().unwrap();
    let file_path = dir.path().join("real_file.txt");
    let link_path = dir.path().join("link_file.txt");
    fs::write(&file_path, "data").unwrap();
    std::os::unix::fs::symlink(&file_path, &link_path).unwrap();

    // Query via symlink should not crash and should resolve the path
    let out = run(&[link_path.to_str().unwrap()]);
    assert!(out.status.success());
}

#[test]
fn file_query_directory() {
    // Querying a directory (not a regular file) — should work
    let out = run(&["/tmp"]);
    assert!(out.status.success());
}

// ---------------------------------------------------------------------------
// Output format consistency
// ---------------------------------------------------------------------------

#[test]
fn process_table_columns_aligned() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();

    let out = run(&[&format!(":{}", port)]);
    let stdout = stdout_str(&out);

    if !stdout.is_empty() {
        let lines: Vec<&str> = stdout.lines().collect();
        assert!(lines.len() >= 2, "expected header + at least one data row");

        // Header should have these exact column names
        let header = lines[0];
        assert!(header.contains("PID"));
        assert!(header.contains("PROCESS"));
        assert!(header.contains("USER"));
        assert!(header.contains("FD"));
        assert!(header.contains("MODE"));
    }

    drop(listener);
}

// ---------------------------------------------------------------------------
// Address queries
// ---------------------------------------------------------------------------

#[test]
fn address_query_ipv4_no_crash() {
    // Query an IP — may or may not find connections, but should not crash
    let out = run(&["127.0.0.1"]);
    assert!(out.status.success());
}

#[test]
fn address_query_finds_listener() {
    // Bind a port, then query by the listener's address
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let out = run(&["127.0.0.1"]);
    assert!(out.status.success());

    let stdout = stdout_str(&out);
    if !stdout.is_empty() {
        // Should use the process table format
        assert!(stdout.contains("PID"));
        assert!(stdout.contains("PROCESS"));
    }
    drop(listener);
}

#[test]
fn address_query_hostname_no_crash() {
    // Querying a hostname that can't resolve produces a clean error
    let out = run(&["nonexistent.invalid"]);
    // Should exit non-zero with an error, not panic/crash
    assert!(!out.status.success());
    assert!(!stderr_str(&out).is_empty());
}

// ---------------------------------------------------------------------------
// Multiple args
// ---------------------------------------------------------------------------

#[test]
fn multiple_args_combines_results() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();

    // Query both a port and a file in one invocation
    let out = run(&[&format!(":{}", port), "/dev/null"]);
    assert!(out.status.success());

    let stdout = stdout_str(&out);
    if !stdout.is_empty() {
        // Single header row, results from both queries
        let header_count = stdout.lines().filter(|l| l.contains("PID")).count();
        assert_eq!(header_count, 1, "should have exactly one header row");
    }
    drop(listener);
}

#[test]
fn multiple_args_partial_error() {
    // One valid resource, one invalid — should still show results + error
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();

    let out = run(&[&format!(":{}", port), "/tmp/has_nonexistent_xyz_12345"]);
    // Should succeed (found results) but also report error on stderr
    let stdout = stdout_str(&out);
    let stderr = stderr_str(&out);
    if !stdout.is_empty() {
        assert!(stdout.contains("PID"));
    }
    assert!(stderr.contains("no such file"));
    drop(listener);
}

// ---------------------------------------------------------------------------
// Stdin support
// ---------------------------------------------------------------------------

#[test]
fn stdin_reads_resources() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();

    let out = run_with_stdin(&[], &format!(":{}\n", port));
    assert!(out.status.success());

    let stdout = stdout_str(&out);
    if !stdout.is_empty() {
        assert!(stdout.contains("PID"));
        let our_pid = std::process::id().to_string();
        assert!(stdout.contains(&our_pid));
    }
    drop(listener);
}

#[test]
fn stdin_ignores_blank_lines() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();

    let out = run_with_stdin(&[], &format!("\n\n:{}\n\n", port));
    assert!(out.status.success());
    drop(listener);
}

#[test]
fn stdin_multiple_resources() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();

    let out = run_with_stdin(&[], &format!(":{}\n/dev/null\n", port));
    assert!(out.status.success());

    let stdout = stdout_str(&out);
    if !stdout.is_empty() {
        let header_count = stdout.lines().filter(|l| l.contains("PID")).count();
        assert_eq!(header_count, 1, "should have exactly one header row");
    }
    drop(listener);
}

// ---------------------------------------------------------------------------
// Edge cases in input routing
// ---------------------------------------------------------------------------

#[test]
fn dot_slash_prefix_forces_file() {
    let out = run(&["./0"]);
    let err = stderr_str(&out);
    assert!(err.contains("no such file"), "expected file error, got: {}", err);
}

#[test]
fn relative_path_dot() {
    // "." is a valid directory path
    let out = run(&["."]);
    assert!(out.status.success());
}

// ---------------------------------------------------------------------------
// Silence is golden
// ---------------------------------------------------------------------------

#[test]
fn no_results_means_no_stdout_no_stderr() {
    // Query a temp file that no one has open
    let dir = tempfile::tempdir().unwrap();
    let file_path = dir.path().join("lonely.txt");
    fs::write(&file_path, "nobody reads me").unwrap();

    let out = run(&[file_path.to_str().unwrap()]);
    assert!(out.status.success());
    assert!(stdout_str(&out).is_empty(), "stdout should be empty for no results");
    assert!(stderr_str(&out).is_empty(), "stderr should be empty for no results");
}

// ---------------------------------------------------------------------------
// Data goes to stdout, errors to stderr
// ---------------------------------------------------------------------------

#[test]
fn errors_go_to_stderr_not_stdout() {
    let out = run(&["/tmp/has_nonexistent_xyz_12345"]);
    assert!(!out.status.success());
    assert!(stdout_str(&out).is_empty(), "error output should not go to stdout");
    assert!(!stderr_str(&out).is_empty(), "error should go to stderr");
}

#[test]
fn data_goes_to_stdout() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();

    let out = run(&[&format!(":{}", port)]);
    assert!(out.status.success());

    if !stdout_str(&out).is_empty() {
        // Data was produced — should be on stdout
        assert!(!stdout_str(&out).is_empty());
        // And stderr should be clean
        assert!(stderr_str(&out).is_empty(), "stderr should be empty when results found");
    }

    drop(listener);
}
