use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Output, Stdio};

fn memo_cmd(home: &std::path::Path) -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_memo"));
    cmd.env("HOME", home);
    cmd.stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    cmd
}

fn run_with_stdin(home: &std::path::Path, args: &[&str], input: &[u8]) -> Output {
    let mut cmd = memo_cmd(home);
    cmd.args(args).stdin(Stdio::piped());
    let mut child = cmd.spawn().unwrap();
    child.stdin.take().unwrap().write_all(input).unwrap();
    child.wait_with_output().unwrap()
}

fn stderr_str(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).to_string()
}

fn stdout_str(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).to_string()
}

fn make_home() -> (tempfile::TempDir, PathBuf) {
    let tmp = tempfile::tempdir().unwrap();
    let p = tmp.path().to_path_buf();
    (tmp, p)
}

// ---------------------------------------------------------------------------
// Basic caching
// ---------------------------------------------------------------------------

#[test]
fn basic_cache_miss_then_hit() {
    let (_tmp, home) = make_home();

    // First run: MISS
    let out1 = memo_cmd(&home)
        .args(["--verbose", "echo", "hello"])
        .output()
        .unwrap();
    assert_eq!(stdout_str(&out1), "hello\n");
    assert!(
        stderr_str(&out1).contains("MISS"),
        "expected MISS in stderr, got: {}",
        stderr_str(&out1)
    );

    // Second run: HIT
    let out2 = memo_cmd(&home)
        .args(["--verbose", "echo", "hello"])
        .output()
        .unwrap();
    assert_eq!(stdout_str(&out2), "hello\n");
    assert!(
        stderr_str(&out2).contains("HIT"),
        "expected HIT in stderr, got: {}",
        stderr_str(&out2)
    );
}

// ---------------------------------------------------------------------------
// Stdout preservation
// ---------------------------------------------------------------------------

#[test]
fn stdout_preserved_exactly() {
    let (_tmp, home) = make_home();

    let out = memo_cmd(&home)
        .args(["echo", "hello world"])
        .output()
        .unwrap();
    assert_eq!(stdout_str(&out), "hello world\n");
}

// ---------------------------------------------------------------------------
// Exit code preservation
// ---------------------------------------------------------------------------

#[test]
fn exit_code_preserved() {
    let (_tmp, home) = make_home();

    // First run
    let out1 = memo_cmd(&home)
        .args(["sh", "-c", "exit 42"])
        .output()
        .unwrap();
    assert_eq!(out1.status.code(), Some(42));

    // Second run (from cache) should also return 42
    let out2 = memo_cmd(&home)
        .args(["sh", "-c", "exit 42"])
        .output()
        .unwrap();
    assert_eq!(out2.status.code(), Some(42));
}

// ---------------------------------------------------------------------------
// Stdin caching
// ---------------------------------------------------------------------------

#[test]
fn stdin_produces_correct_output() {
    let (_tmp, home) = make_home();

    let out = run_with_stdin(&home, &["cat"], b"hello\n");
    assert!(out.status.success());
    assert_eq!(stdout_str(&out), "hello\n");
}

// ---------------------------------------------------------------------------
// show-key
// ---------------------------------------------------------------------------

#[test]
fn show_key_prints_hex_string() {
    let (_tmp, home) = make_home();

    let out = memo_cmd(&home)
        .args(["show-key", "echo", "hello"])
        .output()
        .unwrap();
    assert!(out.status.success());
    let key = stdout_str(&out).trim().to_string();
    // SHA-256 hex digest is 64 characters
    assert_eq!(key.len(), 64, "expected 64-char hex key, got: {}", key);
    assert!(
        key.chars().all(|c| c.is_ascii_hexdigit()),
        "key contains non-hex characters: {}",
        key
    );
}

#[test]
fn show_key_deterministic() {
    let (_tmp, home) = make_home();

    let out1 = memo_cmd(&home)
        .args(["show-key", "echo", "hello"])
        .output()
        .unwrap();
    let out2 = memo_cmd(&home)
        .args(["show-key", "echo", "hello"])
        .output()
        .unwrap();
    assert_eq!(stdout_str(&out1), stdout_str(&out2));
}

#[test]
fn show_key_different_for_different_args() {
    let (_tmp, home) = make_home();

    let out1 = memo_cmd(&home)
        .args(["show-key", "echo", "hello"])
        .output()
        .unwrap();
    let out2 = memo_cmd(&home)
        .args(["show-key", "echo", "world"])
        .output()
        .unwrap();
    assert_ne!(
        stdout_str(&out1).trim(),
        stdout_str(&out2).trim(),
        "different args should produce different keys"
    );
}

// ---------------------------------------------------------------------------
// bust
// ---------------------------------------------------------------------------

