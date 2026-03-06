use crossbeam_channel::Receiver;
use std::fs;
use std::io::Write;
use std::path::PathBuf;

/// Options for the capture background thread.
pub struct CaptureOpts {
    pub data_path: PathBuf,
    pub summary_only: bool,
    pub max_lines: Option<u64>,
    pub max_bytes: Option<u64>,
}

/// Result returned by the capture thread.
pub struct CaptureResult {
    pub bytes_written: u64,
    pub lines_written: u64,
    pub truncated: bool,
    pub sample: Vec<u8>,
}

const SAMPLE_SIZE: usize = 8 * 1024;

/// Spawn a background writer thread that drains chunks from the channel.
pub fn capture_thread(rx: Receiver<Vec<u8>>, opts: CaptureOpts) -> std::thread::JoinHandle<CaptureResult> {
    std::thread::spawn(move || {
        let mut bytes_written: u64 = 0;
        let mut lines_written: u64 = 0;
        let mut truncated = false;
        let mut sample = Vec::with_capacity(SAMPLE_SIZE);
        let mut capturing = true;

        let mut file = if !opts.summary_only {
            if let Some(parent) = opts.data_path.parent() {
                let _ = fs::create_dir_all(parent);
            }
            fs::File::create(&opts.data_path).ok()
        } else {
            None
        };

        for chunk in &rx {
            // Collect sample for format detection
            if sample.len() < SAMPLE_SIZE {
                let remaining = SAMPLE_SIZE - sample.len();
                let take = remaining.min(chunk.len());
                sample.extend_from_slice(&chunk[..take]);
            }

            if !capturing {
                continue;
            }

            let line_count = chunk.iter().filter(|&&b| b == b'\n').count() as u64;

            // Check limits
            if let Some(max_b) = opts.max_bytes {
                if bytes_written + chunk.len() as u64 > max_b {
                    truncated = true;
                    capturing = false;
                    continue;
                }
            }
            if let Some(max_l) = opts.max_lines {
                if lines_written + line_count > max_l {
                    truncated = true;
                    capturing = false;
                    continue;
                }
            }

            bytes_written += chunk.len() as u64;
            lines_written += line_count;

            if let Some(ref mut f) = file {
                if f.write_all(&chunk).is_err() {
                    truncated = true;
                    capturing = false;
                }
            }
        }

        CaptureResult {
            bytes_written,
            lines_written,
            truncated,
            sample,
        }
    })
}
