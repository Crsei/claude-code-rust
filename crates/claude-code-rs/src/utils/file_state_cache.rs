#![allow(unused)]

use lru::LruCache;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::num::NonZeroUsize;

/// An entry in the file state cache, recording the last known state
/// of a file as observed by tools (reads and writes).
#[derive(Debug, Clone)]
pub struct FileCacheEntry {
    /// Hash of the file content at the time it was cached.
    pub content_hash: u64,
    /// File size in bytes.
    pub size_bytes: u64,
    /// Last modified timestamp (Unix millis).
    pub last_modified: i64,
}

/// LRU file state cache that tracks files read/written by tools.
///
/// This cache allows the system to detect whether a file has changed
/// since it was last read, enabling smarter tool behavior (e.g.,
/// detecting external modifications, avoiding redundant reads).
pub struct FileStateCache {
    cache: LruCache<String, FileCacheEntry>,
}

impl FileStateCache {
    /// Create a new FileStateCache with the given capacity.
    ///
    /// # Panics
    /// Panics if `capacity` is 0.
    pub fn new(capacity: usize) -> Self {
        let cap = NonZeroUsize::new(capacity).expect("FileStateCache capacity must be > 0");
        Self {
            cache: LruCache::new(cap),
        }
    }

    /// Get a cached entry by file path, updating its recency.
    /// Returns `None` if the path is not in the cache.
    pub fn get(&mut self, path: &str) -> Option<&FileCacheEntry> {
        self.cache.get(path)
    }

    /// Insert or update a cache entry for the given file path.
    pub fn insert(&mut self, path: String, entry: FileCacheEntry) {
        self.cache.put(path, entry);
    }

    /// Check if a path exists in the cache without updating recency.
    pub fn has(&self, path: &str) -> bool {
        self.cache.contains(path)
    }

    /// Remove a path from the cache (e.g., after a file is deleted or
    /// known to have been modified externally).
    pub fn invalidate(&mut self, path: &str) {
        self.cache.pop(path);
    }

    /// Compute a hash of file content bytes.
    /// Uses DefaultHasher (SipHash) for fast, non-cryptographic hashing.
    pub fn hash_content(content: &[u8]) -> u64 {
        let mut hasher = DefaultHasher::new();
        content.hash(&mut hasher);
        hasher.finish()
    }

    /// Return the number of entries currently in the cache.
    pub fn len(&self) -> usize {
        self.cache.len()
    }

    /// Return true if the cache is empty.
    pub fn is_empty(&self) -> bool {
        self.cache.is_empty()
    }

    /// Clear all entries from the cache.
    pub fn clear(&mut self) {
        self.cache.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_operations() {
        let mut cache = FileStateCache::new(10);

        assert!(cache.is_empty());
        assert!(!cache.has("test.rs"));

        cache.insert(
            "test.rs".to_string(),
            FileCacheEntry {
                content_hash: 12345,
                size_bytes: 100,
                last_modified: 1000,
            },
        );

        assert!(cache.has("test.rs"));
        assert_eq!(cache.len(), 1);

        let entry = cache.get("test.rs").unwrap();
        assert_eq!(entry.content_hash, 12345);
        assert_eq!(entry.size_bytes, 100);
    }

    #[test]
    fn test_invalidate() {
        let mut cache = FileStateCache::new(10);

        cache.insert(
            "file.rs".to_string(),
            FileCacheEntry {
                content_hash: 1,
                size_bytes: 50,
                last_modified: 500,
            },
        );

        assert!(cache.has("file.rs"));
        cache.invalidate("file.rs");
        assert!(!cache.has("file.rs"));
    }

    #[test]
    fn test_lru_eviction() {
        let mut cache = FileStateCache::new(2);

        cache.insert(
            "a.rs".to_string(),
            FileCacheEntry {
                content_hash: 1,
                size_bytes: 10,
                last_modified: 100,
            },
        );
        cache.insert(
            "b.rs".to_string(),
            FileCacheEntry {
                content_hash: 2,
                size_bytes: 20,
                last_modified: 200,
            },
        );
        cache.insert(
            "c.rs".to_string(),
            FileCacheEntry {
                content_hash: 3,
                size_bytes: 30,
                last_modified: 300,
            },
        );

        // "a.rs" should have been evicted (LRU)
        assert!(!cache.has("a.rs"));
        assert!(cache.has("b.rs"));
        assert!(cache.has("c.rs"));
    }

    #[test]
    fn test_hash_content() {
        let hash1 = FileStateCache::hash_content(b"hello world");
        let hash2 = FileStateCache::hash_content(b"hello world");
        let hash3 = FileStateCache::hash_content(b"hello world!");

        assert_eq!(hash1, hash2);
        assert_ne!(hash1, hash3);
    }

    #[test]
    fn test_clear() {
        let mut cache = FileStateCache::new(10);
        cache.insert(
            "x.rs".to_string(),
            FileCacheEntry {
                content_hash: 0,
                size_bytes: 0,
                last_modified: 0,
            },
        );
        assert!(!cache.is_empty());
        cache.clear();
        assert!(cache.is_empty());
    }
}
