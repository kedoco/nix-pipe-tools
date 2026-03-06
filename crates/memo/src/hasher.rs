use sha2::{Digest, Sha256};
use shared::fileident::FileIdent;
use shared::hash::sha256_file;
use std::path::{Path, PathBuf};

/// Resolved command info used for cache key computation.
pub struct ResolvedCommand {
    pub path: PathBuf,
    pub ident: FileIdent,
}

impl ResolvedCommand {
    /// Resolve a command name to its full path and identity.
    pub fn resolve(command: &str) -> std::io::Result<Self> {
        let path = which::which(command).map_err(|e| {
            std::io::Error::new(std::io::ErrorKind::NotFound, format!("{}: {}", command, e))
        })?;
        let ident = FileIdent::from_path(&path)?;
        Ok(Self { path, ident })
    }
}

/// Inputs that determine the cache key.
pub struct CacheKeyInputs<'a> {
    pub resolved: &'a ResolvedCommand,
    pub args: &'a [String],
    pub stdin_hash: Option<&'a str>,
    pub env_vars: &'a [(String, String)],
    pub watched_files: &'a [PathBuf],
    pub tag: Option<&'a str>,
}

/// Compute the cache key from the given inputs.
pub fn compute_key(inputs: &CacheKeyInputs) -> std::io::Result<String> {
    let mut hasher = Sha256::new();

    // Command path
    hasher.update(inputs.resolved.path.to_string_lossy().as_bytes());
    // Binary mtime + size
    hasher.update(inputs.resolved.ident.modified_secs.to_le_bytes());
    hasher.update(inputs.resolved.ident.size.to_le_bytes());

    // Args
    for arg in inputs.args {
        hasher.update(arg.as_bytes());
        hasher.update(b"\0");
    }

    // Stdin hash
    if let Some(h) = inputs.stdin_hash {
        hasher.update(b"stdin:");
        hasher.update(h.as_bytes());
    }

    // Env vars (sorted for determinism)
    let mut env_sorted: Vec<_> = inputs.env_vars.to_vec();
    env_sorted.sort();
    for (k, v) in &env_sorted {
        hasher.update(b"env:");
        hasher.update(k.as_bytes());
        hasher.update(b"=");
        hasher.update(v.as_bytes());
    }

    // Watched files
    for path in inputs.watched_files {
        let hash = sha256_file(path)?;
        hasher.update(b"watch:");
        hasher.update(path.to_string_lossy().as_bytes());
        hasher.update(b"=");
        hasher.update(hash.as_bytes());
    }

    // Tag
    if let Some(t) = inputs.tag {
        hasher.update(b"tag:");
        hasher.update(t.as_bytes());
    }

    Ok(format!("{:x}", hasher.finalize()))
}

/// Compute the cache key for a command (convenience for bust/show-key).
pub fn compute_key_for_command(
    command: &str,
    args: &[String],
    env_keys: &[String],
    watched_files: &[PathBuf],
    tag: Option<&str>,
) -> std::io::Result<String> {
    let resolved = ResolvedCommand::resolve(command)?;
    let env_vars: Vec<(String, String)> = env_keys
        .iter()
        .filter_map(|k| std::env::var(k).ok().map(|v| (k.clone(), v)))
        .collect();
    let inputs = CacheKeyInputs {
        resolved: &resolved,
        args,
        stdin_hash: None,
        env_vars: &env_vars,
        watched_files,
        tag,
    };
    compute_key(&inputs)
}

/// Compute stdin hash by reading from a reader into a temp file.
/// Returns (hash, path_to_temp_file).
pub fn hash_stdin_to_file(
    cache_root: &Path,
) -> std::io::Result<(String, tempfile::NamedTempFile)> {
    use shared::hash::HashReader;
    use std::io::{self, Read, Write};

    let tmp = tempfile::NamedTempFile::new_in(cache_root)?;
    let mut writer = std::io::BufWriter::new(tmp);
    let mut reader = HashReader::new(io::stdin().lock());
    let mut buf = [0u8; 64 * 1024];

    loop {
        let n = reader.read(&mut buf)?;
        if n == 0 {
            break;
        }
        writer.write_all(&buf[..n])?;
    }

    let (hash, _bytes) = reader.finish();
    let tmp = writer.into_inner().map_err(|e| e.into_error())?;
    Ok((hash, tmp))
}
