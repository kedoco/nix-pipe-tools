use crate::config::Config;
use crate::db::Database;
use chrono::Utc;
use std::io::BufRead;
use std::process::Command;

pub fn trace_command(
    command: &str,
    args: &[String],
    db: &Database,
    config: &Config,
) -> anyhow::Result<i32> {
    let tmpfile = tempfile::NamedTempFile::new()?;
    let tmppath = tmpfile.path().to_string_lossy().to_string();

    let mut strace_args = vec![
        "-f".to_string(),
        "-e".to_string(),
        "trace=openat,creat,rename,renameat,renameat2,unlink,unlinkat".to_string(),
        "-o".to_string(),
        tmppath.clone(),
        command.to_string(),
    ];
    strace_args.extend(args.iter().cloned());

    let start = std::time::Instant::now();
    let timestamp = Utc::now().to_rfc3339();
    let cwd = std::env::current_dir()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();

    let status = Command::new("strace").args(&strace_args).status()?;
    let duration_ms = start.elapsed().as_millis() as i64;
    let exit_code = status.code().unwrap_or(-1);

    let cmd_id = db.insert_command(command, args, &cwd, &timestamp, Some(duration_ms), Some(exit_code))?;

    let file = std::fs::File::open(&tmppath)?;
    let reader = std::io::BufReader::new(file);
    let event_ts = Utc::now().to_rfc3339();

    for line in reader.lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => continue,
        };
        if let Some((path, event_type)) = parse_strace_line(&line) {
            if !config.should_ignore(path) {
                db.insert_file_event(cmd_id, path, event_type, &event_ts)?;
            }
        }
    }

    Ok(exit_code)
}

fn parse_strace_line(line: &str) -> Option<(&str, &str)> {
    // Skip incomplete or resumed lines
    if line.contains("<unfinished") || line.contains("resumed>") {
        return None;
    }

    if let Some(rest) = line.strip_suffix(')') {
        let _ = rest; // we work with the full line
    }

    if line.contains("openat(") {
        let path = extract_quoted_string(line)?;
        let event_type = if line.contains("O_WRONLY") || line.contains("O_RDWR") {
            if line.contains("O_CREAT") {
                "create"
            } else {
                "write"
            }
        } else {
            "read"
        };
        Some((path, event_type))
    } else if line.contains("creat(") {
        let path = extract_quoted_string(line)?;
        Some((path, "create"))
    } else if line.contains("rename(") || line.contains("renameat(") || line.contains("renameat2(") {
        // For rename, we track as rename event on the destination
        let path = extract_last_quoted_string(line)?;
        Some((path, "rename"))
    } else if line.contains("unlink(") || line.contains("unlinkat(") {
        let path = extract_quoted_string(line)?;
        Some((path, "delete"))
    } else {
        None
    }
}

fn extract_quoted_string(line: &str) -> Option<&str> {
    let start = line.find('"')? + 1;
    let end = start + line[start..].find('"')?;
    Some(&line[start..end])
}

fn extract_last_quoted_string(line: &str) -> Option<&str> {
    let mut last_start = None;
    let mut last_end = None;
    let bytes = line.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'"' {
            let start = i + 1;
            if let Some(end_offset) = line[start..].find('"') {
                last_start = Some(start);
                last_end = Some(start + end_offset);
                i = start + end_offset + 1;
            } else {
                break;
            }
        } else {
            i += 1;
        }
    }
    Some(&line[last_start?..last_end?])
}

// Re-export for use by main.rs on linux
pub use trace_command as wrap_command;
