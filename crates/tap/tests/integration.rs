use std::io::Write;
use std::process::{Command, Output, Stdio};

fn tap_cmd(tmpdir: &std::path::Path) -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_tap"));
    cmd.env("TMPDIR", tmpdir);
    cmd.env("HOME", tmpdir);
    // Use a unique USER based on tmpdir to isolate /tmp/tap-{USER}/ per test
    let user = tmpdir
        .file_name()
        .unwrap()
        .to_string_lossy()
        .to_string();
    cmd.env("USER", &user);
    cmd.stdout(Stdio::piped()).stderr(Stdio::piped());
    cmd
}

fn run_with_stdin(tmpdir: &std::path::Path, args: &[&str], input: &[u8]) -> Output {
    let mut cmd = tap_cmd(tmpdir);
    cmd.args(args).stdin(Stdio::piped());
    let mut child = cmd.spawn().unwrap();
    child.stdin.take().unwrap().write_all(input).unwrap();
    child.wait_with_output().unwrap()
}

fn run_no_stdin(tmpdir: &std::path::Path, args: &[&str]) -> Output {
    let mut cmd = tap_cmd(tmpdir);
    cmd.args(args).stdin(Stdio::null());
    cmd.output().unwrap()
}

fn stdout_str(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).to_string()
}

fn stderr_str(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).to_string()
}

/// Clean up the /tmp/tap-{USER} directory created by the test.
fn cleanup(tmpdir: &std::path::Path) {
    let user = tmpdir
        .file_name()
        .unwrap()
        .to_string_lossy()
        .to_string();
    let tap_dir = std::path::PathBuf::from(format!("/tmp/tap-{}", user));
    let _ = std::fs::remove_dir_all(&tap_dir);
}

// ---------------------------------------------------------------------------
// Passthrough (no -n flag)
// ---------------------------------------------------------------------------

#[test]
fn passthrough_relays_stdin_to_stdout() {
    let tmp = tempfile::tempdir().unwrap();
    let input = b"hello\nworld\n";
    let out = run_with_stdin(tmp.path(), &[], input);

    assert!(out.status.success(), "exit code: {:?}", out.status);
    assert_eq!(stdout_str(&out), "hello\nworld\n");
    // stderr should be empty (no capture name => no summary)
    assert!(
        stderr_str(&out).is_empty(),
        "stderr should be empty, got: {:?}",
        stderr_str(&out)
    );
    cleanup(tmp.path());
}

#[test]
fn passthrough_creates_no_files() {
    let tmp = tempfile::tempdir().unwrap();
    let user = tmp.path().file_name().unwrap().to_string_lossy().to_string();
    let tap_dir = std::path::PathBuf::from(format!("/tmp/tap-{}", user));

    let input = b"data\n";
    let out = run_with_stdin(tmp.path(), &[], input);
    assert!(out.status.success());

    // No session directory should have been created
    let sessions_dir = tap_dir.join("sessions");
    assert!(
        !sessions_dir.exists() || std::fs::read_dir(&sessions_dir).unwrap().count() == 0,
        "no session files should be created in passthrough mode"
    );
    cleanup(tmp.path());
}

// ---------------------------------------------------------------------------
// Capture mode (-n)
// ---------------------------------------------------------------------------

#[test]
fn capture_relays_and_prints_summary() {
    let tmp = tempfile::tempdir().unwrap();
    let input = b"line1\nline2\nline3\n";
    let out = run_with_stdin(tmp.path(), &["-n", "testcap"], input);

    assert!(out.status.success(), "exit: {:?}", out.status);
    assert_eq!(stdout_str(&out), "line1\nline2\nline3\n");

    let err = stderr_str(&out);
    assert!(
        err.contains("tap: testcap"),
        "stderr should contain 'tap: testcap', got: {:?}",
        err
    );
    cleanup(tmp.path());
}

// ---------------------------------------------------------------------------
// Show captured data
// ---------------------------------------------------------------------------

