use crate::cache::Cache;
use std::io;

/// Run garbage collection, removing oldest entries until total size is under max_bytes.
pub fn run_gc(cache: &Cache, max_bytes: u64) -> io::Result<GcResult> {
    let mut entries = cache.list_entries()?;
    let total_before: u64 = entries.iter().map(|e| e.size).sum();

    if total_before <= max_bytes {
        return Ok(GcResult {
            removed: 0,
            freed: 0,
            total_before,
            total_after: total_before,
        });
    }

    // Sort by access time ascending (oldest first)
    entries.sort_by_key(|e| e.accessed);

    let mut current_size = total_before;
    let mut removed = 0u64;
    let mut freed = 0u64;

    for entry in &entries {
        if current_size <= max_bytes {
            break;
        }
        if std::fs::remove_dir_all(&entry.path).is_ok() {
            current_size -= entry.size;
            freed += entry.size;
            removed += 1;
        }
    }

    Ok(GcResult {
        removed,
        freed,
        total_before,
        total_after: current_size,
    })
}

pub struct GcResult {
    pub removed: u64,
    pub freed: u64,
    pub total_before: u64,
    pub total_after: u64,
}
