use sha2::{Digest, Sha256};
use std::io::{self, Read};

/// Compute SHA-256 of a byte slice, returning hex string.
pub fn sha256_bytes(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    format!("{:x}", hasher.finalize())
}

/// Compute SHA-256 of a reader, streaming in chunks.
/// Returns (hex_hash, bytes_read).
pub fn sha256_reader<R: Read>(mut reader: R) -> io::Result<(String, u64)> {
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 64 * 1024];
    let mut total = 0u64;
    loop {
        let n = reader.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
        total += n as u64;
    }
    Ok((format!("{:x}", hasher.finalize()), total))
}

/// Compute SHA-256 of a file by path.
pub fn sha256_file(path: &std::path::Path) -> io::Result<String> {
    let f = std::fs::File::open(path)?;
    let (hash, _) = sha256_reader(f)?;
    Ok(hash)
}

/// A reader wrapper that hashes data as it passes through.
pub struct HashReader<R> {
    inner: R,
    hasher: Sha256,
    bytes_read: u64,
}

impl<R: Read> HashReader<R> {
    pub fn new(inner: R) -> Self {
        Self {
            inner,
            hasher: Sha256::new(),
            bytes_read: 0,
        }
    }

    pub fn finish(self) -> (String, u64) {
        (format!("{:x}", self.hasher.finalize()), self.bytes_read)
    }
}

impl<R: Read> Read for HashReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let n = self.inner.read(buf)?;
        if n > 0 {
            self.hasher.update(&buf[..n]);
            self.bytes_read += n as u64;
        }
        Ok(n)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sha256_bytes() {
        let hash = sha256_bytes(b"hello");
        assert_eq!(
            hash,
            "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824"
        );
    }

    #[test]
    fn test_sha256_reader() {
        let data = b"hello world";
        let (hash, len) = sha256_reader(&data[..]).unwrap();
        assert_eq!(len, 11);
        assert!(!hash.is_empty());
    }

    #[test]
    fn test_hash_reader() {
        let data = b"test data";
        let mut hr = HashReader::new(&data[..]);
        let mut out = Vec::new();
        hr.read_to_end(&mut out).unwrap();
        assert_eq!(out, data);
        let (hash, len) = hr.finish();
        assert_eq!(len, 9);
        assert_eq!(hash, sha256_bytes(data));
    }
}
