use std::io::Write;
use std::process::{Command, Output, Stdio};

fn cel_cmd() -> Command {
    Command::new(env!("CARGO_BIN_EXE_cel"))
}

fn run_with_stdin(args: &[&str], input: &[u8]) -> Output {
    let mut cmd = cel_cmd();
    cmd.args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = cmd.spawn().unwrap();
    child.stdin.take().unwrap().write_all(input).unwrap();
    child.wait_with_output().unwrap()
}

fn stdout_str(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).to_string()
}

fn stderr_str(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).to_string()
}

// ---------------------------------------------------------------------------
// CSV input
// ---------------------------------------------------------------------------

#[test]
fn csv_basic_extraction() {
    let input = b"name,age\nAlice,30\nBob,25\n";
    let out = run_with_stdin(&["name", "-o", "csv"], input);
    assert!(out.status.success());
    let s = stdout_str(&out);
    assert!(s.contains("Alice"));
    assert!(s.contains("Bob"));
    assert!(!s.contains("30"));
    assert!(!s.contains("25"));
}

#[test]
fn csv_multiple_columns() {
    let input = b"name,age,city\nAlice,30,NYC\nBob,25,LA\n";
    let out = run_with_stdin(&["name,age", "-o", "csv"], input);
    assert!(out.status.success());
    let s = stdout_str(&out);
    assert!(s.contains("Alice"));
    assert!(s.contains("30"));
    assert!(s.contains("Bob"));
    assert!(s.contains("25"));
    assert!(!s.contains("NYC"));
    assert!(!s.contains("LA"));
}

#[test]
fn csv_select_by_index() {
    let input = b"name,age\nAlice,30\nBob,25\n";
    let out = run_with_stdin(&["1", "-o", "csv"], input);
    assert!(out.status.success());
    let s = stdout_str(&out);
    assert!(s.contains("name"));
    assert!(s.contains("Alice"));
    assert!(s.contains("Bob"));
    assert!(!s.contains("age"));
    assert!(!s.contains("30"));
    assert!(!s.contains("25"));
}

#[test]
fn csv_select_by_range() {
    let input = b"name,age,city\nAlice,30,NYC\nBob,25,LA\n";
    let out = run_with_stdin(&["1-2", "-o", "csv"], input);
    assert!(out.status.success());
    let s = stdout_str(&out);
    assert!(s.contains("name"));
    assert!(s.contains("age"));
    assert!(s.contains("Alice"));
    assert!(s.contains("30"));
    assert!(!s.contains("city"));
    assert!(!s.contains("NYC"));
}

// ---------------------------------------------------------------------------
// TSV input
// ---------------------------------------------------------------------------

#[test]
fn tsv_basic_extraction() {
    let input = b"name\tage\nAlice\t30\n";
    let out = run_with_stdin(&["name", "-t", "tsv", "-o", "csv"], input);
    assert!(out.status.success());
    let s = stdout_str(&out);
    assert!(s.contains("Alice"));
    assert!(!s.contains("30"));
}

// ---------------------------------------------------------------------------
// Markdown table input
// ---------------------------------------------------------------------------

#[test]
fn markdown_table_extraction() {
    let input = b"| name | age |\n|------|-----|\n| Alice | 30 |\n";
    let out = run_with_stdin(&["name", "-o", "csv"], input);
    assert!(out.status.success());
    let s = stdout_str(&out);
    assert!(s.contains("Alice"));
    assert!(!s.contains("30"));
}

// ---------------------------------------------------------------------------
// ASCII-aligned table input (ps/docker style)
// ---------------------------------------------------------------------------

#[test]
fn ascii_aligned_table_extraction() {
    let input = b"NAME     STATUS    AGE\nfoo      Running   5d\nbar      Pending   1d\n";
    let out = run_with_stdin(&["name,status", "-o", "csv"], input);
    assert!(out.status.success());
    let s = stdout_str(&out);
    assert!(s.contains("foo"));
    assert!(s.contains("Running"));
    assert!(s.contains("bar"));
    assert!(s.contains("Pending"));
}

// ---------------------------------------------------------------------------
// Whitespace table input
// ---------------------------------------------------------------------------

#[test]
fn whitespace_table_extraction() {
    let input = b"a b c\n1 2 3\n";
    let out = run_with_stdin(&["1", "-t", "plain", "-o", "csv"], input);
    assert!(out.status.success());
    let s = stdout_str(&out);
    assert!(s.contains("a"));
    assert!(s.contains("1"));
    assert!(!s.contains("b"));
    assert!(!s.contains("2"));
}

// ---------------------------------------------------------------------------
// Column selection
// ---------------------------------------------------------------------------

#[test]
fn select_by_name_case_insensitive() {
    let input = b"name,age\nAlice,30\nBob,25\n";
    let out = run_with_stdin(&["Name", "-o", "csv"], input);
    assert!(out.status.success());
    let s = stdout_str(&out);
    assert!(s.contains("Alice"));
    assert!(s.contains("Bob"));
    assert!(!s.contains("30"));
}

