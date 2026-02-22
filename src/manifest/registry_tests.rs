use super::*;

#[test]
fn test_resolve_simple_dependency() {
    let dir = tempfile::tempdir().unwrap();
    let mut registry = PackageRegistry::new(dir.path().to_path_buf());

    let manifest_str = r#"
[package]
name = "test"
version = "0.1.0"

[dependencies]
mylib = "1.0"
"#;
    let manifest = Manifest::parse(manifest_str).unwrap();
    let packages = registry.resolve(&manifest).unwrap();

    assert_eq!(packages.len(), 1);
    assert_eq!(packages[0].name, "mylib");
    assert_eq!(packages[0].version, "1.0");
    assert!(matches!(packages[0].source, PackageSource::Registry { .. }));
}

#[test]
fn test_resolve_path_dependency() {
    let dir = tempfile::tempdir().unwrap();
    let mut registry = PackageRegistry::new(dir.path().to_path_buf());

    let manifest_str = r#"
[package]
name = "test"
version = "0.1.0"

[dependencies]
local-lib = { path = "../lib" }
"#;
    let manifest = Manifest::parse(manifest_str).unwrap();
    let packages = registry.resolve(&manifest).unwrap();

    assert_eq!(packages.len(), 1);
    assert_eq!(packages[0].name, "local-lib");
    assert!(matches!(packages[0].source, PackageSource::Path(_)));
}

#[test]
fn test_resolve_git_dependency() {
    let dir = tempfile::tempdir().unwrap();
    let mut registry = PackageRegistry::new(dir.path().to_path_buf());

    let manifest_str = r#"
[package]
name = "test"
version = "0.1.0"

[dependencies]
remote-lib = { git = "https://github.com/example/lib", branch = "main" }
"#;
    let manifest = Manifest::parse(manifest_str).unwrap();
    let packages = registry.resolve(&manifest).unwrap();

    assert_eq!(packages.len(), 1);
    assert_eq!(packages[0].name, "remote-lib");
    assert!(matches!(packages[0].source, PackageSource::Git { .. }));
}

#[test]
fn test_generate_lockfile() {
    let dir = tempfile::tempdir().unwrap();
    let registry = PackageRegistry::new(dir.path().to_path_buf());

    let packages = vec![ResolvedPackage {
        name: "mylib".to_string(),
        version: "1.0.0".to_string(),
        source: PackageSource::Registry {
            version: "1.0.0".to_string(),
        },
        local_path: dir.path().join("mylib").join("1.0.0"),
        dependencies: vec![],
    }];

    let lockfile = registry.generate_lockfile(&packages);
    assert_eq!(lockfile.version, 1);
    assert_eq!(lockfile.packages.len(), 1);
    assert_eq!(lockfile.packages[0].name, "mylib");
}

#[test]
fn test_sanitize_for_path() {
    let result = sanitize_for_path("https://github.com/example/lib");
    // URL characters are sanitized to underscores
    assert!(result.contains("github"));
    assert!(result.contains("example"));
    assert!(result.contains("lib"));
    assert!(!result.contains('/'));
    assert!(!result.contains(':'));
}
