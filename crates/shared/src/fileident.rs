use serde::{Deserialize, Serialize};
use std::fs;
use std::io;
use std::path::Path;
use std::time::SystemTime;

/// Cross-platform file identity based on metadata.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct FileIdent {
    pub size: u64,
    pub modified_secs: i64,
    #[cfg(unix)]
    pub inode: u64,
    #[cfg(unix)]
    pub dev: u64,
}

impl FileIdent {
    /// Get file identity from path.
    pub fn from_path(path: &Path) -> io::Result<Self> {
        let meta = fs::metadata(path)?;
        let modified_secs = meta
            .modified()?
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;

        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            Ok(Self {
                size: meta.len(),
                modified_secs,
                inode: meta.ino(),
                dev: meta.dev(),
            })
        }

        #[cfg(not(unix))]
        {
            Ok(Self {
                size: meta.len(),
                modified_secs,
            })
        }
    }

    /// Quick check if a file has changed since we last saw it.
    pub fn has_changed(&self, path: &Path) -> io::Result<bool> {
        let current = Self::from_path(path)?;
        Ok(current != *self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_file_ident() {
        let mut tmp = NamedTempFile::new().unwrap();
        write!(tmp, "hello").unwrap();
        let ident = FileIdent::from_path(tmp.path()).unwrap();
        assert_eq!(ident.size, 5);
        assert!(!ident.has_changed(tmp.path()).unwrap());
    }
}
