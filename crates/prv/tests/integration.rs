use std::fs;
use std::process::{Command, Output, Stdio};
use tempfile::TempDir;

fn prv_cmd(home: &std::path::Path) -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_prv"));
    cmd.env("HOME", home);
    cmd.env("XDG_DATA_HOME", home.join(".local/share"));
    cmd.env("XDG_CONFIG_HOME", home.join(".config"));
    cmd.stdout(Stdio::piped()).stderr(Stdio::piped());
    cmd
}

fn stdout_str(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).to_string()
}

fn stderr_str(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).to_string()
}

fn combined_output(output: &Output) -> String {
    format!("{}{}", stdout_str(output), stderr_str(output))
}

// ---------------------------------------------------------------------------
// Init (shell hooks)
// ---------------------------------------------------------------------------

#[test]
fn init_zsh_outputs_hook_code() {
    let tmp = TempDir::new().unwrap();
    let output = prv_cmd(tmp.path())
        .args(["init", "--zsh"])
        .output()
        .expect("failed to run prv init --zsh");

    assert!(output.status.success(), "prv init --zsh failed: {}", stderr_str(&output));
    let out = stdout_str(&output);
    assert!(!out.is_empty(), "zsh hook output should not be empty");
    assert!(out.contains("preexec"), "zsh hook should contain preexec, got: {}", out);
    assert!(out.contains("precmd"), "zsh hook should contain precmd, got: {}", out);
}

#[test]
fn init_bash_outputs_hook_code() {
    let tmp = TempDir::new().unwrap();
    let output = prv_cmd(tmp.path())
        .args(["init", "--bash"])
        .output()
        .expect("failed to run prv init --bash");

    assert!(output.status.success(), "prv init --bash failed: {}", stderr_str(&output));
    let out = stdout_str(&output);
    assert!(!out.is_empty(), "bash hook output should not be empty");
    assert!(
        out.contains("PROMPT_COMMAND") || out.contains("DEBUG"),
        "bash hook should contain PROMPT_COMMAND or DEBUG, got: {}",
        out
    );
}

// ---------------------------------------------------------------------------
// Record a command
// ---------------------------------------------------------------------------

#[test]
fn record_command_succeeds() {
    let tmp = TempDir::new().unwrap();
    let testfile = tmp.path().join("testfile.txt");
    fs::write(&testfile, "hello world").unwrap();

    let output = prv_cmd(tmp.path())
        .arg("record")
        .arg("--exit-code")
        .arg("0")
        .arg("--")
        .arg(format!("cat {}", testfile.display()))
        .output()
        .expect("failed to run prv record");

    assert!(
        output.status.success(),
        "prv record should succeed, stderr: {}",
        stderr_str(&output)
    );
}

// ---------------------------------------------------------------------------
// Log query
// ---------------------------------------------------------------------------

#[test]
fn log_shows_recorded_command() {
    let tmp = TempDir::new().unwrap();
    let testfile = tmp.path().join("data.txt");
    fs::write(&testfile, "some content").unwrap();

    // Record a command that references the file
    let record_out = prv_cmd(tmp.path())
        .arg("record")
        .arg("--exit-code")
        .arg("0")
        .arg("--")
        .arg(format!("cat {}", testfile.display()))
        .output()
        .expect("failed to run prv record");
    assert!(record_out.status.success(), "record failed: {}", stderr_str(&record_out));

    // Query log for that file
    let log_out = prv_cmd(tmp.path())
        .args(["log", &testfile.to_string_lossy()])
        .output()
        .expect("failed to run prv log");

    assert!(log_out.status.success(), "prv log failed: {}", stderr_str(&log_out));
    let out = stdout_str(&log_out);
    assert!(
        out.contains("cat"),
        "prv log should mention the recorded command 'cat', got: {}",
        out
    );
}

