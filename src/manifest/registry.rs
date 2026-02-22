//! v1.1: Package registry for publishing and installing third-party packages.
//!
//! Provides dependency resolution, package fetching (from local paths, git, and
//! the central registry), and lockfile management.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use super::{Dependency, DetailedDependency, LockedPackage, Lockfile, Manifest, ManifestError};

/// Default registry URL (placeholder for future central registry)
pub const DEFAULT_REGISTRY_URL: &str = "https://registry.astra-lang.org";

/// Package source type
#[derive(Debug, Clone, PartialEq)]
pub enum PackageSource {
    /// From a path on disk
    Path(PathBuf),
    /// From a git repository
    Git { url: String, reference: GitRef },
    /// From the central registry
    Registry { version: String },
}

/// Git reference type
#[derive(Debug, Clone, PartialEq)]
pub enum GitRef {
    Branch(String),
    Tag(String),
    Rev(String),
    Default,
}

/// A resolved package with all its metadata
#[derive(Debug, Clone)]
pub struct ResolvedPackage {
    /// Package name
    pub name: String,
    /// Resolved version
    pub version: String,
    /// Where the package comes from
    pub source: PackageSource,
    /// Path to the package on disk (after fetching)
    pub local_path: PathBuf,
    /// This package's dependencies
    pub dependencies: Vec<String>,
}

/// Package registry and resolver
pub struct PackageRegistry {
    /// Base directory for the project
    project_root: PathBuf,
    /// Directory where packages are cached
    cache_dir: PathBuf,
    /// Resolved packages
    resolved: HashMap<String, ResolvedPackage>,
}

impl PackageRegistry {
    /// Create a new package registry for a project
    pub fn new(project_root: PathBuf) -> Self {
        let cache_dir = project_root.join(".astra").join("packages");
        Self {
            project_root,
            cache_dir,
            resolved: HashMap::new(),
        }
    }

    /// Get the cache directory
    pub fn cache_dir(&self) -> &Path {
        &self.cache_dir
    }

    /// Resolve all dependencies from a manifest
    pub fn resolve(&mut self, manifest: &Manifest) -> Result<Vec<ResolvedPackage>, ManifestError> {
        // Ensure cache directory exists
        std::fs::create_dir_all(&self.cache_dir)
            .map_err(|e| ManifestError::Io(format!("Failed to create cache dir: {}", e)))?;

        let mut packages = Vec::new();

        for (name, dep) in &manifest.dependencies {
            let resolved = self.resolve_dependency(name, dep)?;
            self.resolved.insert(name.clone(), resolved.clone());
            packages.push(resolved);
        }

        Ok(packages)
    }

    /// Resolve a single dependency
    fn resolve_dependency(
        &self,
        name: &str,
        dep: &Dependency,
    ) -> Result<ResolvedPackage, ManifestError> {
        match dep {
            Dependency::Simple(version) => {
                // Simple version string -> registry dependency
                Ok(ResolvedPackage {
                    name: name.to_string(),
                    version: version.clone(),
                    source: PackageSource::Registry {
                        version: version.clone(),
                    },
                    local_path: self.cache_dir.join(name).join(version),
                    dependencies: Vec::new(),
                })
            }
            Dependency::Detailed(detail) => self.resolve_detailed(name, detail),
        }
    }

    /// Resolve a detailed dependency spec
    fn resolve_detailed(
        &self,
        name: &str,
        detail: &DetailedDependency,
    ) -> Result<ResolvedPackage, ManifestError> {
        if let Some(ref path) = detail.path {
            // Local path dependency
            let local_path = self.project_root.join(path);
            let version = detail
                .version
                .clone()
                .unwrap_or_else(|| "0.0.0".to_string());
            return Ok(ResolvedPackage {
                name: name.to_string(),
                version,
                source: PackageSource::Path(local_path.clone()),
                local_path,
                dependencies: Vec::new(),
            });
        }

        if let Some(ref git_url) = detail.git {
            // Git dependency
            let git_ref = if let Some(ref branch) = detail.branch {
                GitRef::Branch(branch.clone())
            } else if let Some(ref tag) = detail.tag {
                GitRef::Tag(tag.clone())
            } else if let Some(ref rev) = detail.rev {
                GitRef::Rev(rev.clone())
            } else {
                GitRef::Default
            };

            let version = detail
                .version
                .clone()
                .unwrap_or_else(|| "0.0.0-git".to_string());
            let cache_path = self.cache_dir.join(name).join(sanitize_for_path(git_url));

            return Ok(ResolvedPackage {
                name: name.to_string(),
                version,
                source: PackageSource::Git {
                    url: git_url.clone(),
                    reference: git_ref,
                },
                local_path: cache_path,
                dependencies: Vec::new(),
            });
        }

        // Registry dependency
        let version = detail.version.clone().ok_or_else(|| {
            ManifestError::Validation(format!("No version specified for `{}`", name))
        })?;

        Ok(ResolvedPackage {
            name: name.to_string(),
            version: version.clone(),
            source: PackageSource::Registry {
                version: version.clone(),
            },
            local_path: self.cache_dir.join(name).join(&version),
            dependencies: Vec::new(),
        })
    }

