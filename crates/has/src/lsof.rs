use std::path::Path;
use std::process::Command;

use crate::types::Entry;

/// Query lsof for processes holding the given file open.
pub fn query_file(path: &Path) -> Result<Vec<Entry>, String> {
    let path_str = path.to_str().ok_or("path contains invalid UTF-8")?;
    run_lsof(&[path_str])
}

/// Query lsof for processes using the given port.
pub fn query_port(port: u16) -> Result<Vec<Entry>, String> {
    run_lsof(&["-i", &format!(":{}", port)])
}

/// Query lsof for resources held by the given PID.
pub fn query_pid(pid: u32) -> Result<Vec<Entry>, String> {
    run_lsof(&["-p", &pid.to_string()])
}

fn run_lsof(extra_args: &[&str]) -> Result<Vec<Entry>, String> {
    let mut cmd = Command::new("lsof");
    // -F for machine-parseable output
    // Fields: p=PID, c=command, L=login name, f=fd, t=type, a=access, n=name
    cmd.arg("-F").arg("pcLftan");
    cmd.args(extra_args);

    let output = cmd
        .output()
        .map_err(|e| format!("failed to run lsof: {}", e))?;

    // lsof exits 1 when no results found — that's not an error
    if !output.status.success() && output.stdout.is_empty() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stderr = stderr.trim();
        if !stderr.is_empty() && !stderr.contains("No such process") {
            let errors: Vec<&str> = stderr
                .lines()
                .filter(|l| !l.starts_with("lsof: WARNING:"))
                .collect();
            if !errors.is_empty() {
                return Err(errors.join("\n"));
            }
        }
        return Ok(Vec::new());
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    parse_output(&stdout)
}

/// Parse lsof -F output into entries.
///
/// The -F format emits one field per line, where the first character
/// identifies the field type:
///   p = PID, c = command, L = login name,
///   f = file descriptor, t = file type, a = access mode, n = name
///
/// Process-level fields (p, c, L) apply to all subsequent file-level
/// entries until the next process block.
fn parse_output(output: &str) -> Result<Vec<Entry>, String> {
    let mut entries = Vec::new();

    let mut pid = String::new();
    let mut command = String::new();
    let mut user = String::new();

    let mut fd = String::new();
    let mut file_type = String::new();
    let mut access = String::new();
    let mut name: Option<String> = None;

    for line in output.lines() {
        if line.is_empty() {
            continue;
        }

        let (tag, value) = line.split_at(1);
        match tag {
            "p" => {
                if let Some(n) = name.take() {
                    entries.push(Entry {
                        pid: pid.clone(),
                        command: command.clone(),
                        user: user.clone(),
                        fd: fd.clone(),
                        file_type: file_type.clone(),
                        access: access.clone(),
                        name: n,
                    });
                    fd.clear();
                    file_type.clear();
                    access.clear();
                }
                pid = value.to_string();
                command.clear();
                user.clear();
            }
            "c" => command = value.to_string(),
            "L" => user = value.to_string(),
            "f" => {
                if let Some(n) = name.take() {
                    entries.push(Entry {
                        pid: pid.clone(),
                        command: command.clone(),
                        user: user.clone(),
                        fd: fd.clone(),
                        file_type: file_type.clone(),
                        access: access.clone(),
                        name: n,
                    });
                }
                fd = value.to_string();
                file_type.clear();
                access.clear();
            }
            "t" => file_type = value.to_string(),
            "a" => access = value.to_string(),
            "n" => name = Some(value.to_string()),
            _ => {}
        }
    }

    if let Some(n) = name.take() {
        entries.push(Entry {
            pid: pid.clone(),
            command: command.clone(),
            user: user.clone(),
            fd: fd.clone(),
            file_type: file_type.clone(),
            access: access.clone(),
            name: n,
        });
    }

    Ok(entries)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_single_process_single_file() {
        let input = "p1234\ncpython\nLkevin\nf3\ntREG\nar\nn/tmp/data.db\n";
        let entries = parse_output(input).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].pid, "1234");
        assert_eq!(entries[0].command, "python");
        assert_eq!(entries[0].user, "kevin");
        assert_eq!(entries[0].fd, "3");
        assert_eq!(entries[0].file_type, "REG");
        assert_eq!(entries[0].access, "r");
        assert_eq!(entries[0].name, "/tmp/data.db");
    }

    #[test]
    fn parse_single_process_multiple_files() {
        let input = "p1234\ncnode\nLroot\nf4\ntIPv4\nau\nn*:8080\nf5\ntREG\nar\nn/var/log/app.log\n";
        let entries = parse_output(input).unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].fd, "4");
        assert_eq!(entries[0].name, "*:8080");
        assert_eq!(entries[1].fd, "5");
        assert_eq!(entries[1].name, "/var/log/app.log");
        assert_eq!(entries[0].pid, "1234");
        assert_eq!(entries[1].pid, "1234");
    }

    #[test]
    fn parse_multiple_processes() {
        let input = "p100\ncpython\nLkevin\nf3\ntREG\narw\nn/tmp/db\np200\ncsqlite3\nLkevin\nf5\ntREG\nar\nn/tmp/db\n";
        let entries = parse_output(input).unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].pid, "100");
        assert_eq!(entries[0].command, "python");
        assert_eq!(entries[0].access, "rw");
        assert_eq!(entries[1].pid, "200");
        assert_eq!(entries[1].command, "sqlite3");
        assert_eq!(entries[1].access, "r");
    }

    #[test]
    fn parse_empty_input() {
        let entries = parse_output("").unwrap();
        assert!(entries.is_empty());
    }
}
