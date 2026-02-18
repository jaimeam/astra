//! Handler for the `astra fmt` subcommand.

use std::path::PathBuf;

use crate::parser::{Lexer, Parser as AstraParser, SourceFile};

use super::{json_escape, walkdir};

pub(crate) fn run_fmt(
    paths: &[PathBuf],
    check: bool,
    json: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut files_formatted = 0;
    let mut files_changed = 0;
    let mut changed_files: Vec<String> = Vec::new();

    for path in paths {
        if path.is_file() && path.extension().is_some_and(|ext| ext == "astra") {
            match fmt_file(path, check)? {
                FmtResult::Unchanged => files_formatted += 1,
                FmtResult::Changed => {
                    files_formatted += 1;
                    files_changed += 1;
                    changed_files.push(path.display().to_string());
                }
                FmtResult::Error => {}
            }
        } else if path.is_dir() {
            for entry in walkdir(path)? {
                if entry.extension().is_some_and(|ext| ext == "astra") {
                    match fmt_file(&entry, check)? {
                        FmtResult::Unchanged => files_formatted += 1,
                        FmtResult::Changed => {
                            files_formatted += 1;
                            files_changed += 1;
                            changed_files.push(entry.display().to_string());
                        }
                        FmtResult::Error => {}
                    }
                }
            }
        }
    }

    if json {
        let files_json: Vec<String> = changed_files.iter().map(|f| json_escape(f)).collect();
        println!(
            "{{\"checked\":{},\"changed\":{},\"files\":[{}]}}",
            files_formatted,
            files_changed,
            files_json.join(",")
        );
    } else if check {
        if files_changed > 0 {
            println!(
                "{} file(s) would be reformatted ({} checked)",
                files_changed, files_formatted
            );
            std::process::exit(1);
        } else {
            println!("{} file(s) already formatted", files_formatted);
        }
    } else {
        println!(
            "Formatted {} file(s) ({} changed)",
            files_formatted, files_changed
        );
    }

    if check && files_changed > 0 && json {
        std::process::exit(1);
    }

    Ok(())
}

enum FmtResult {
    Unchanged,
    Changed,
    Error,
}

fn fmt_file(path: &PathBuf, check: bool) -> Result<FmtResult, Box<dyn std::error::Error>> {
    let source =
        std::fs::read_to_string(path).map_err(|e| format!("Failed to read {:?}: {}", path, e))?;

    let source_file = SourceFile::new(path.clone(), source.clone());
    let lexer = Lexer::new(&source_file);
    let mut parser = AstraParser::new(lexer, source_file.clone());

    let module = match parser.parse_module() {
        Ok(m) => m,
        Err(e) => {
            eprintln!("Parse error in {:?}:\n{}", path, e.format_text(&source));
            return Ok(FmtResult::Error);
        }
    };

    let mut formatter = crate::formatter::Formatter::new();
    let formatted = formatter.format_module(&module);

    if formatted == source {
        return Ok(FmtResult::Unchanged);
    }

    if check {
        println!("Would reformat: {:?}", path);
        Ok(FmtResult::Changed)
    } else {
        std::fs::write(path, &formatted)
            .map_err(|e| format!("Failed to write {:?}: {}", path, e))?;
        println!("Formatted: {:?}", path);
        Ok(FmtResult::Changed)
    }
}