#[test]
fn log_nonexistent_file_shows_no_provenance() {
    let tmp = TempDir::new().unwrap();

    // Ensure database exists by running any command first
    let _ = prv_cmd(tmp.path())
        .args(["log", "/nonexistent/file/that/does/not/exist"])
        .output()
        .expect("failed to run prv log");

    let output = prv_cmd(tmp.path())
        .args(["log", "/nonexistent/file/that/does/not/exist"])
        .output()
        .expect("failed to run prv log");

    assert!(output.status.success(), "prv log should not crash: {}", stderr_str(&output));
    let out = stdout_str(&output);
    assert!(
        out.contains("No provenance"),
        "expected 'No provenance' message, got: {}",
        out
    );
}

// ---------------------------------------------------------------------------
// Search
// ---------------------------------------------------------------------------

#[test]
fn search_finds_recorded_command() {
    let tmp = TempDir::new().unwrap();
    let testfile = tmp.path().join("searchable.txt");
    fs::write(&testfile, "content").unwrap();

    // Record a command
    let record_out = prv_cmd(tmp.path())
        .arg("record")
        .arg("--exit-code")
        .arg("0")
        .arg("--")
        .arg(format!("cat {}", testfile.display()))
        .output()
        .expect("failed to run prv record");
    assert!(record_out.status.success());

    // Search for it
    let search_out = prv_cmd(tmp.path())
        .args(["search", "cat"])
        .output()
        .expect("failed to run prv search");

    assert!(search_out.status.success(), "prv search failed: {}", stderr_str(&search_out));
    let out = stdout_str(&search_out);
    assert!(out.contains("cat"), "search for 'cat' should find it, got: {}", out);
}

#[test]
fn search_no_match_shows_message() {
    let tmp = TempDir::new().unwrap();

    let output = prv_cmd(tmp.path())
        .args(["search", "zzzznonexistent"])
        .output()
        .expect("failed to run prv search");

    assert!(output.status.success(), "prv search should not crash: {}", stderr_str(&output));
    let out = stdout_str(&output);
    assert!(
        out.contains("No commands matching"),
        "expected 'No commands matching' message, got: {}",
        out
    );
}

// ---------------------------------------------------------------------------
// Deps query
// ---------------------------------------------------------------------------

#[test]
fn deps_unknown_file_shows_no_dependencies() {
    let tmp = TempDir::new().unwrap();

    let output = prv_cmd(tmp.path())
        .args(["deps", "/unknown/file/path"])
        .output()
        .expect("failed to run prv deps");

    assert!(output.status.success(), "prv deps should not crash: {}", stderr_str(&output));
    let out = stdout_str(&output);
    assert!(
        out.contains("No known dependencies"),
        "expected 'No known dependencies' message, got: {}",
        out
    );
}

#[test]
fn deps_does_not_crash_on_recorded_file() {
    let tmp = TempDir::new().unwrap();
    let input = tmp.path().join("input.txt");
    let output_file = tmp.path().join("output.txt");
    fs::write(&input, "data").unwrap();
    fs::write(&output_file, "result").unwrap();

    // Record a command that simulates reading input and writing output via redirect
    let record_out = prv_cmd(tmp.path())
        .arg("record")
        .arg("--exit-code")
        .arg("0")
        .arg("--")
        .arg(format!("cat {} > {}", input.display(), output_file.display()))
        .output()
        .expect("failed to run prv record");
    assert!(record_out.status.success());

    // Query deps -- may or may not find linkages depending on heuristic,
    // but should not crash
    let deps_out = prv_cmd(tmp.path())
        .args(["deps", &output_file.to_string_lossy()])
        .output()
        .expect("failed to run prv deps");

    assert!(deps_out.status.success(), "prv deps should not crash: {}", stderr_str(&deps_out));
}

// ---------------------------------------------------------------------------
// Rdeps query
// ---------------------------------------------------------------------------

#[test]
fn rdeps_unknown_file_shows_no_reverse_dependencies() {
    let tmp = TempDir::new().unwrap();

    let output = prv_cmd(tmp.path())
        .args(["rdeps", "/unknown/file/path"])
        .output()
        .expect("failed to run prv rdeps");

    assert!(output.status.success(), "prv rdeps should not crash: {}", stderr_str(&output));
    let out = stdout_str(&output);
    assert!(
        out.contains("No known reverse dependencies"),
        "expected 'No known reverse dependencies' message, got: {}",
        out
    );
}

