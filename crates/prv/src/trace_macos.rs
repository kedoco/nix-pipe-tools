use crate::config::Config;
use crate::db::Database;
use chrono::Utc;
use std::io::BufRead;
use std::process::{Command, Stdio};

pub fn trace_command(
    command: &str,
    args: &[String],
    db: &Database,
    config: &Config,
) -> anyhow::Result<i32> {
    let start = std::time::Instant::now();
    let timestamp = Utc::now().to_rfc3339();
    let cwd = std::env::current_dir()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();

    // Start the actual command as a child process
    let mut child = Command::new(command)
        .args(args)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()?;

    let pid = child.id();

    // Start fs_usage to monitor the child
    let fs_usage = Command::new("sudo")
        .args(["fs_usage", "-w", "-f", "filesys", &pid.to_string()])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn();

    let status = child.wait()?;
    let duration_ms = start.elapsed().as_millis() as i64;
    let exit_code = status.code().unwrap_or(-1);

    let cmd_id = db.insert_command(
        command,
        args,
        &cwd,
        &timestamp,
        Some(duration_ms),
        Some(exit_code),
    )?;

    // Process fs_usage output if we managed to start it
    if let Ok(mut fs_proc) = fs_usage {
        // Give it a moment to flush, then kill
        let _ = nix::sys::signal::kill(
            nix::unistd::Pid::from_raw(fs_proc.id() as i32),
            nix::sys::signal::Signal::SIGTERM,
        );

        if let Some(stdout) = fs_proc.stdout.take() {
            let reader = std::io::BufReader::new(stdout);
            let event_ts = Utc::now().to_rfc3339();

            for line in reader.lines() {
                let line = match line {
                    Ok(l) => l,
                    Err(_) => continue,
                };
                if let Some((path, event_type)) = parse_fs_usage_line(&line) {
                    if !config.should_ignore(&path) {
                        db.insert_file_event(cmd_id, &path, event_type, &event_ts)?;
                    }
                }
            }
        }

        let _ = fs_proc.wait();
    }

    Ok(exit_code)
}

fn parse_fs_usage_line(line: &str) -> Option<(String, &'static str)> {
    let line = line.trim();
    if line.is_empty() {
        return None;
    }

    // fs_usage output format varies but generally:
    // timestamp  operation  path  ...
    // We look for known operations and extract paths

    let event_type = if line.contains("open") {
        if line.contains("W") || line.contains("RW") {
            "write"
        } else {
            "read"
        }
    } else if line.contains("create") || line.contains("Create") {
        "create"
    } else if line.contains("rename") || line.contains("Rename") {
        "rename"
    } else if line.contains("unlink") || line.contains("delete") {
        "delete"
    } else {
        return None;
    };

    // Extract path: look for absolute path starting with /
    let path = extract_path(line)?;
    Some((path.to_string(), event_type))
}

fn extract_path(line: &str) -> Option<&str> {
    // Find a path component starting with /
    for (i, _) in line.match_indices('/') {
        // Find the end of the path (space or end of line)
        let rest = &line[i..];
        let end = rest
            .find(|c: char| c.is_whitespace())
            .unwrap_or(rest.len());
        let path = &rest[..end];
        // Basic validation: must have at least one more char after /
        if path.len() > 1 {
            return Some(path);
        }
    }
    None
}

pub use trace_command as wrap_command;
