//! Manifest parsing for Astra projects (astra.toml)

pub mod registry;

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

/// Astra project manifest
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Manifest {
    /// Package information
    pub package: Package,

    /// Target configurations
    #[serde(default)]
    pub targets: Targets,

    /// Dependencies
    #[serde(default)]
    pub dependencies: HashMap<String, Dependency>,

    /// Development dependencies
    #[serde(default, rename = "dev-dependencies")]
    pub dev_dependencies: HashMap<String, Dependency>,

    /// Feature flags
    #[serde(default)]
    pub features: HashMap<String, Vec<String>>,
}

/// Package information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Package {
    /// Package name
    pub name: String,

    /// Package version
    pub version: String,

    /// Package description
    #[serde(default)]
    pub description: Option<String>,

    /// Package authors
    #[serde(default)]
    pub authors: Vec<String>,

    /// License
    #[serde(default)]
    pub license: Option<String>,

    /// Repository URL
    #[serde(default)]
    pub repository: Option<String>,

    /// Documentation URL
    #[serde(default)]
    pub documentation: Option<String>,

    /// Entry point for executables
    #[serde(default)]
    pub main: Option<String>,
}

/// Target configurations
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Targets {
    /// Default target
    #[serde(default = "default_target")]
    pub default: String,

    /// WASM target options
    #[serde(default)]
    pub wasm: Option<WasmTarget>,

    /// Native target options
    #[serde(default)]
    pub native: Option<NativeTarget>,
}

fn default_target() -> String {
    "interpreter".to_string()
}

/// WASM target configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WasmTarget {
    /// Output filename
    #[serde(default)]
    pub output: Option<String>,

    /// Optimize for size
    #[serde(default)]
    pub optimize_size: bool,
}

/// Native target configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NativeTarget {
    /// Output filename
    #[serde(default)]
    pub output: Option<String>,

    /// Optimization level (0-3)
    #[serde(default)]
    pub opt_level: u8,
}

/// Dependency specification
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Dependency {
    /// Simple version string
    Simple(String),

    /// Detailed dependency spec
    Detailed(DetailedDependency),
}

/// Detailed dependency specification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetailedDependency {
    /// Version requirement
    #[serde(default)]
    pub version: Option<String>,

    /// Git repository
    #[serde(default)]
    pub git: Option<String>,

    /// Git branch
    #[serde(default)]
    pub branch: Option<String>,

    /// Git tag
    #[serde(default)]
    pub tag: Option<String>,

    /// Git revision
    #[serde(default)]
    pub rev: Option<String>,

    /// Local path
    #[serde(default)]
    pub path: Option<String>,

    /// Features to enable
    #[serde(default)]
    pub features: Vec<String>,

    /// Default features
    #[serde(default = "default_true")]
    pub default_features: bool,

    /// Optional dependency
    #[serde(default)]
    pub optional: bool,
}

fn default_true() -> bool {
    true
}

impl Manifest {
    /// Load a manifest from a file
    pub fn load(path: &Path) -> Result<Self, ManifestError> {
        let content =
            std::fs::read_to_string(path).map_err(|e| ManifestError::Io(e.to_string()))?;

        Self::parse(&content)
    }

    /// Parse a manifest from TOML string
    pub fn parse(content: &str) -> Result<Self, ManifestError> {
        toml::from_str(content).map_err(|e| ManifestError::Parse(e.to_string()))
    }

    /// Serialize the manifest to TOML
    pub fn to_toml(&self) -> Result<String, ManifestError> {
        toml::to_string_pretty(self).map_err(|e| ManifestError::Serialize(e.to_string()))
    }

    /// Get the entry point file
    pub fn entry_point(&self) -> &str {
        self.package.main.as_deref().unwrap_or("src/main.astra")
    }
}

/// Manifest errors
#[derive(Debug, Clone)]
pub enum ManifestError {
    /// IO error
    Io(String),
    /// Parse error
    Parse(String),
    /// Serialization error
    Serialize(String),
    /// Validation error
    Validation(String),
}

impl std::fmt::Display for ManifestError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(msg) => write!(f, "IO error: {}", msg),
            Self::Parse(msg) => write!(f, "Parse error: {}", msg),
            Self::Serialize(msg) => write!(f, "Serialization error: {}", msg),
            Self::Validation(msg) => write!(f, "Validation error: {}", msg),
        }
    }
}

impl std::error::Error for ManifestError {}

/// Lockfile for reproducible builds
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Lockfile {
    /// Lockfile version
    pub version: u32,

    /// Locked packages
    #[serde(default)]
    pub packages: Vec<LockedPackage>,
}

/// A locked package
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LockedPackage {
    /// Package name
    pub name: String,

    /// Exact version
    pub version: String,

    /// Source (registry, git, path)
    pub source: String,

    /// Checksum
    #[serde(default)]
    pub checksum: Option<String>,

    /// Dependencies
    #[serde(default)]
    pub dependencies: Vec<String>,
}

impl Lockfile {
    /// Create a new empty lockfile
    pub fn new() -> Self {
        Self {
            version: 1,
            packages: Vec::new(),
        }
    }

    /// Load a lockfile
    pub fn load(path: &Path) -> Result<Self, ManifestError> {
        let content =
            std::fs::read_to_string(path).map_err(|e| ManifestError::Io(e.to_string()))?;

        toml::from_str(&content).map_err(|e| ManifestError::Parse(e.to_string()))
    }

    /// Save the lockfile
    pub fn save(&self, path: &Path) -> Result<(), ManifestError> {
        let content =
            toml::to_string_pretty(self).map_err(|e| ManifestError::Serialize(e.to_string()))?;

        std::fs::write(path, content).map_err(|e| ManifestError::Io(e.to_string()))
    }
}

impl Default for Lockfile {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
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
}
