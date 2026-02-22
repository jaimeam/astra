//! Incremental checking cache for the Astra compiler.
//!
//! Stores content hashes alongside check results to skip re-checking
//! unchanged files. Cache is stored in `.astra-cache/` in the project root.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::path::{Path, PathBuf};

/// Name of the cache directory
const CACHE_DIR: &str = ".astra-cache";
/// Name of the cache file inside the directory
const CACHE_FILE: &str = "check-cache.json";

/// Cached result for a single file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedFileResult {
    /// Hash of the file contents when it was last checked
    pub content_hash: u64,
    /// Number of errors found
    pub errors: usize,
    /// Number of warnings found
    pub warnings: usize,
    /// Serialized diagnostics (JSON strings)
    pub diagnostics: Vec<String>,
}

/// The on-disk cache structure
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct CheckCache {
    /// Per-file cached results, keyed by canonical path
    files: HashMap<String, CachedFileResult>,
}

impl CheckCache {
    /// Load the cache from disk, returning an empty cache on any error.
    pub fn load(project_root: &Path) -> Self {
        let cache_path = project_root.join(CACHE_DIR).join(CACHE_FILE);
        match std::fs::read_to_string(&cache_path) {
            Ok(content) => serde_json::from_str(&content).unwrap_or_default(),
            Err(_) => Self::default(),
        }
    }

    /// Save the cache to disk.
    pub fn save(&self, project_root: &Path) -> Result<(), String> {
        let cache_dir = project_root.join(CACHE_DIR);
        std::fs::create_dir_all(&cache_dir)
            .map_err(|e| format!("Failed to create cache directory: {}", e))?;
        let cache_path = cache_dir.join(CACHE_FILE);
        let json = serde_json::to_string_pretty(self)
            .map_err(|e| format!("Failed to serialize cache: {}", e))?;
        std::fs::write(&cache_path, json).map_err(|e| format!("Failed to write cache: {}", e))?;
        Ok(())
    }

    /// Look up the cached result for a file. Returns `Some` if the file hash
    /// matches the cached hash (meaning the file hasn't changed).
    pub fn lookup(&self, path: &Path, current_hash: u64) -> Option<&CachedFileResult> {
        let key = path_key(path);
        self.files
            .get(&key)
            .filter(|r| r.content_hash == current_hash)
    }

    /// Store a result in the cache.
    pub fn store(&mut self, path: &Path, result: CachedFileResult) {
        let key = path_key(path);
        self.files.insert(key, result);
    }

    /// Remove stale entries for files that no longer exist.
    pub fn prune(&mut self) {
        self.files.retain(|path, _| Path::new(path).exists());
    }
}

/// Compute a content hash for a string using the default hasher.
pub fn hash_content(content: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    content.hash(&mut hasher);
    hasher.finish()
}

/// Normalize a path to a stable string key.
fn path_key(path: &Path) -> String {
    path.canonicalize()
        .unwrap_or_else(|_| path.to_path_buf())
        .to_string_lossy()
        .to_string()
}

/// Find the project root by searching for `astra.toml` upward from a directory.
pub fn find_project_root(start: &Path) -> PathBuf {
    let mut dir = if start.is_file() {
        start.parent().unwrap_or(start).to_path_buf()
    } else {
        start.to_path_buf()
    };
    loop {
        if dir.join("astra.toml").exists() {
            return dir;
        }
        match dir.parent() {
            Some(parent) => dir = parent.to_path_buf(),
            None => break,
        }
    }
    // Fall back to the current directory
    std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
}
#[cfg(test)]
#[path = "cache_tests.rs"]
mod tests;
