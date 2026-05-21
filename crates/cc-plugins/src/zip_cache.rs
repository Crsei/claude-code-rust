//! ZIP file caching and extraction utilities.
//!
//! Provides an LRU cache of downloaded plugin ZIP files and safe extraction
//! with path traversal protection.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use lru::LruCache;
use parking_lot::Mutex;
use sha2::{Digest, Sha256};
use std::num::NonZeroUsize;
use std::sync::LazyLock;

/// Default cache capacity (number of ZIP files).
const DEFAULT_CACHE_CAPACITY: usize = 100;

/// Thread-safe LRU cache for downloaded plugin ZIP data.
pub struct ZipCache {
    inner: Mutex<LruCache<String, Vec<u8>>>,
}

impl ZipCache {
    /// Create a new ZIP cache with the given capacity.
    pub fn new(capacity: NonZeroUsize) -> Self {
        Self {
            inner: Mutex::new(LruCache::new(capacity)),
        }
    }

    /// Get cached entry by key.
    pub fn get(&self, key: &str) -> Option<Vec<u8>> {
        self.inner.lock().get(key).cloned()
    }

    /// Insert entry into cache.
    pub fn put(&self, key: String, data: Vec<u8>) {
        self.inner.lock().put(key, data);
    }

    /// Check if key exists in cache.
    pub fn contains(&self, key: &str) -> bool {
        self.inner.lock().contains(key)
    }

    /// Clear all cached entries.
    pub fn clear(&self) {
        self.inner.lock().clear();
    }

    /// Current number of cached entries.
    pub fn len(&self) -> usize {
        self.inner.lock().len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl Default for ZipCache {
    fn default() -> Self {
        Self::new(NonZeroUsize::new(DEFAULT_CACHE_CAPACITY).unwrap())
    }
}

/// Global ZIP cache instance.
pub static GLOBAL_ZIP_CACHE: LazyLock<ZipCache> = LazyLock::new(ZipCache::default);

/// Compute a cache key from a source URL and optional checksum.
pub fn cache_key(url: &str, checksum: Option<&str>) -> String {
    match checksum {
        Some(cs) => format!("{}#{}", url, cs),
        None => {
            let mut hasher = Sha256::new();
            hasher.update(url.as_bytes());
            hex::encode(hasher.finalize())
        }
    }
}

/// Safe extraction of a ZIP archive to a destination directory.
///
/// This function protects against path traversal attacks by rejecting entries
/// that would escape the destination directory.
pub fn extract_zip_to(zip_data: &[u8], dest_dir: &Path) -> Result<()> {
    let cursor = std::io::Cursor::new(zip_data);
    let mut archive = zip::ZipArchive::new(cursor).context("Failed to open ZIP archive")?;

    std::fs::create_dir_all(dest_dir).with_context(|| {
        format!(
            "Failed to create destination directory: {}",
            dest_dir.display()
        )
    })?;

    for i in 0..archive.len() {
        let mut entry = archive
            .by_index(i)
            .with_context(|| format!("Failed to read ZIP entry at index {}", i))?;

        let entry_name = match entry.enclosed_name() {
            Some(name) => name.to_string_lossy().to_string(),
            None => {
                anyhow::bail!(
                    "Path traversal detected: entry at index {} would escape destination '{}'",
                    i,
                    dest_dir.display()
                );
            }
        };

        let entry_path = dest_dir.join(&entry_name);

        if entry.is_dir() {
            std::fs::create_dir_all(&entry_path)
                .with_context(|| format!("Failed to create directory: {}", entry_path.display()))?;
        } else {
            if let Some(parent) = entry_path.parent() {
                std::fs::create_dir_all(parent).with_context(|| {
                    format!("Failed to create parent directory: {}", parent.display())
                })?;
            }

            let mut output_file = std::fs::File::create(&entry_path)
                .with_context(|| format!("Failed to create file: {}", entry_path.display()))?;

            std::io::copy(&mut entry, &mut output_file)
                .with_context(|| format!("Failed to extract entry: {}", entry_name))?;
        }
    }

    Ok(())
}

/// Compute SHA-256 hash of a file.
pub fn hash_zip(path: &Path) -> Result<String> {
    let bytes = std::fs::read(path)
        .with_context(|| format!("Failed to read file for hashing: {}", path.display()))?;

    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    Ok(hex::encode(hasher.finalize()))
}

/// Get the default plugin cache directory path.
pub fn cache_dir() -> PathBuf {
    crate::cache_dir().join("downloads")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn test_zip_cache_basic() {
        let cache = ZipCache::new(NonZeroUsize::new(10).unwrap());
        assert!(cache.is_empty());

        cache.put("key1".to_string(), vec![1, 2, 3]);
        assert!(cache.contains("key1"));
        assert_eq!(cache.get("key1"), Some(vec![1, 2, 3]));
        assert!(!cache.contains("key2"));

        cache.clear();
        assert!(cache.is_empty());
    }

    #[test]
    fn test_cache_key() {
        let key1 = cache_key("https://example.com/plugin.zip", Some("abc123"));
        let key2 = cache_key("https://example.com/plugin.zip", Some("abc123"));
        let key3 = cache_key("https://example.com/plugin.zip", Some("def456"));
        let key4 = cache_key("https://different.com/pkg.zip", None);

        assert_eq!(key1, key2);
        assert_ne!(key1, key3);
        assert!(key4.len() == 64 || key4.len() > 32); // SHA-256 hex
    }

    #[test]
    fn test_extract_zip_to_rejects_path_traversal() {
        // Create a malicious ZIP that attempts path traversal
        // We need to construct it programmatically

        let mut zip_buf = Vec::new();
        {
            let mut zip_writer = zip::ZipWriter::new(std::io::Cursor::new(&mut zip_buf));
            let options = zip::write::FileOptions::<()>::default();
            zip_writer
                .start_file("../evil.sh", options)
                .expect("start file");
            zip_writer.write(b"malicious content").expect("write");
            zip_writer.finish().expect("finish");
        }

        let dest = tempfile::tempdir().unwrap();
        let result = extract_zip_to(&zip_buf, dest.path());
        assert!(result.is_err());
        assert!(
            result.unwrap_err().to_string().contains("Path traversal"),
            "expected path traversal error"
        );
    }

    #[test]
    fn test_extract_zip_to_empty_archive() {
        let mut zip_buf = Vec::new();
        {
            let mut zip_writer = zip::ZipWriter::new(std::io::Cursor::new(&mut zip_buf));
            zip_writer.finish().expect("finish");
        }

        let dest = tempfile::tempdir().unwrap();
        let result = extract_zip_to(&zip_buf, dest.path());
        assert!(result.is_ok());
    }

    #[test]
    fn test_extract_zip_to_invalid_zip() {
        let invalid_data = b"this is not a zip file";
        let dest = tempfile::tempdir().unwrap();
        let result = extract_zip_to(invalid_data, dest.path());
        assert!(result.is_err());
    }

    #[test]
    fn test_hash_zip_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.zip");
        std::fs::write(&path, b"content").unwrap();

        let hash = hash_zip(&path).unwrap();
        assert_eq!(hash.len(), 64);

        let hash2 = hash_zip(&path).unwrap();
        assert_eq!(hash, hash2);
    }
}
