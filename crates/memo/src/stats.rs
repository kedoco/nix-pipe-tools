use serde::{Deserialize, Serialize};
use std::fs;
use std::io;
use std::path::PathBuf;

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Stats {
    pub hits: u64,
    pub misses: u64,
}

impl Stats {
    fn path() -> io::Result<PathBuf> {
        let home = std::env::var("HOME")
            .map_err(|_| io::Error::new(io::ErrorKind::NotFound, "HOME not set"))?;
        Ok(PathBuf::from(home)
            .join(".cache")
            .join("memo")
            .join("stats.json"))
    }

    pub fn load() -> io::Result<Self> {
        let path = Self::path()?;
        match fs::read_to_string(&path) {
            Ok(data) => serde_json::from_str(&data)
                .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e)),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => Err(e),
        }
    }

    pub fn save(&self) -> io::Result<()> {
        let path = Self::path()?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&path, serde_json::to_string_pretty(self)?)?;
        Ok(())
    }

    pub fn record_hit() -> io::Result<()> {
        let mut s = Self::load()?;
        s.hits += 1;
        s.save()
    }

    pub fn record_miss() -> io::Result<()> {
        let mut s = Self::load()?;
        s.misses += 1;
        s.save()
    }
}