#[test]
fn bust_invalidates_cache() {
    let (_tmp, home) = make_home();
    let memo = env!("CARGO_BIN_EXE_memo");

    // Use `script` to run memo in a pty so that stdin appears as a terminal.
    // This ensures the cache key matches between cmd_run and bust/show-key
    // (both compute keys with stdin_hash=None when stdin is a tty).
    // macOS: script -q /dev/null sh -c "..."
    // Linux: script -qc "..." /dev/null
    let script_run = |args: &str| -> Output {
        let mut cmd = Command::new("script");
        if cfg!(target_os = "macos") {
            cmd.args(["-q", "/dev/null", "sh", "-c", args]);
        } else {
            cmd.args(["-qc", args, "/dev/null"]);
        }
        cmd.env("HOME", &home)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .unwrap()
    };

    // First run: MISS
    let out1 = script_run(&format!("{} --verbose echo busttest", memo));
    let combined1 = stdout_str(&out1);
    assert!(
        combined1.contains("MISS"),
        "expected MISS on first run, got: {}",
        combined1
    );

    // Second run: HIT
    let out2 = script_run(&format!("{} --verbose echo busttest", memo));
    let combined2 = stdout_str(&out2);
    assert!(
        combined2.contains("HIT"),
        "expected HIT on second run, got: {}",
        combined2
    );

    // Bust the cache
    let bust = script_run(&format!("{} bust echo busttest", memo));
    assert!(bust.status.success(), "bust command failed");

    // Third run: MISS again
    let out3 = script_run(&format!("{} --verbose echo busttest", memo));
    let combined3 = stdout_str(&out3);
    assert!(
        combined3.contains("MISS"),
        "expected MISS after bust, got: {}",
        combined3
    );
}

// ---------------------------------------------------------------------------
// purge
// ---------------------------------------------------------------------------

#[test]
fn purge_clears_cache() {
    let (_tmp, home) = make_home();

    // Populate cache
    memo_cmd(&home)
        .args(["echo", "purgetest"])
        .output()
        .unwrap();

    // Purge
    let purge = memo_cmd(&home)
        .args(["purge"])
        .output()
        .unwrap();
    assert!(purge.status.success());

    // Should be MISS
    let out = memo_cmd(&home)
        .args(["--verbose", "echo", "purgetest"])
        .output()
        .unwrap();
    assert!(
        stderr_str(&out).contains("MISS"),
        "expected MISS after purge, got: {}",
        stderr_str(&out)
    );
}

// ---------------------------------------------------------------------------
// stats
// ---------------------------------------------------------------------------

#[test]
fn stats_fresh_home() {
    let (_tmp, home) = make_home();

    let out = memo_cmd(&home)
        .args(["stats"])
        .output()
        .unwrap();
    assert!(out.status.success());
    let s = stdout_str(&out);
    assert!(s.contains("hits:    0"), "expected hits: 0, got: {}", s);
    assert!(s.contains("misses:  0"), "expected misses: 0, got: {}", s);
}

#[test]
fn stats_records_hits_and_misses() {
    let (_tmp, home) = make_home();

    // MISS
    memo_cmd(&home)
        .args(["--verbose", "echo", "statstest"])
        .output()
        .unwrap();

    // HIT
    memo_cmd(&home)
        .args(["--verbose", "echo", "statstest"])
        .output()
        .unwrap();

    let out = memo_cmd(&home)
        .args(["stats"])
        .output()
        .unwrap();
    let s = stdout_str(&out);
    assert!(s.contains("hits:    1"), "expected hits: 1, got: {}", s);
    assert!(s.contains("misses:  1"), "expected misses: 1, got: {}", s);
}

// ---------------------------------------------------------------------------
// gc
// ---------------------------------------------------------------------------

#[test]
fn gc_evicts_entries() {
    let (_tmp, home) = make_home();

    // Populate cache
    memo_cmd(&home)
        .args(["echo", "gctest"])
        .output()
        .unwrap();

    // GC with max-size 0 to evict everything
    let gc = memo_cmd(&home)
        .args(["gc", "--max-size", "0"])
        .output()
        .unwrap();
    assert!(gc.status.success());
    let gc_stderr = stderr_str(&gc);
    assert!(
        gc_stderr.contains("removed"),
        "expected 'removed' in gc stderr, got: {}",
        gc_stderr
    );

    // Should be MISS after eviction
    let out = memo_cmd(&home)
        .args(["--verbose", "echo", "gctest"])
        .output()
        .unwrap();
    assert!(
        stderr_str(&out).contains("MISS"),
        "expected MISS after gc, got: {}",
        stderr_str(&out)
    );
}

// ---------------------------------------------------------------------------
// tag changes key
// ---------------------------------------------------------------------------

#[test]
fn tag_changes_cache_key() {
    let (_tmp, home) = make_home();

    let out1 = memo_cmd(&home)
        .args(["--tag", "v1", "show-key", "echo", "hello"])
        .output()
        .unwrap();
    let out2 = memo_cmd(&home)
        .args(["--tag", "v2", "show-key", "echo", "hello"])
        .output()
        .unwrap();
    assert!(out1.status.success());
    assert!(out2.status.success());
    assert_ne!(
        stdout_str(&out1).trim(),
        stdout_str(&out2).trim(),
        "different tags should produce different keys"
    );
}

// ---------------------------------------------------------------------------
// Error: no command
// ---------------------------------------------------------------------------

#[test]
fn no_command_exits_nonzero() {
    let (_tmp, home) = make_home();

    let out = memo_cmd(&home).output().unwrap();
    assert!(
        !out.status.success(),
        "expected non-zero exit code with no args"
    );
    assert!(
        stderr_str(&out).contains("memo:"),
        "expected 'memo:' in stderr, got: {}",
        stderr_str(&out)
    );
}