#[test]
fn select_by_index_second_column() {
    let input = b"name,age,city\nAlice,30,NYC\n";
    let out = run_with_stdin(&["2", "-o", "csv"], input);
    assert!(out.status.success());
    let s = stdout_str(&out);
    assert!(s.contains("age"));
    assert!(s.contains("30"));
    assert!(!s.contains("name"));
    assert!(!s.contains("Alice"));
}

#[test]
fn select_open_ended_range() {
    let input = b"a,b,c,d\n1,2,3,4\n";
    let out = run_with_stdin(&["2-", "-o", "csv"], input);
    assert!(out.status.success());
    let s = stdout_str(&out);
    assert!(!s.contains("a,") && !s.starts_with("a\n"));
    assert!(s.contains("b"));
    assert!(s.contains("c"));
    assert!(s.contains("d"));
    assert!(s.contains("2"));
    assert!(s.contains("3"));
    assert!(s.contains("4"));
}

#[test]
fn exclude_mode() {
    let input = b"name,age,city\nAlice,30,NYC\nBob,25,LA\n";
    let out = run_with_stdin(&["-x", "name", "-o", "csv"], input);
    assert!(out.status.success());
    let s = stdout_str(&out);
    assert!(!s.contains("name"));
    assert!(!s.contains("Alice"));
    assert!(!s.contains("Bob"));
    assert!(s.contains("age"));
    assert!(s.contains("city"));
    assert!(s.contains("30"));
    assert!(s.contains("NYC"));
}

// ---------------------------------------------------------------------------
// Filters (-w)
// ---------------------------------------------------------------------------

#[test]
fn filter_numeric_greater_than() {
    let input = b"name,age\nAlice,30\nBob,20\n";
    let out = run_with_stdin(&["-w", "age > 25", "-o", "csv"], input);
    assert!(out.status.success());
    let s = stdout_str(&out);
    assert!(s.contains("Alice"));
    assert!(!s.contains("Bob"));
}

#[test]
fn filter_string_equality() {
    let input = b"name,age\nAlice,30\nBob,20\n";
    let out = run_with_stdin(&["-w", "name = Alice", "-o", "csv"], input);
    assert!(out.status.success());
    let s = stdout_str(&out);
    assert!(s.contains("Alice"));
    assert!(!s.contains("Bob"));
}

#[test]
fn filter_regex_match() {
    let input = b"name,age\nAlice,30\nBob,20\n";
    let out = run_with_stdin(&["-w", "name ~ ^A", "-o", "csv"], input);
    assert!(out.status.success());
    let s = stdout_str(&out);
    assert!(s.contains("Alice"));
    assert!(!s.contains("Bob"));
}

// ---------------------------------------------------------------------------
// Output formats
// ---------------------------------------------------------------------------

#[test]
fn output_format_csv() {
    let input = b"name,age\nAlice,30\nBob,25\n";
    let out = run_with_stdin(&["-o", "csv"], input);
    assert!(out.status.success());
    let s = stdout_str(&out);
    let lines: Vec<&str> = s.lines().collect();
    assert_eq!(lines[0], "name,age");
    assert_eq!(lines[1], "Alice,30");
    assert_eq!(lines[2], "Bob,25");
}

#[test]
fn output_format_tsv() {
    let input = b"name,age\nAlice,30\nBob,25\n";
    let out = run_with_stdin(&["-o", "tsv"], input);
    assert!(out.status.success());
    let s = stdout_str(&out);
    let lines: Vec<&str> = s.lines().collect();
    assert_eq!(lines[0], "name\tage");
    assert_eq!(lines[1], "Alice\t30");
    assert_eq!(lines[2], "Bob\t25");
}

#[test]
fn output_format_json() {
    let input = b"name,age\nAlice,30\nBob,25\n";
    let out = run_with_stdin(&["-o", "json"], input);
    assert!(out.status.success());
    let s = stdout_str(&out);
    let parsed: serde_json::Value = serde_json::from_str(&s).expect("valid JSON");
    let arr = parsed.as_array().expect("should be array");
    assert_eq!(arr.len(), 2);
    assert_eq!(arr[0]["name"], "Alice");
    assert_eq!(arr[0]["age"], "30");
    assert_eq!(arr[1]["name"], "Bob");
    assert_eq!(arr[1]["age"], "25");
}

#[test]
fn output_format_plain() {
    let input = b"name,age\nAlice,30\nBob,25\n";
    let out = run_with_stdin(&["-o", "plain"], input);
    assert!(out.status.success());
    let s = stdout_str(&out);
    let lines: Vec<&str> = s.lines().collect();
    assert_eq!(lines[0], "name age");
    assert_eq!(lines[1], "Alice 30");
    assert_eq!(lines[2], "Bob 25");
}

