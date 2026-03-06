use std::path::PathBuf;
use std::time::SystemTime;

/// Derive a session ID from PPID and current epoch seconds.
///
/// If there's already a session directory for this PPID created within the
/// last 60 seconds, reuse it. This handles the case where multiple `tap`
/// invocations in the same pipeline (same PPID) start at slightly different
/// times, and also when a user re-runs a pipeline quickly.
pub fn session_id() -> String {
    let ppid = nix::unistd::getppid();
    let ppid_prefix = format!("{}-", ppid);

    // Look for an existing session from the same PPID within 60s
    if let Ok(entries) = std::fs::read_dir(sessions_dir()) {
        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let mut best: Option<(String, u64)> = None;
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if let Some(epoch_str) = name.strip_prefix(&ppid_prefix) {
                if let Ok(epoch) = epoch_str.parse::<u64>() {
                    if now.saturating_sub(epoch) < 60 {
                        match &best {
                            Some((_, best_epoch)) if epoch > *best_epoch => {
                                best = Some((name, epoch));
                            }
                            None => {
                                best = Some((name, epoch));
                            }
                            _ => {}
                        }
                    }
                }
            }
        }
        if let Some((existing_id, _)) = best {
            return existing_id;
        }
    }

    let epoch = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    format!("{}-{}", ppid, epoch)
}

/// Base directory for all tap storage.
pub fn base_dir() -> PathBuf {
    let user = std::env::var("USER").unwrap_or_else(|_| "unknown".to_string());
    PathBuf::from(format!("/tmp/tap-{}", user))
}

/// Sessions root directory.
pub fn sessions_dir() -> PathBuf {
    base_dir().join("sessions")
}

/// Directory for a specific session.
pub fn session_dir(session_id: &str) -> PathBuf {
    sessions_dir().join(session_id)
}

/// Path to the data file for a capture point.
pub fn data_path(session_id: &str, name: &str) -> PathBuf {
    session_dir(session_id).join(format!("{}.data", name))
}

/// Path to the meta file for a capture point.
pub fn meta_path(session_id: &str, name: &str) -> PathBuf {
    session_dir(session_id).join(format!("{}.meta.json", name))
}

/// Metadata written alongside each capture.
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct Meta {
    pub name: String,
    pub session_id: String,
    pub timestamp: String,
    pub bytes: u64,
    pub lines: u64,
    pub duration_secs: f64,
    pub format: crate::detect::Format,
    pub truncated: bool,
}