#[test]
fn rdeps_does_not_crash_on_recorded_file() {
    let tmp = TempDir::new().unwrap();
    let input = tmp.path().join("source.txt");
    let output_file = tmp.path().join("derived.txt");
    fs::write(&input, "source data").unwrap();
    fs::write(&output_file, "derived data").unwrap();

    // Record a command
    let record_out = prv_cmd(tmp.path())
        .arg("record")
        .arg("--exit-code")
        .arg("0")
        .arg("--")
        .arg(format!("cat {} > {}", input.display(), output_file.display()))
        .output()
        .expect("failed to run prv record");
    assert!(record_out.status.success());

    let rdeps_out = prv_cmd(tmp.path())
        .args(["rdeps", &input.to_string_lossy()])
        .output()
        .expect("failed to run prv rdeps");

    assert!(
        rdeps_out.status.success(),
        "prv rdeps should not crash: {}",
        stderr_str(&rdeps_out)
    );
}

// ---------------------------------------------------------------------------
// Dot graph output
// ---------------------------------------------------------------------------

#[test]
fn dot_outputs_digraph_format() {
    let tmp = TempDir::new().unwrap();

    let output = prv_cmd(tmp.path())
        .args(["dot", "/some/file"])
        .output()
        .expect("failed to run prv dot");

    assert!(output.status.success(), "prv dot should not crash: {}", stderr_str(&output));
    let out = stdout_str(&output);
    assert!(
        out.contains("digraph"),
        "DOT output should contain 'digraph', got: {}",
        out
    );
}

#[test]
fn dot_with_recorded_data_outputs_digraph() {
    let tmp = TempDir::new().unwrap();
    let input = tmp.path().join("a.txt");
    let output_file = tmp.path().join("b.txt");
    fs::write(&input, "aaa").unwrap();
    fs::write(&output_file, "bbb").unwrap();

    let record_out = prv_cmd(tmp.path())
        .arg("record")
        .arg("--exit-code")
        .arg("0")
        .arg("--")
        .arg(format!("cp {} {}", input.display(), output_file.display()))
        .output()
        .expect("failed to run prv record");
    assert!(record_out.status.success());

    let dot_out = prv_cmd(tmp.path())
        .args(["dot", &output_file.to_string_lossy()])
        .output()
        .expect("failed to run prv dot");

    assert!(dot_out.status.success(), "prv dot failed: {}", stderr_str(&dot_out));
    let out = stdout_str(&dot_out);
    assert!(out.contains("digraph"), "DOT output should contain 'digraph', got: {}", out);
}

// ---------------------------------------------------------------------------
// Mermaid output
// ---------------------------------------------------------------------------

#[test]
fn dot_mermaid_outputs_graph_format() {
    let tmp = TempDir::new().unwrap();

    let output = prv_cmd(tmp.path())
        .args(["dot", "/some/file", "--mermaid"])
        .output()
        .expect("failed to run prv dot --mermaid");

    assert!(
        output.status.success(),
        "prv dot --mermaid should not crash: {}",
        stderr_str(&output)
    );
    let out = stdout_str(&output);
    assert!(
        out.contains("graph"),
        "Mermaid output should contain 'graph', got: {}",
        out
    );
}

// ---------------------------------------------------------------------------
// GC (garbage collection)
// ---------------------------------------------------------------------------

#[test]
fn gc_removes_records() {
    let tmp = TempDir::new().unwrap();
    let testfile = tmp.path().join("gc_test.txt");
    fs::write(&testfile, "gc data").unwrap();

    // Record a command so there is something to GC
    let record_out = prv_cmd(tmp.path())
        .arg("record")
        .arg("--exit-code")
        .arg("0")
        .arg("--")
        .arg(format!("cat {}", testfile.display()))
        .output()
        .expect("failed to run prv record");
    assert!(record_out.status.success());

    // Run GC with --older-than 0s to clean everything
    let gc_out = prv_cmd(tmp.path())
        .args(["gc", "--older-than", "1s"])
        .output()
        .expect("failed to run prv gc");

    assert!(gc_out.status.success(), "prv gc failed: {}", stderr_str(&gc_out));
    let out = stdout_str(&gc_out);
    assert!(
        out.contains("Removed"),
        "gc output should mention 'Removed', got: {}",
        out
    );
}

