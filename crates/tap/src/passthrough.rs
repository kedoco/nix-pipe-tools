use crossbeam_channel::Sender;
use std::io::{self, Read, Write};

const BUF_SIZE: usize = 64 * 1024;

/// Relay stdin to stdout in 64KB chunks.
/// If a sender is provided, send copies of each chunk to the capture thread.
/// Returns (total_bytes, total_lines).
pub fn relay(sender: Option<&Sender<Vec<u8>>>) -> io::Result<(u64, u64)> {
    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut reader = stdin.lock();
    let mut writer = stdout.lock();
    let mut buf = [0u8; BUF_SIZE];
    let mut total_bytes: u64 = 0;
    let mut total_lines: u64 = 0;

    loop {
        let n = reader.read(&mut buf)?;
        if n == 0 {
            break;
        }
        writer.write_all(&buf[..n])?;

        total_bytes += n as u64;
        total_lines += bytecount(&buf[..n]);

        if let Some(tx) = sender {
            // Non-blocking send: if channel is full, we drop the chunk
            // The capture thread will detect this via the truncated flag
            let _ = tx.try_send(buf[..n].to_vec());
        }
    }
    writer.flush()?;
    Ok((total_bytes, total_lines))
}

fn bytecount(data: &[u8]) -> u64 {
    data.iter().filter(|&&b| b == b'\n').count() as u64
}
