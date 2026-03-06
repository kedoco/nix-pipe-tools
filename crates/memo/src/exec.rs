use serde::{Deserialize, Serialize};
use std::io::{self, Read, Write};
use std::path::Path;
use std::process::{Command, Stdio};

/// A single interleave record: which fd and how many bytes.
#[derive(Debug, Serialize, Deserialize)]
pub struct InterleaveEntry {
    pub fd: u8,
    pub len: usize,
}

/// Result of executing a command.
pub struct ExecResult {
    pub exit_code: i32,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    pub interleave_log: Vec<u8>,
    pub duration_ms: u64,
}

/// Execute a command, capturing stdout/stderr with interleave ordering.
///
/// Uses two reader threads to capture stdout and stderr concurrently,
/// sending chunks through a channel to preserve interleave ordering.
pub fn run_command(
    command: &Path,
    args: &[String],
    stdin_file: Option<&Path>,
) -> io::Result<ExecResult> {
    use std::sync::mpsc;
    use std::time::Instant;

    let start = Instant::now();

    let stdin_cfg = if stdin_file.is_some() {
        Stdio::piped()
    } else {
        Stdio::inherit()
    };

    let mut child = Command::new(command)
        .args(args)
        .stdin(stdin_cfg)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    // Feed stdin from file if provided
    if let Some(path) = stdin_file {
        let data = std::fs::read(path)?;
        if let Some(mut stdin) = child.stdin.take() {
            std::thread::spawn(move || {
                let _ = stdin.write_all(&data);
            });
        }
    }

    let stdout_pipe = child.stdout.take().unwrap();
    let stderr_pipe = child.stderr.take().unwrap();

    // Channel for interleaved chunks: (fd, data)
    let (tx, rx) = mpsc::channel::<(u8, Vec<u8>)>();

    let tx1 = tx.clone();
    let t1 = std::thread::spawn(move || {
        read_pipe(stdout_pipe, 1, tx1);
    });

    let tx2 = tx;
    let t2 = std::thread::spawn(move || {
        read_pipe(stderr_pipe, 2, tx2);
    });

    // Collect interleaved output
    let mut stdout_buf = Vec::new();
    let mut stderr_buf = Vec::new();
    let mut interleave: Vec<InterleaveEntry> = Vec::new();

    for (fd, data) in rx {
        let len = data.len();
        match fd {
            1 => stdout_buf.extend_from_slice(&data),
            2 => stderr_buf.extend_from_slice(&data),
            _ => {}
        }
        interleave.push(InterleaveEntry { fd, len });
    }

    let _ = t1.join();
    let _ = t2.join();

    let status = child.wait()?;
    let duration_ms = start.elapsed().as_millis() as u64;

    // Serialize interleave log as newline-delimited JSON
    let mut log_buf = Vec::new();
    for entry in &interleave {
        serde_json::to_writer(&mut log_buf, entry)?;
        log_buf.push(b'\n');
    }

    Ok(ExecResult {
        exit_code: status.code().unwrap_or(-1),
        stdout: stdout_buf,
        stderr: stderr_buf,
        interleave_log: log_buf,
        duration_ms,
    })
}

fn read_pipe<R: Read>(
    mut pipe: R,
    fd: u8,
    tx: std::sync::mpsc::Sender<(u8, Vec<u8>)>,
) {
    let mut buf = [0u8; 64 * 1024];
    loop {
        match pipe.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => {
                if tx.send((fd, buf[..n].to_vec())).is_err() {
                    break;
                }
            }
            Err(_) => break,
        }
    }
}
