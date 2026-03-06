use crate::session::{self, Meta};
use shared::human;
use std::fs;
use std::io::{self, IsTerminal, Write};
use std::path::Path;
use std::process::Command;

/// Find the most recent session ID by directory modification time.
fn latest_session() -> Option<String> {
    let dir = session::sessions_dir();
    let mut entries: Vec<_> = fs::read_dir(&dir)
        .ok()?
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().map(|t| t.is_dir()).unwrap_or(false))
        .collect();
    // Sort by modification time (most recent first), not by name
    entries.sort_by(|a, b| {
        let mtime_a = a.metadata().and_then(|m| m.modified()).unwrap_or(std::time::SystemTime::UNIX_EPOCH);
        let mtime_b = b.metadata().and_then(|m| m.modified()).unwrap_or(std::time::SystemTime::UNIX_EPOCH);
        mtime_b.cmp(&mtime_a)
    });
    entries.first().map(|e| e.file_name().to_string_lossy().to_string())
}

/// Find the session containing the most recent capture with the given name.
fn resolve_session_for_name(session: &Option<String>, name: &str) -> Result<String, String> {
    if let Some(s) = session {
        return Ok(s.clone());
    }
    // Search all sessions for the most recent capture with this name
    let dir = session::sessions_dir();
    let Ok(entries) = fs::read_dir(&dir) else {
        return Err("no sessions found".to_string());
    };
    let mut best: Option<(String, String)> = None; // (session_id, timestamp)
    for entry in entries.flatten() {
        if !entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
            continue;
        }
        let sid = entry.file_name().to_string_lossy().to_string();
        let meta_path = session::meta_path(&sid, name);
        if let Ok(data) = fs::read_to_string(&meta_path) {
            if let Ok(meta) = serde_json::from_str::<Meta>(&data) {
                match &best {
                    Some((_, ts)) if *ts >= meta.timestamp => {}
                    _ => best = Some((sid, meta.timestamp)),
                }
            }
        }
    }
    best.map(|(sid, _)| sid)
        .ok_or_else(|| format!("capture '{}' not found in any session", name))
}

/// Read all meta files in a session directory.
fn read_metas(session_id: &str) -> Vec<Meta> {
    let dir = session::session_dir(session_id);
    let Ok(entries) = fs::read_dir(&dir) else {
        return Vec::new();
    };
    entries
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.path()
                .extension()
                .map(|ext| ext == "json")
                .unwrap_or(false)
        })
        .filter_map(|e| {
            let data = fs::read_to_string(e.path()).ok()?;
            serde_json::from_str::<Meta>(&data).ok()
        })
        .collect()
}

/// Read all metas across all sessions, keeping only the most recent per name.
fn all_metas_latest_per_name() -> Vec<Meta> {
    let dir = session::sessions_dir();
    let Ok(entries) = fs::read_dir(&dir) else {
        return Vec::new();
    };
    let mut by_name: std::collections::HashMap<String, Meta> = std::collections::HashMap::new();
    for entry in entries.flatten() {
        if !entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
            continue;
        }
        let sid = entry.file_name().to_string_lossy().to_string();
        for meta in read_metas(&sid) {
            match by_name.get(&meta.name) {
                Some(existing) if existing.timestamp >= meta.timestamp => {}
                _ => {
                    by_name.insert(meta.name.clone(), meta);
                }
            }
        }
    }
    let mut result: Vec<Meta> = by_name.into_values().collect();
    result.sort_by(|a, b| a.name.cmp(&b.name));
    result
}

/// `tap show <name> [-S session]`
pub fn show(name: &str, session: &Option<String>) -> Result<(), String> {
    let sid = resolve_session_for_name(session, name)?;
    let path = session::data_path(&sid, name);
    if !path.exists() {
        return Err(format!("capture '{}' not found in session {}", name, sid));
    }

    let is_tty = std::io::stdout().is_terminal();
    if is_tty {
        if let Ok(pager) = std::env::var("PAGER") {
            let _ = Command::new(pager).arg(&path).status();
            return Ok(());
        }
        let _ = Command::new("less").arg(&path).status();
        return Ok(());
    }

    let data = fs::read(&path).map_err(|e| e.to_string())?;
    io::stdout().write_all(&data).map_err(|e| e.to_string())?;
    Ok(())
}