#[test]
fn show_displays_captured_data() {
    let tmp = tempfile::tempdir().unwrap();
    let input = b"show_data_line1\nshow_data_line2\n";

    // Capture first
    let cap = run_with_stdin(tmp.path(), &["-n", "mydata"], input);
    assert!(cap.status.success(), "capture failed: {:?}", stderr_str(&cap));

    // Show - set PAGER=cat and TERM="" to avoid pager issues
    let mut cmd = tap_cmd(tmp.path());
    cmd.args(["show", "mydata"])
        .stdin(Stdio::null())
        .env("PAGER", "cat")
        .env("TERM", "");
    let show_out = cmd.output().unwrap();

    assert!(show_out.status.success(), "show failed: {:?}", stderr_str(&show_out));
    let shown = stdout_str(&show_out);
    assert!(
        shown.contains("show_data_line1"),
        "show output should contain captured data, got: {:?}",
        shown
    );
    assert!(
        shown.contains("show_data_line2"),
        "show output should contain all captured lines, got: {:?}",
        shown
    );
    cleanup(tmp.path());
}

// ---------------------------------------------------------------------------
// Replay
// ---------------------------------------------------------------------------

#[test]
fn replay_outputs_captured_data() {
    let tmp = tempfile::tempdir().unwrap();
    let input = b"replay_line_a\nreplay_line_b\n";

    let cap = run_with_stdin(tmp.path(), &["-n", "replaytest"], input);
    assert!(cap.status.success());

    let replay = run_no_stdin(tmp.path(), &["replay", "replaytest"]);
    assert!(replay.status.success(), "replay failed: {:?}", stderr_str(&replay));
    assert_eq!(
        stdout_str(&replay),
        "replay_line_a\nreplay_line_b\n",
        "replayed data must match original input"
    );
    cleanup(tmp.path());
}

// ---------------------------------------------------------------------------
// Stats
// ---------------------------------------------------------------------------

#[test]
fn stats_lists_captured_names() {
    let tmp = tempfile::tempdir().unwrap();
    let input = b"stats_data\n";

    let cap = run_with_stdin(tmp.path(), &["-n", "statstest"], input);
    assert!(cap.status.success());

    let stats = run_no_stdin(tmp.path(), &["stats"]);
    assert!(stats.status.success(), "stats failed: {:?}", stderr_str(&stats));

    let out = stdout_str(&stats);
    assert!(
        out.contains("statstest"),
        "stats output should contain 'statstest', got: {:?}",
        out
    );
    cleanup(tmp.path());
}

// ---------------------------------------------------------------------------
// Line limit (-l)
// ---------------------------------------------------------------------------

#[test]
fn line_limit_passes_all_data_through() {
    let tmp = tempfile::tempdir().unwrap();
    let input: String = (1..=100).map(|i| format!("line{}\n", i)).collect();

    let out = run_with_stdin(tmp.path(), &["-n", "limited", "-l", "5"], input.as_bytes());

    assert!(out.status.success(), "exit: {:?}", out.status);

    // All 100 lines must pass through stdout (passthrough is not limited)
    let stdout = stdout_str(&out);
    assert_eq!(
        stdout.lines().count(),
        100,
        "all 100 lines should pass through stdout"
    );
    assert!(stdout.contains("line1\n"), "first line should be present");
    assert!(stdout.contains("line100\n"), "last line should be present");

    // stderr should have capture summary
    let err = stderr_str(&out);
    assert!(
        err.contains("tap: limited"),
        "stderr should mention the capture name, got: {:?}",
        err
    );
    cleanup(tmp.path());
}

// ---------------------------------------------------------------------------
// Summary mode (-s)
// ---------------------------------------------------------------------------

#[test]
fn summary_mode_passes_data_through() {
    let tmp = tempfile::tempdir().unwrap();
    let input = b"sum1\nsum2\nsum3\n";

    let out = run_with_stdin(tmp.path(), &["-n", "sumtest", "-s"], input);

    assert!(out.status.success(), "exit: {:?}", out.status);
    assert_eq!(stdout_str(&out), "sum1\nsum2\nsum3\n");

    let err = stderr_str(&out);
    assert!(
        err.contains("tap: sumtest"),
        "stderr should contain summary, got: {:?}",
        err
    );
    cleanup(tmp.path());
}

// ---------------------------------------------------------------------------
// Sessions
// ---------------------------------------------------------------------------

