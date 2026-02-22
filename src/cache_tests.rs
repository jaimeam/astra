use super::*;
use tempfile::TempDir;

#[test]
fn test_hash_content_deterministic() {
    let h1 = hash_content("hello world");
    let h2 = hash_content("hello world");
    assert_eq!(h1, h2);
}

#[test]
fn test_hash_content_different() {
    let h1 = hash_content("hello");
    let h2 = hash_content("world");
    assert_ne!(h1, h2);
}

#[test]
fn test_cache_round_trip() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();

    let mut cache = CheckCache::default();
    let file_path = root.join("test.astra");
    std::fs::write(&file_path, "module test").unwrap();

    cache.store(
        &file_path,
        CachedFileResult {
            content_hash: 12345,
            errors: 0,
            warnings: 1,
            diagnostics: vec!["{}".to_string()],
        },
    );

    cache.save(root).unwrap();

    let loaded = CheckCache::load(root);
    let result = loaded.lookup(&file_path, 12345);
    assert!(result.is_some());
    let r = result.unwrap();
    assert_eq!(r.errors, 0);
    assert_eq!(r.warnings, 1);
}

#[test]
fn test_cache_miss_on_different_hash() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();

    let mut cache = CheckCache::default();
    let file_path = root.join("test.astra");
    std::fs::write(&file_path, "module test").unwrap();

    cache.store(
        &file_path,
        CachedFileResult {
            content_hash: 12345,
            errors: 0,
            warnings: 0,
            diagnostics: vec![],
        },
    );

    // Different hash should miss
    assert!(cache.lookup(&file_path, 99999).is_none());
}

#[test]
fn test_cache_load_missing_file() {
    let tmp = TempDir::new().unwrap();
    let cache = CheckCache::load(tmp.path());
    assert!(cache.files.is_empty());
}