/// `tap diff <name1> <name2> [-S session]`
pub fn diff(name1: &str, name2: &str, session: &Option<String>) -> Result<(), String> {
    let sid1 = resolve_session_for_name(session, name1)?;
    let sid2 = resolve_session_for_name(session, name2)?;
    let path1 = session::data_path(&sid1, name1);
    let path2 = session::data_path(&sid2, name2);
    if !path1.exists() {
        return Err(format!("capture '{}' not found in session {}", name1, sid1));
    }
    if !path2.exists() {
        return Err(format!("capture '{}' not found in session {}", name2, sid2));
    }

    let status = Command::new("diff")
        .arg(&path1)
        .arg(&path2)
        .status()
        .map_err(|e| format!("failed to run diff: {}", e))?;

    // diff returns 1 if files differ, that's fine
    if !status.success() && status.code() != Some(1) {
        return Err(format!("diff exited with status {:?}", status.code()));
    }
    Ok(())
}

/// `tap stats [-S session]`
///
/// Without `-S`: shows the most recent capture for each name across all sessions.
/// With `-S`: scoped to that session only.
pub fn stats(session: &Option<String>) -> Result<(), String> {
    let metas = if let Some(sid) = session {
        read_metas(sid)
    } else {
        all_metas_latest_per_name()
    };

    if metas.is_empty() {
        eprintln!("no captures found");
        return Ok(());
    }

    let mut table = comfy_table::Table::new();
    table.set_header(["Name", "Session", "Lines", "Bytes", "Duration", "Format", "Truncated"]);
    for m in &metas {
        table.add_row([
            m.name.clone(),
            m.session_id.clone(),
            m.lines.to_string(),
            human::format_bytes(m.bytes),
            human::format_duration(m.duration_secs),
            m.format.to_string(),
            if m.truncated { "yes" } else { "no" }.to_string(),
        ]);
    }
    println!("{table}");
    Ok(())
}

/// `tap replay <name> [-S session]`
pub fn replay(name: &str, session: &Option<String>) -> Result<(), String> {
    let sid = resolve_session_for_name(session, name)?;
    let path = session::data_path(&sid, name);
    if !path.exists() {
        return Err(format!("capture '{}' not found in session {}", name, sid));
    }
    let data = fs::read(&path).map_err(|e| e.to_string())?;
    io::stdout().write_all(&data).map_err(|e| e.to_string())?;
    Ok(())
}

/// `tap last`
pub fn last() -> Result<(), String> {
    let sid = latest_session().ok_or("no sessions found")?;
    let metas = read_metas(&sid);
    if metas.is_empty() {
        eprintln!("no captures in session {}", sid);
        return Ok(());
    }
    println!("Session: {}", sid);
    for m in &metas {
        println!(
            "  {} ({}, {} lines, {})",
            m.name,
            human::format_bytes(m.bytes),
            m.lines,
            m.format,
        );
    }
    Ok(())
}

/// `tap sessions`
pub fn sessions() -> Result<(), String> {
    let dir = session::sessions_dir();
    if !dir.exists() {
        println!("no sessions found");
        return Ok(());
    }
    let mut entries: Vec<_> = fs::read_dir(&dir)
        .map_err(|e| e.to_string())?
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().map(|t| t.is_dir()).unwrap_or(false))
        .collect();
    entries.sort_by(|a, b| {
        let mtime_a = a.metadata().and_then(|m| m.modified()).unwrap_or(std::time::SystemTime::UNIX_EPOCH);
        let mtime_b = b.metadata().and_then(|m| m.modified()).unwrap_or(std::time::SystemTime::UNIX_EPOCH);
        mtime_b.cmp(&mtime_a)
    });

    for entry in &entries {
        let sid = entry.file_name().to_string_lossy().to_string();
        let metas = read_metas(&sid);
        let capture_count = metas.len();
        let timestamp = metas.first().map(|m| m.timestamp.as_str()).unwrap_or("?");
        println!("{} ({} captures, {})", sid, capture_count, timestamp);
    }
    Ok(())
}

/// `tap clean --older-than <duration>`
pub fn clean(older_than: &str) -> Result<(), String> {
    let duration = human::parse_duration(older_than)?;
    let cutoff = std::time::SystemTime::now() - duration;

    let dir = session::sessions_dir();
    if !dir.exists() {
        return Ok(());
    }

    let entries: Vec<_> = fs::read_dir(&dir)
        .map_err(|e| e.to_string())?
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().map(|t| t.is_dir()).unwrap_or(false))
        .collect();

    let mut removed = 0usize;
    for entry in &entries {
        let modified = entry
            .metadata()
            .and_then(|m| m.modified())
            .unwrap_or(std::time::SystemTime::UNIX_EPOCH);
        if modified < cutoff {
            let path = entry.path();
            if remove_dir_contents(&path).is_ok() {
                removed += 1;
            }
        }
    }
    println!("removed {} session(s)", removed);
    Ok(())
}

fn remove_dir_contents(path: &Path) -> io::Result<()> {
    fs::remove_dir_all(path)
}
