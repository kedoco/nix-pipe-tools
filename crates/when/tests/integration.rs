use std::io::Write;
use std::process::{Command, Output, Stdio};

fn when_cmd() -> Command {
    Command::new(env!("CARGO_BIN_EXE_when"))
}

fn run(args: &[&str]) -> Output {
    when_cmd()
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .unwrap()
}

fn run_with_stdin(args: &[&str], input: &[u8]) -> Output {
    let mut cmd = when_cmd();
    cmd.args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = cmd.spawn().unwrap();
    child.stdin.take().unwrap().write_all(input).unwrap();
    child.wait_with_output().unwrap()
}

fn stdout_trimmed(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).trim().to_string()
}

fn stderr_trimmed(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).trim().to_string()
}

// ---------------------------------------------------------------------------
// Epoch conversion (deterministic, fixed values)
// ---------------------------------------------------------------------------

#[test]
fn epoch_seconds_10_digit() {
    let out = run(&["1709740800"]);
    assert!(out.status.success());
    assert_eq!(stdout_trimmed(&out), "2024-03-06T16:00:00Z");
}

#[test]
fn epoch_millis_13_digit() {
    let out = run(&["1709740800000"]);
    assert!(out.status.success());
    assert_eq!(stdout_trimmed(&out), "2024-03-06T16:00:00Z");
}

#[test]
fn epoch_micros_16_digit() {
    let out = run(&["1709740800000000"]);
    assert!(out.status.success());
    assert_eq!(stdout_trimmed(&out), "2024-03-06T16:00:00Z");
}

#[test]
fn epoch_nanos_19_digit() {
    let out = run(&["1709740800000000000"]);
    assert!(out.status.success());
    assert_eq!(stdout_trimmed(&out), "2024-03-06T16:00:00Z");
}

#[test]
fn epoch_float() {
    let out = run(&["1709740800.5"]);
    assert!(out.status.success());
    assert_eq!(stdout_trimmed(&out), "2024-03-06T16:00:00.500Z");
}

#[test]
fn epoch_negative() {
    let out = run(&["--", "-86400"]);
    assert!(out.status.success());
    assert_eq!(stdout_trimmed(&out), "1969-12-31T00:00:00Z");
}

#[test]
fn epoch_zero() {
    let out = run(&["0"]);
    assert!(out.status.success());
    assert_eq!(stdout_trimmed(&out), "1970-01-01T00:00:00Z");
}

// ---------------------------------------------------------------------------
// Format parsing
// ---------------------------------------------------------------------------

#[test]
fn parse_rfc3339() {
    let out = run(&["2024-03-06T12:00:00Z", "-o", "epoch"]);
    assert!(out.status.success());
    assert_eq!(stdout_trimmed(&out), "1709726400");
}

#[test]
fn parse_rfc3339_with_offset() {
    let out = run(&["2024-03-06T12:00:00+05:00", "-o", "epoch"]);
    assert!(out.status.success());
    assert_eq!(stdout_trimmed(&out), "1709708400");
}

#[test]
fn parse_iso8601_no_tz() {
    let out = run(&["2024-03-06T12:00:00", "-o", "epoch"]);
    assert!(out.status.success());
    assert_eq!(stdout_trimmed(&out), "1709726400");
}

#[test]
fn parse_space_separated_datetime() {
    let out = run(&["2024-03-06 12:00:00", "-o", "epoch"]);
    assert!(out.status.success());
    assert_eq!(stdout_trimmed(&out), "1709726400");
}

#[test]
fn parse_date_only() {
    let out = run(&["2024-03-06", "-o", "epoch"]);
    assert!(out.status.success());
    assert_eq!(stdout_trimmed(&out), "1709683200");
}

#[test]
fn parse_json_quoted_epoch_via_stdin() {
    let out = run_with_stdin(&[], b"\"1709740800\"\n");
    assert!(out.status.success());
    assert_eq!(stdout_trimmed(&out), "2024-03-06T16:00:00Z");
}

#[test]
fn parse_json_quoted_string_via_stdin() {
    let out = run_with_stdin(&[], b"\"2024-03-06T12:00:00Z\"\n");
    assert!(out.status.success());
    // Should parse successfully; verify it produces valid output
    let s = stdout_trimmed(&out);
    assert!(!s.is_empty(), "expected non-empty output for quoted RFC 3339 string");
}

// ---------------------------------------------------------------------------
// Output formats
// ---------------------------------------------------------------------------

#[test]
fn output_epoch() {
    let out = run(&["2024-03-06T16:00:00Z", "-o", "epoch"]);
    assert!(out.status.success());
    assert_eq!(stdout_trimmed(&out), "1709740800");
}

#[test]
fn output_epoch_ms() {
    let out = run(&["2024-03-06T16:00:00Z", "-o", "epoch-ms"]);
    assert!(out.status.success());
    assert_eq!(stdout_trimmed(&out), "1709740800000");
}

