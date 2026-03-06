use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Serialize, Deserialize)]
pub struct Config {
    #[serde(default = "default_ignore_patterns")]
    pub ignore_patterns: Vec<String>,
}

fn default_ignore_patterns() -> Vec<String> {
    vec![
        "/tmp/*".into(),
        "/proc/*".into(),
        "/dev/*".into(),
        "/sys/*".into(),
        ".git/objects/*".into(),
        "node_modules/*".into(),
    ]
}

impl Default for Config {
    fn default() -> Self {
        Self {
            ignore_patterns: default_ignore_patterns(),
        }
    }
}

impl Config {
    pub fn config_path() -> PathBuf {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
        PathBuf::from(home).join(".config/prv/config.toml")
    }

    pub fn load() -> Self {
        let path = Self::config_path();
        if path.exists() {
            match fs::read_to_string(&path) {
                Ok(contents) => toml::from_str(&contents).unwrap_or_default(),
                Err(_) => Self::default(),
            }
        } else {
            let config = Self::default();
            config.save_default();
            config
        }
    }

    fn save_default(&self) {
        let path = Self::config_path();
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        if let Ok(contents) = toml::to_string_pretty(self) {
            let _ = fs::write(&path, contents);
        }
    }

    pub fn should_ignore(&self, path: &str) -> bool {
        for pattern in &self.ignore_patterns {
            if let Ok(pat) = glob::Pattern::new(pattern) {
                if pat.matches(path) {
                    return true;
                }
            }
        }
        false
    }
}
