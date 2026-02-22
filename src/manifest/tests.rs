use super::*;

#[test]
fn test_parse_minimal_manifest() {
    let content = r#"
[package]
name = "test"
version = "0.1.0"
"#;

    let manifest = Manifest::parse(content).unwrap();
    assert_eq!(manifest.package.name, "test");
    assert_eq!(manifest.package.version, "0.1.0");
}

#[test]
fn test_parse_full_manifest() {
    let content = r#"
[package]
name = "myapp"
version = "1.0.0"
description = "My Astra application"
authors = ["Alice <alice@example.com>"]
license = "MIT"
main = "src/app.astra"

[targets]
default = "wasm"

[targets.wasm]
output = "build/app.wasm"
optimize_size = true

[dependencies]
std = "0.1"
http = { version = "1.0", features = ["json"] }
local-lib = { path = "../lib" }

[dev-dependencies]
test-utils = "0.1"

[features]
default = ["std"]
extra = ["http/extra"]
"#;

    let manifest = Manifest::parse(content).unwrap();
    assert_eq!(manifest.package.name, "myapp");
    assert_eq!(manifest.targets.default, "wasm");
    assert!(manifest.dependencies.contains_key("std"));
    assert!(manifest.dependencies.contains_key("http"));
}
