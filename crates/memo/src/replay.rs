use crate::exec::InterleaveEntry;
use std::io::{self, Write};

/// Replay cached output, preserving stdout/stderr interleave order.
pub fn replay(
    stdout_data: &[u8],
    stderr_data: &[u8],
    interleave_log: &[u8],
) -> io::Result<()> {
    let mut stdout_offset = 0usize;
    let mut stderr_offset = 0usize;

    let stdout = io::stdout();
    let stderr = io::stderr();
    let mut stdout_lock = stdout.lock();
    let mut stderr_lock = stderr.lock();

    for line in interleave_log.split(|&b| b == b'\n') {
        if line.is_empty() {
            continue;
        }
        let entry: InterleaveEntry = match serde_json::from_slice(line) {
            Ok(e) => e,
            Err(_) => continue,
        };
        match entry.fd {
            1 => {
                let end = (stdout_offset + entry.len).min(stdout_data.len());
                stdout_lock.write_all(&stdout_data[stdout_offset..end])?;
                stdout_offset = end;
            }
            2 => {
                let end = (stderr_offset + entry.len).min(stderr_data.len());
                stderr_lock.write_all(&stderr_data[stderr_offset..end])?;
                stderr_offset = end;
            }
            _ => {}
        }
    }

    stdout_lock.flush()?;
    stderr_lock.flush()?;
    Ok(())
}
