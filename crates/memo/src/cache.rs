use serde::{Deserialize, Serialize};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// Metadata stored alongside cached output.
#[derive(Debug, Serialize, Deserialize)]
pub struct CacheMeta {
    pub exit_code: i32,
    pub duration_ms: u64,
    pub created_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ttl_secs: Option<u64>,
    pub command: String,
    pub args: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stdin_hash: Option<String>,
    pub watched_files: Vec<String>,
}

/// Root of the memo cache.
pub struct Cache {
    root: PathBuf,
}

impl Cache {
    pub fn new() -> io::Result<Self> {
        let root = cache_root()?;
        fs::create_dir_all(&root)?;
        Ok(Self { root })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Path to the blob directory for a given key.
    pub fn blob_dir(&self, key: &str) -> PathBuf {
        self.root.join("blobs").join(key)
    }

    /// Check if a cached entry exists and is not expired.
    pub fn lookup(&self, key: &str) -> Option<CacheMeta> {
        let dir = self.blob_dir(key);
        let meta_path = dir.join("meta.json");
        let data = fs::read_to_string(&meta_path).ok()?;
        let meta: CacheMeta = serde_json::from_str(&data).ok()?;

        // Check TTL
        if let Some(ttl) = meta.ttl_secs {
            if let Ok(created) = parse_rfc3339_secs(&meta.created_at) {
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs();
                if now > created + ttl {
                    return None;
                }
            }
        }

        // Touch the meta.json to update access time for LRU tracking.
        // We do this by opening the file which updates atime on most systems.
        let _ = fs::File::open(&meta_path);

        Some(meta)
    }

    /// Atomically store a cache entry.
    pub fn store(
        &self,
        key: &str,
        meta: &CacheMeta,
        stdout_data: &[u8],
        stderr_data: &[u8],
        interleave_log: &[u8],
    ) -> io::Result<()> {
        let blobs_dir = self.root.join("blobs");
        fs::create_dir_all(&blobs_dir)?;

        let tmp_dir = tempfile::tempdir_in(&blobs_dir)?;

        fs::write(tmp_dir.path().join("stdout"), stdout_data)?;
        fs::write(tmp_dir.path().join("stderr"), stderr_data)?;
        fs::write(tmp_dir.path().join("interleave.log"), interleave_log)?;
        fs::write(
            tmp_dir.path().join("meta.json"),
            serde_json::to_string_pretty(meta)?,
        )?;

        let target = self.blob_dir(key);
        // Remove existing if present
        let _ = fs::remove_dir_all(&target);
        // Atomic rename
        let tmp_path = tmp_dir.keep();
        fs::rename(&tmp_path, &target)?;

        Ok(())
    }

    /// Remove a specific cache entry.
    pub fn remove(&self, key: &str) -> io::Result<bool> {
        let dir = self.blob_dir(key);
        if dir.exists() {
            fs::remove_dir_all(&dir)?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Remove the entire cache.
    pub fn purge(&self) -> io::Result<()> {
        let blobs = self.root.join("blobs");
        if blobs.exists() {
            fs::remove_dir_all(&blobs)?;
        }
        let stats_path = self.root.join("stats.json");
        let _ = fs::remove_file(&stats_path);
        Ok(())
    }

    /// Read stdout from cache.
    pub fn read_stdout(&self, key: &str) -> io::Result<Vec<u8>> {
        fs::read(self.blob_dir(key).join("stdout"))
    }

    /// Read stderr from cache.
    pub fn read_stderr(&self, key: &str) -> io::Result<Vec<u8>> {
        fs::read(self.blob_dir(key).join("stderr"))
    }

    /// Read interleave log from cache.
    pub fn read_interleave(&self, key: &str) -> io::Result<Vec<u8>> {
        fs::read(self.blob_dir(key).join("interleave.log"))
    }

    /// Acquire a file lock for the given key (for deduplication).
    pub fn lock_key(&self, key: &str) -> io::Result<fs::File> {
        use fs2::FileExt;
        let lock_dir = self.root.join("locks");
        fs::create_dir_all(&lock_dir)?;
        let lock_path = lock_dir.join(key);
        let file = fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .open(&lock_path)?;
        file.lock_exclusive()?;
        Ok(file)
    }

    /// List all cached entries with their metadata and total size.
    pub fn list_entries(&self) -> io::Result<Vec<CacheEntry>> {
        let blobs = self.root.join("blobs");
        if !blobs.exists() {
            return Ok(Vec::new());
        }
        let mut entries = Vec::new();
        for entry in fs::read_dir(&blobs)? {
            let entry = entry?;
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            let key = entry.file_name().to_string_lossy().to_string();
            let meta: Option<CacheMeta> = fs::read_to_string(path.join("meta.json"))
                .ok()
                .and_then(|s| serde_json::from_str(&s).ok());
            let size = dir_size(&path);
            let accessed = dir_access_time(&path);
            entries.push(CacheEntry {
                key,
                path,
                meta,
                size,
                accessed,
            });
        }
        Ok(entries)
    }
}

pub struct CacheEntry {
    pub key: String,
    pub path: PathBuf,
    pub meta: Option<CacheMeta>,
    pub size: u64,
    pub accessed: u64,
}

fn cache_root() -> io::Result<PathBuf> {
    let home = std::env::var("HOME")
        .map_err(|_| io::Error::new(io::ErrorKind::NotFound, "HOME not set"))?;
    Ok(PathBuf::from(home).join(".cache").join("memo"))
}

fn dir_size(path: &Path) -> u64 {
    let mut total = 0;
    if let Ok(entries) = fs::read_dir(path) {
        for entry in entries.flatten() {
            if let Ok(meta) = entry.metadata() {
                total += meta.len();
            }
        }
    }
    total
}

fn dir_access_time(path: &Path) -> u64 {
    fs::metadata(path)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn parse_rfc3339_secs(s: &str) -> Result<u64, ()> {
    humantime::parse_rfc3339(s)
        .map_err(|_| ())
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).map_err(|_| ()))
        .map(|d| d.as_secs())
}