#[test]
fn gc_with_no_records_shows_zero_removed() {
    let tmp = TempDir::new().unwrap();

    let output = prv_cmd(tmp.path())
        .args(["gc", "--older-than", "1s"])
        .output()
        .expect("failed to run prv gc");

    assert!(output.status.success(), "prv gc should not crash: {}", stderr_str(&output));
    let out = stdout_str(&output);
    assert!(
        out.contains("Removed"),
        "gc output should mention 'Removed', got: {}",
        out
    );
}

// ---------------------------------------------------------------------------
// Replay
// ---------------------------------------------------------------------------

#[test]
fn replay_dry_run_does_not_crash() {
    let tmp = TempDir::new().unwrap();

    let output = prv_cmd(tmp.path())
        .args(["replay", "/some/file", "--dry-run"])
        .output()
        .expect("failed to run prv replay");

    assert!(
        output.status.success(),
        "prv replay --dry-run should not crash: {}",
        stderr_str(&output)
    );
    let out = stdout_str(&output);
    assert!(
        out.contains("No replay steps") || out.contains("Replay plan"),
        "replay output should mention steps or lack thereof, got: {}",
        out
    );
}

#[test]
fn replay_dry_run_after_record() {
    let tmp = TempDir::new().unwrap();
    let testfile = tmp.path().join("replay_target.txt");
    fs::write(&testfile, "data for replay").unwrap();

    // Record a command
    let record_out = prv_cmd(tmp.path())
        .arg("record")
        .arg("--exit-code")
        .arg("0")
        .arg("--")
        .arg(format!("cat {}", testfile.display()))
        .output()
        .expect("failed to run prv record");
    assert!(record_out.status.success());

    let replay_out = prv_cmd(tmp.path())
        .args(["replay", &testfile.to_string_lossy(), "--dry-run"])
        .output()
        .expect("failed to run prv replay --dry-run");

    assert!(
        replay_out.status.success(),
        "prv replay --dry-run should not crash: {}",
        stderr_str(&replay_out)
    );
    let out = stdout_str(&replay_out);
    assert!(
        out.contains("No replay steps") || out.contains("Replay plan"),
        "replay output should mention steps info, got: {}",
        out
    );
}

// ---------------------------------------------------------------------------
// Error cases
// ---------------------------------------------------------------------------

#[test]
fn no_subcommand_shows_help_or_error() {
    let tmp = TempDir::new().unwrap();

    let output = prv_cmd(tmp.path())
        .output()
        .expect("failed to run prv with no args");

    assert!(
        !output.status.success(),
        "prv with no subcommand should exit non-zero"
    );
    let combined = combined_output(&output);
    assert!(
        combined.contains("Usage") || combined.contains("help") || combined.contains("error"),
        "should show usage or error info, got: {}",
        combined
    );
}

#[test]
fn missing_required_args_exits_non_zero() {
    let tmp = TempDir::new().unwrap();

    // 'log' requires a file argument
    let output = prv_cmd(tmp.path())
        .args(["log"])
        .output()
        .expect("failed to run prv log without args");

    assert!(
        !output.status.success(),
        "prv log without file arg should exit non-zero"
    );
}

#[test]
fn search_missing_pattern_exits_non_zero() {
    let tmp = TempDir::new().unwrap();

    let output = prv_cmd(tmp.path())
        .args(["search"])
        .output()
        .expect("failed to run prv search without pattern");

    assert!(
        !output.status.success(),
        "prv search without pattern should exit non-zero"
    );
}

#[test]
fn gc_missing_older_than_exits_non_zero() {
    let tmp = TempDir::new().unwrap();

    let output = prv_cmd(tmp.path())
        .args(["gc"])
        .output()
        .expect("failed to run prv gc without --older-than");

    assert!(
        !output.status.success(),
        "prv gc without --older-than should exit non-zero"
    );
}

