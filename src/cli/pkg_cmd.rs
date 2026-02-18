//! Handlers for the `astra package` and `astra pkg` subcommands.

use std::path::PathBuf;

use crate::parser::{Lexer, Parser as AstraParser, SourceFile};

use super::{configure_checker_search_paths, walkdir, PkgAction};

pub(crate) fn run_package(
    output: &PathBuf,
    target: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    // P7.2: Basic package command
    println!("Packaging project...");

    // Read project manifest
    let manifest_path = std::env::current_dir()?.join("astra.toml");
    if !manifest_path.exists() {
        return Err(
            "No astra.toml found in current directory. Run `astra init` to create one.".into(),
        );
    }

    let manifest_content = std::fs::read_to_string(&manifest_path)?;
    println!("  Found manifest: astra.toml");

    // Collect all .astra source files
    let current_dir = std::env::current_dir()?;
    let source_files = walkdir(&current_dir)?
        .into_iter()
        .filter(|p| p.extension().is_some_and(|ext| ext == "astra"))
        .collect::<Vec<_>>();

    println!("  Found {} source files", source_files.len());

    // Validate all files parse and type-check
    let mut errors = 0;
    for file in &source_files {
        let source = std::fs::read_to_string(file)?;
        let source_file = SourceFile::new(file.clone(), source.clone());
        let lexer = Lexer::new(&source_file);
        let mut parser = AstraParser::new(lexer, source_file.clone());
        match parser.parse_module() {
            Ok(module) => {
                let mut checker = crate::typechecker::TypeChecker::new();
                configure_checker_search_paths(&mut checker, file.parent());
                if checker.check_module(&module).is_err() {
                    eprintln!("  Type error in {:?}", file);
                    errors += 1;
                }
            }
            Err(_) => {
                eprintln!("  Parse error in {:?}", file);
                errors += 1;
            }
        }
    }

    if errors > 0 {
        return Err(format!("{} file(s) have errors. Fix them before packaging.", errors).into());
    }

    // Create output directory
    std::fs::create_dir_all(output)?;

    // Copy source files to output
    for file in &source_files {
        let relative = file.strip_prefix(&current_dir).unwrap_or(file);
        let dest = output.join(relative);
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::copy(file, &dest)?;
    }

    // Copy manifest
    std::fs::copy(&manifest_path, output.join("astra.toml"))?;

    // Write package metadata
    let metadata = format!(
        "# Astra Package\n# Target: {}\n# Manifest:\n{}\n",
        target, manifest_content
    );
    std::fs::write(output.join("PACKAGE.md"), metadata)?;

    println!("  Package created at {:?} (target: {})", output, target);
    println!("  {} files packaged successfully", source_files.len());

    Ok(())
}

/// v1.1: Run package management commands
pub(crate) fn run_pkg(action: PkgAction) -> Result<(), Box<dyn std::error::Error>> {
    let cwd = std::env::current_dir()?;
    let manifest_path = cwd.join("astra.toml");

    match action {
        PkgAction::Install => {
            if !manifest_path.exists() {
                return Err(
                    "No astra.toml found. Run `astra init` to create a project first.".into(),
                );
            }

            let manifest = crate::manifest::Manifest::load(&manifest_path)
                .map_err(|e| format!("Failed to load manifest: {}", e))?;

            let mut registry = crate::manifest::registry::PackageRegistry::new(cwd.clone());
            let packages = registry
                .resolve(&manifest)
                .map_err(|e| format!("Failed to resolve dependencies: {}", e))?;

            if packages.is_empty() {
                println!("No dependencies to install.");
                return Ok(());
            }

            println!("Installing {} dependencies...", packages.len());
            registry
                .install(&packages)
                .map_err(|e| format!("Failed to install: {}", e))?;

            // Generate lockfile
            let lockfile = registry.generate_lockfile(&packages);
            let lockfile_path = cwd.join("astra.lock");
            lockfile
                .save(&lockfile_path)
                .map_err(|e| format!("Failed to write lockfile: {}", e))?;

            for pkg in &packages {
                println!("  Installed {} v{}", pkg.name, pkg.version);
            }
            println!("Done.");
        }
        PkgAction::Add {
            name,
            version,
            git,
            path,
        } => {
            if !manifest_path.exists() {
                return Err(
                    "No astra.toml found. Run `astra init` to create a project first.".into(),
                );
            }

            let mut manifest = crate::manifest::Manifest::load(&manifest_path)
                .map_err(|e| format!("Failed to load manifest: {}", e))?;

            // Build the dependency spec
            if path.is_some() || git.is_some() {
                manifest.dependencies.insert(
                    name.clone(),
                    crate::manifest::Dependency::Detailed(crate::manifest::DetailedDependency {
                        version,
                        git,
                        branch: None,
                        tag: None,
                        rev: None,
                        path,
                        features: Vec::new(),
                        default_features: true,
                        optional: false,
                    }),
                );
            } else {
                let ver = version.unwrap_or_else(|| "*".to_string());
                manifest
                    .dependencies
                    .insert(name.clone(), crate::manifest::Dependency::Simple(ver));
            }

            // Write back
            let toml_str = manifest
                .to_toml()
                .map_err(|e| format!("Failed to serialize manifest: {}", e))?;
            std::fs::write(&manifest_path, toml_str)?;
            println!("Added `{}` to dependencies.", name);
        }
        PkgAction::Remove { name } => {
            if !manifest_path.exists() {
                return Err(
                    "No astra.toml found. Run `astra init` to create a project first.".into(),
                );
            }

            let mut manifest = crate::manifest::Manifest::load(&manifest_path)
                .map_err(|e| format!("Failed to load manifest: {}", e))?;

            if manifest.dependencies.remove(&name).is_some() {
                let toml_str = manifest
                    .to_toml()
                    .map_err(|e| format!("Failed to serialize manifest: {}", e))?;
                std::fs::write(&manifest_path, toml_str)?;
                println!("Removed `{}` from dependencies.", name);
            } else {
                println!("Dependency `{}` not found.", name);
            }
        }
        PkgAction::List => {
            if !manifest_path.exists() {
                return Err(
                    "No astra.toml found. Run `astra init` to create a project first.".into(),
                );
            }

            let manifest = crate::manifest::Manifest::load(&manifest_path)
                .map_err(|e| format!("Failed to load manifest: {}", e))?;

            if manifest.dependencies.is_empty() {
                println!("No dependencies.");
            } else {
                println!("Dependencies:");
                for (name, dep) in &manifest.dependencies {
                    match dep {
                        crate::manifest::Dependency::Simple(v) => {
                            println!("  {} = \"{}\"", name, v);
                        }
                        crate::manifest::Dependency::Detailed(d) => {
                            if let Some(ref v) = d.version {
                                println!("  {} = \"{}\"", name, v);
                            } else if let Some(ref g) = d.git {
                                println!("  {} (git: {})", name, g);
                            } else if let Some(ref p) = d.path {
                                println!("  {} (path: {})", name, p);
                            }
                        }
                    }
                }
            }
        }
    }

    Ok(())
}