#[test]
fn sessions_lists_sessions_after_capture() {
    let tmp = tempfile::tempdir().unwrap();
    let input = b"session_data\n";

    let cap = run_with_stdin(tmp.path(), &["-n", "sesstest"], input);
    assert!(cap.status.success());

    let sessions = run_no_stdin(tmp.path(), &["sessions"]);
    assert!(sessions.status.success(), "sessions failed: {:?}", stderr_str(&sessions));

    let out = stdout_str(&sessions);
    // Should list at least one session (format: "{ppid}-{epoch} (N captures, ...)")
    assert!(
        out.contains("captures"),
        "sessions output should list at least one session, got: {:?}",
        out
    );
    cleanup(tmp.path());
}

// ---------------------------------------------------------------------------
// Last
// ---------------------------------------------------------------------------

#[test]
fn last_shows_most_recent_session() {
    let tmp = tempfile::tempdir().unwrap();
    let input = b"last_data\n";

    let cap = run_with_stdin(tmp.path(), &["-n", "lasttest"], input);
    assert!(cap.status.success());

    let last = run_no_stdin(tmp.path(), &["last"]);
    assert!(last.status.success(), "last failed: {:?}", stderr_str(&last));

    let out = stdout_str(&last);
    assert!(
        out.contains("Session:"),
        "last output should contain 'Session:', got: {:?}",
        out
    );
    assert!(
        out.contains("lasttest"),
        "last output should contain the capture name, got: {:?}",
        out
    );
    cleanup(tmp.path());
}

// ---------------------------------------------------------------------------
// Clean
// ---------------------------------------------------------------------------

#[test]
fn clean_does_not_error() {
    let tmp = tempfile::tempdir().unwrap();
    let input = b"clean_data\n";

    let cap = run_with_stdin(tmp.path(), &["-n", "cleantest"], input);
    assert!(cap.status.success());

    // Sleep briefly so the session is at least 1s old, then clean it
    std::thread::sleep(std::time::Duration::from_secs(2));
    let clean = run_no_stdin(tmp.path(), &["clean", "--older-than", "1s"]);
    assert!(
        clean.status.success(),
        "clean failed: {:?}\nstdout: {:?}\nstderr: {:?}",
        clean.status,
        stdout_str(&clean),
        stderr_str(&clean)
    );
    cleanup(tmp.path());
}

// ---------------------------------------------------------------------------
// Format detection (-f)
// ---------------------------------------------------------------------------

#[test]
fn format_detection_identifies_json() {
    let tmp = tempfile::tempdir().unwrap();
    let input = b"{\"key\":\"val\"}\n";

    let out = run_with_stdin(tmp.path(), &["-n", "jsontest", "-f"], input);

    assert!(out.status.success(), "exit: {:?}", out.status);

    let err = stderr_str(&out);
    assert!(
        err.contains("json"),
        "stderr should mention json format, got: {:?}",
        err
    );
    cleanup(tmp.path());
}

// ---------------------------------------------------------------------------
// Empty input
// ---------------------------------------------------------------------------

#[test]
fn empty_input_no_error() {
    let tmp = tempfile::tempdir().unwrap();
    let out = run_with_stdin(tmp.path(), &["-n", "empty"], b"");

    assert!(
        out.status.success(),
        "empty input should not cause an error, exit: {:?}, stderr: {:?}",
        out.status,
        stderr_str(&out)
    );
    // stdout should be empty
    assert_eq!(stdout_str(&out), "");
    cleanup(tmp.path());
}

// ---------------------------------------------------------------------------
// Capture then replay round-trip with binary-safe data
// ---------------------------------------------------------------------------

#[test]
fn capture_replay_round_trip() {
    let tmp = tempfile::tempdir().unwrap();
    let input = b"alpha\nbeta\ngamma\ndelta\n";

    let cap = run_with_stdin(tmp.path(), &["-n", "roundtrip"], input);
    assert!(cap.status.success());
    // Passthrough must match
    assert_eq!(stdout_str(&cap), "alpha\nbeta\ngamma\ndelta\n");

    let replay = run_no_stdin(tmp.path(), &["replay", "roundtrip"]);
    assert!(replay.status.success());
    assert_eq!(
        stdout_str(&replay),
        "alpha\nbeta\ngamma\ndelta\n",
        "replay should exactly match captured input"
    );
    cleanup(tmp.path());
}