#[test]
fn output_epoch_us() {
    let out = run(&["2024-03-06T16:00:00Z", "-o", "epoch-us"]);
    assert!(out.status.success());
    assert_eq!(stdout_trimmed(&out), "1709740800000000");
}

#[test]
fn output_epoch_ns() {
    let out = run(&["2024-03-06T16:00:00Z", "-o", "epoch-ns"]);
    assert!(out.status.success());
    assert_eq!(stdout_trimmed(&out), "1709740800000000000");
}

#[test]
fn output_relative() {
    let out = run(&["2024-03-06T16:00:00Z", "-o", "relative"]);
    assert!(out.status.success());
    let s = stdout_trimmed(&out);
    assert!(
        s.contains("ago") || s.contains("in"),
        "relative output should contain 'ago' or 'in', got: {}",
        s
    );
}

#[test]
fn output_strftime() {
    let out = run(&["1709740800", "-o", "%Y-%m-%d"]);
    assert!(out.status.success());
    assert_eq!(stdout_trimmed(&out), "2024-03-06");
}

// ---------------------------------------------------------------------------
// Arithmetic
// ---------------------------------------------------------------------------

#[test]
fn arithmetic_add_duration() {
    let out = run(&["1709740800", "+", "1d", "-o", "epoch"]);
    assert!(out.status.success());
    assert_eq!(stdout_trimmed(&out), "1709827200");
}

#[test]
fn arithmetic_subtract_duration() {
    let out = run(&["1709740800", "-", "1h", "-o", "epoch"]);
    assert!(out.status.success());
    assert_eq!(stdout_trimmed(&out), "1709737200");
}

#[test]
fn arithmetic_timestamp_difference() {
    let out = run(&["1709827200", "-", "1709740800"]);
    assert!(out.status.success());
    let s = stdout_trimmed(&out);
    assert!(
        s.contains("1d"),
        "expected difference to contain '1d', got: {}",
        s
    );
}

#[test]
fn arithmetic_chained() {
    // 1709740800 + 86400 + 7200 - 1800 = 1709832600
    let out = run(&["1709740800", "+", "1d", "+", "2h", "-", "30m", "-o", "epoch"]);
    assert!(out.status.success());
    assert_eq!(stdout_trimmed(&out), "1709832600");
}

#[test]
fn arithmetic_duration_difference_to_epoch() {
    let out = run(&["1709827200", "-", "1709740800", "-o", "epoch"]);
    assert!(out.status.success());
    assert_eq!(stdout_trimmed(&out), "86400");
}

// ---------------------------------------------------------------------------
// Stdin pipe mode
// ---------------------------------------------------------------------------

#[test]
fn stdin_single_line() {
    let out = run_with_stdin(&[], b"1709740800\n");
    assert!(out.status.success());
    assert_eq!(stdout_trimmed(&out), "2024-03-06T16:00:00Z");
}

#[test]
fn stdin_multiple_lines() {
    let out = run_with_stdin(&[], b"1709740800\n0\n");
    assert!(out.status.success());
    let s = stdout_trimmed(&out);
    let lines: Vec<&str> = s.lines().collect();
    assert_eq!(lines.len(), 2, "expected 2 output lines, got: {:?}", lines);
    assert_eq!(lines[0], "2024-03-06T16:00:00Z");
    assert_eq!(lines[1], "1970-01-01T00:00:00Z");
}

#[test]
fn stdin_with_format_flag() {
    let out = run_with_stdin(&["-o", "epoch-ms"], b"1709740800\n");
    assert!(out.status.success());
    assert_eq!(stdout_trimmed(&out), "1709740800000");
}

#[test]
fn stdin_empty_lines_skipped() {
    let out = run_with_stdin(&[], b"\n1709740800\n\n");
    assert!(out.status.success());
    let s = stdout_trimmed(&out);
    let lines: Vec<&str> = s.lines().collect();
    assert_eq!(
        lines.len(),
        1,
        "expected 1 output line (empty lines skipped), got: {:?}",
        lines
    );
    assert_eq!(lines[0], "2024-03-06T16:00:00Z");
}

// ---------------------------------------------------------------------------
// Error cases
// ---------------------------------------------------------------------------

#[test]
fn error_invalid_input() {
    let out = run(&["garbage"]);
    assert!(
        !out.status.success(),
        "expected non-zero exit code for invalid input"
    );
    let err = stderr_trimmed(&out);
    assert!(
        err.contains("when:"),
        "expected stderr to contain 'when:', got: {}",
        err
    );
}

#[test]
fn error_unknown_output_format() {
    let out = run(&["-o", "badformat", "now"]);
    assert!(
        !out.status.success(),
        "expected non-zero exit code for unknown output format"
    );
}