#[test]
fn output_format_markdown() {
    let input = b"name,age\nAlice,30\nBob,25\n";
    let out = run_with_stdin(&["-o", "markdown"], input);
    assert!(out.status.success());
    let s = stdout_str(&out);
    assert!(s.contains("|"));
    assert!(s.contains("name"));
    assert!(s.contains("age"));
    // Markdown must have a separator line with dashes
    let has_separator = s.lines().any(|l| l.contains("---"));
    assert!(has_separator, "markdown output should have a separator line");
    assert!(s.contains("Alice"));
    assert!(s.contains("Bob"));
}

#[test]
fn output_format_ascii() {
    let input = b"name,age\nAlice,30\nBob,25\n";
    let out = run_with_stdin(&["-o", "ascii"], input);
    assert!(out.status.success());
    let s = stdout_str(&out);
    assert!(s.contains("+---"));
    assert!(s.contains("|"));
    assert!(s.contains("name"));
    assert!(s.contains("Alice"));
}

#[test]
fn output_format_box() {
    let input = b"name,age\nAlice,30\nBob,25\n";
    let out = run_with_stdin(&["-o", "box"], input);
    assert!(out.status.success());
    let s = stdout_str(&out);
    assert!(s.contains('\u{250c}'), "box output should contain top-left corner");
    assert!(s.contains('\u{2502}'), "box output should contain vertical line");
    assert!(s.contains('\u{2514}'), "box output should contain bottom-left corner");
    assert!(s.contains("name"));
    assert!(s.contains("Alice"));
}

// ---------------------------------------------------------------------------
// List columns mode
// ---------------------------------------------------------------------------

#[test]
fn list_columns_mode() {
    let input = b"name,age,city\nAlice,30,NYC\n";
    let out = run_with_stdin(&["-l"], input);
    assert!(out.status.success());
    let s = stdout_str(&out);
    assert!(s.contains("1"));
    assert!(s.contains("name"));
    assert!(s.contains("2"));
    assert!(s.contains("age"));
    assert!(s.contains("3"));
    assert!(s.contains("city"));
}

// ---------------------------------------------------------------------------
// No-header mode
// ---------------------------------------------------------------------------

#[test]
fn no_header_mode() {
    let input = b"name,age\nAlice,30\nBob,25\n";
    let out = run_with_stdin(&["--no-header", "-o", "csv"], input);
    assert!(out.status.success());
    let s = stdout_str(&out);
    let lines: Vec<&str> = s.lines().collect();
    // With --no-header, the header row is suppressed in output;
    // all rows (including original header) are treated as data
    // but the header line is still used for column names internally.
    // The output should not print the header line.
    // First output line should be Alice,30 (data row) since header is suppressed.
    assert_eq!(lines[0], "Alice,30");
    assert_eq!(lines[1], "Bob,25");
    assert_eq!(lines.len(), 2);
}

// ---------------------------------------------------------------------------
// Custom header
// ---------------------------------------------------------------------------

#[test]
fn custom_header_override() {
    let input = b"name,age\nAlice,30\nBob,25\n";
    let out = run_with_stdin(&["--header", "x,y", "-o", "csv"], input);
    assert!(out.status.success());
    let s = stdout_str(&out);
    let lines: Vec<&str> = s.lines().collect();
    assert_eq!(lines[0], "x,y");
    assert_eq!(lines[1], "Alice,30");
}

// ---------------------------------------------------------------------------
// Empty input
// ---------------------------------------------------------------------------

#[test]
fn empty_input_succeeds() {
    let out = run_with_stdin(&[], b"");
    assert!(out.status.success());
    let s = stdout_str(&out);
    assert!(s.is_empty());
}

#[test]
fn whitespace_only_input_succeeds() {
    let out = run_with_stdin(&[], b"   \n  \n");
    assert!(out.status.success());
    let s = stdout_str(&out);
    assert!(s.is_empty());
}

// ---------------------------------------------------------------------------
// Error cases
// ---------------------------------------------------------------------------

#[test]
fn invalid_column_name_exits_nonzero() {
    let input = b"name,age\nAlice,30\n";
    let out = run_with_stdin(&["nonexistent", "-o", "csv"], input);
    assert!(!out.status.success());
    let err = stderr_str(&out);
    assert!(
        err.contains("cel:"),
        "stderr should contain 'cel:' prefix, got: {}",
        err
    );
}

#[test]
fn invalid_filter_syntax_exits_nonzero() {
    let input = b"name,age\nAlice,30\n";
    let out = run_with_stdin(&["-w", "no_operator_here", "-o", "csv"], input);
    assert!(!out.status.success());
    let err = stderr_str(&out);
    assert!(
        err.contains("cel:"),
        "stderr should contain 'cel:' prefix, got: {}",
        err
    );
}