    /// Install resolved dependencies (fetch them to local cache)
    pub fn install(&self, packages: &[ResolvedPackage]) -> Result<(), ManifestError> {
        for pkg in packages {
            match &pkg.source {
                PackageSource::Path(path) => {
                    // Verify path exists
                    if !path.exists() {
                        return Err(ManifestError::Validation(format!(
                            "Path dependency `{}` not found at: {}",
                            pkg.name,
                            path.display()
                        )));
                    }
                }
                PackageSource::Git { url, reference } => {
                    // Create cache directory
                    if !pkg.local_path.exists() {
                        std::fs::create_dir_all(&pkg.local_path)
                            .map_err(|e| ManifestError::Io(e.to_string()))?;

                        // Clone the git repository
                        let ref_arg = match reference {
                            GitRef::Branch(b) => format!("--branch {}", b),
                            GitRef::Tag(t) => format!("--branch {}", t),
                            GitRef::Rev(_) => String::new(),
                            GitRef::Default => String::new(),
                        };

                        let status = std::process::Command::new("git")
                            .args(["clone", "--depth", "1"])
                            .args(if ref_arg.is_empty() {
                                vec![]
                            } else {
                                ref_arg.split_whitespace().map(String::from).collect()
                            })
                            .arg(url)
                            .arg(&pkg.local_path)
                            .status()
                            .map_err(|e| ManifestError::Io(format!("git clone failed: {}", e)))?;

                        if !status.success() {
                            return Err(ManifestError::Io(format!(
                                "git clone failed for `{}`",
                                pkg.name
                            )));
                        }

                        // If a specific revision was requested, check it out
                        if let GitRef::Rev(rev) = reference {
                            let status = std::process::Command::new("git")
                                .current_dir(&pkg.local_path)
                                .args(["checkout", rev])
                                .status()
                                .map_err(|e| {
                                    ManifestError::Io(format!("git checkout failed: {}", e))
                                })?;

                            if !status.success() {
                                return Err(ManifestError::Io(format!(
                                    "git checkout {} failed for `{}`",
                                    rev, pkg.name
                                )));
                            }
                        }
                    }
                }
                PackageSource::Registry { version } => {
                    // For now, registry packages are not yet downloadable
                    // This will be implemented when the central registry is available
                    if !pkg.local_path.exists() {
                        return Err(ManifestError::Validation(format!(
                            "Package `{}@{}` not found in local cache. \
                             Central registry is not yet available. \
                             Use `path` or `git` dependencies instead.",
                            pkg.name, version
                        )));
                    }
                }
            }
        }
        Ok(())
    }

    /// Generate a lockfile from resolved packages
    pub fn generate_lockfile(&self, packages: &[ResolvedPackage]) -> Lockfile {
        let locked: Vec<LockedPackage> = packages
            .iter()
            .map(|pkg| {
                let source = match &pkg.source {
                    PackageSource::Path(p) => format!("path+{}", p.display()),
                    PackageSource::Git { url, .. } => format!("git+{}", url),
                    PackageSource::Registry { .. } => {
                        format!("registry+{}", DEFAULT_REGISTRY_URL)
                    }
                };
                LockedPackage {
                    name: pkg.name.clone(),
                    version: pkg.version.clone(),
                    source,
                    checksum: None,
                    dependencies: pkg.dependencies.clone(),
                }
            })
            .collect();

        Lockfile {
            version: 1,
            packages: locked,
        }
    }

    /// Get the search paths for all resolved dependencies
    pub fn search_paths(&self) -> Vec<PathBuf> {
        self.resolved
            .values()
            .map(|pkg| pkg.local_path.clone())
            .collect()
    }

    /// Get a resolved package by name
    pub fn get_package(&self, name: &str) -> Option<&ResolvedPackage> {
        self.resolved.get(name)
    }
}

/// Sanitize a URL to be used in a file path
fn sanitize_for_path(url: &str) -> String {
    url.replace("://", "_").replace(['/', ':', '.'], "_")
}
#[cfg(test)]
#[path = "registry_tests.rs"]
mod tests;