// ---------------------------------------------------------------------------
// Record without exit code
// ---------------------------------------------------------------------------

#[test]
fn record_without_exit_code_succeeds() {
    let tmp = TempDir::new().unwrap();

    let output = prv_cmd(tmp.path())
        .arg("record")
        .arg("--")
        .arg("echo hello")
        .output()
        .expect("failed to run prv record without exit code");

    assert!(
        output.status.success(),
        "prv record without --exit-code should succeed, stderr: {}",
        stderr_str(&output)
    );
}

// ---------------------------------------------------------------------------
// Multiple records then search
// ---------------------------------------------------------------------------

#[test]
fn multiple_records_searchable() {
    let tmp = TempDir::new().unwrap();
    let file_a = tmp.path().join("alpha.txt");
    let file_b = tmp.path().join("beta.txt");
    fs::write(&file_a, "alpha").unwrap();
    fs::write(&file_b, "beta").unwrap();

    // Record two different commands
    let r1 = prv_cmd(tmp.path())
        .arg("record")
        .arg("--exit-code").arg("0")
        .arg("--")
        .arg(format!("grep pattern {}", file_a.display()))
        .output()
        .expect("failed record 1");
    assert!(r1.status.success());

    let r2 = prv_cmd(tmp.path())
        .arg("record")
        .arg("--exit-code").arg("0")
        .arg("--")
        .arg(format!("wc -l {}", file_b.display()))
        .output()
        .expect("failed record 2");
    assert!(r2.status.success());

    // Search for grep
    let search_grep = prv_cmd(tmp.path())
        .args(["search", "grep"])
        .output()
        .expect("failed to search grep");
    assert!(search_grep.status.success());
    assert!(
        stdout_str(&search_grep).contains("grep"),
        "search for grep should find it"
    );

    // Search for wc
    let search_wc = prv_cmd(tmp.path())
        .args(["search", "wc"])
        .output()
        .expect("failed to search wc");
    assert!(search_wc.status.success());
    assert!(
        stdout_str(&search_wc).contains("wc"),
        "search for wc should find it"
    );
}

// ---------------------------------------------------------------------------
// Trace query
// ---------------------------------------------------------------------------

#[test]
fn trace_nonexistent_file_shows_no_records() {
    let tmp = TempDir::new().unwrap();

    let output = prv_cmd(tmp.path())
        .args(["trace", "/nonexistent/trace/target"])
        .output()
        .expect("failed to run prv trace");

    assert!(output.status.success(), "prv trace should not crash: {}", stderr_str(&output));
    let out = stdout_str(&output);
    assert!(
        out.contains("No trace records"),
        "expected 'No trace records' message, got: {}",
        out
    );
}

// ---------------------------------------------------------------------------
// Database isolation
// ---------------------------------------------------------------------------

#[test]
fn separate_temp_dirs_have_isolated_databases() {
    let tmp1 = TempDir::new().unwrap();
    let tmp2 = TempDir::new().unwrap();
    let file1 = tmp1.path().join("isolated.txt");
    fs::write(&file1, "isolated").unwrap();

    // Record in tmp1's database
    let r = prv_cmd(tmp1.path())
        .arg("record")
        .arg("--exit-code").arg("0")
        .arg("--")
        .arg(format!("cat {}", file1.display()))
        .output()
        .expect("failed record");
    assert!(r.status.success());

    // Search in tmp2's database -- should find nothing
    let search = prv_cmd(tmp2.path())
        .args(["search", "cat"])
        .output()
        .expect("failed search in tmp2");
    assert!(search.status.success());
    assert!(
        stdout_str(&search).contains("No commands matching"),
        "tmp2 db should be empty, got: {}",
        stdout_str(&search)
    );

    // Search in tmp1's database -- should find the command
    let search1 = prv_cmd(tmp1.path())
        .args(["search", "cat"])
        .output()
        .expect("failed search in tmp1");
    assert!(search1.status.success());
    assert!(
        stdout_str(&search1).contains("cat"),
        "tmp1 db should have the record, got: {}",
        stdout_str(&search1)
    );
}
