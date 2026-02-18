//! Handlers for the `astra explain` and `astra fix` subcommands.

use std::collections::HashSet;
use std::path::PathBuf;

use crate::parser::{Lexer, Parser as AstraParser, SourceFile};

use super::{configure_checker_search_paths, walkdir};

pub(crate) fn run_fix(
    paths: &[PathBuf],
    only: Option<&str>,
    dry_run: bool,
    json: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    // Parse --only filter into a set of codes
    let code_filter: Option<HashSet<&str>> = only.map(|codes| codes.split(',').collect());

    // Collect all .astra files
    let mut astra_files = Vec::new();
    for path in paths {
        if path.is_file() && path.extension().is_some_and(|ext| ext == "astra") {
            astra_files.push(path.clone());
        } else if path.is_dir() {
            for entry in walkdir(path)? {
                if entry.extension().is_some_and(|ext| ext == "astra") {
                    astra_files.push(entry);
                }
            }
        }
    }

    let mut total_fixes = 0;
    let mut files_fixed = 0;

    for file_path in &astra_files {
        let source = std::fs::read_to_string(file_path)
            .map_err(|e| format!("Failed to read {:?}: {}", file_path, e))?;

        // Parse and check
        let source_file = SourceFile::new(file_path.clone(), source.clone());
        let lexer = Lexer::new(&source_file);
        let mut parser = AstraParser::new(lexer, source_file.clone());
        let module = match parser.parse_module() {
            Ok(m) => m,
            Err(_) => continue, // Skip files with parse errors (can't fix syntax errors)
        };

        let mut checker = crate::typechecker::TypeChecker::new();
        configure_checker_search_paths(&mut checker, file_path.parent());
        let _type_result = checker.check_module(&module);
        let all_diags = checker.diagnostics();

        // Collect all edits from suggestions, grouped by file
        let mut edits: Vec<crate::diagnostics::Edit> = Vec::new();
        for diag in all_diags.diagnostics() {
            // Apply code filter if specified
            if let Some(ref filter) = code_filter {
                if !filter.contains(diag.code.as_str()) {
                    continue;
                }
            }

            for suggestion in &diag.suggestions {
                for edit in &suggestion.edits {
                    edits.push(edit.clone());
                }
            }
        }

        if edits.is_empty() {
            continue;
        }

        // Sort edits by start position (descending) so we apply from end to start
        // This avoids offset invalidation
        edits.sort_by(|a, b| b.span.start.cmp(&a.span.start));

        // Deduplicate edits at the same span
        edits.dedup_by(|a, b| a.span.start == b.span.start && a.span.end == b.span.end);

        let fix_count = edits.len();
        let mut fixed_source = source.clone();

        for edit in &edits {
            // Apply edit: replace bytes from span.start..span.end with replacement
            if edit.span.start <= fixed_source.len() && edit.span.end <= fixed_source.len() {
                fixed_source.replace_range(edit.span.start..edit.span.end, &edit.replacement);
            }
        }

        if fixed_source != source {
            total_fixes += fix_count;
            files_fixed += 1;

            if dry_run {
                if json {
                    println!(
                        "{{\"file\":{},\"fixes\":{}}}",
                        serde_json::to_string(&file_path.display().to_string()).unwrap_or_default(),
                        fix_count
                    );
                } else {
                    println!(
                        "Would fix {} issue(s) in {:?}",
                        fix_count,
                        file_path.display()
                    );
                }
            } else {
                std::fs::write(file_path, &fixed_source)
                    .map_err(|e| format!("Failed to write {:?}: {}", file_path, e))?;
                if json {
                    println!(
                        "{{\"file\":{},\"fixes\":{}}}",
                        serde_json::to_string(&file_path.display().to_string()).unwrap_or_default(),
                        fix_count
                    );
                } else {
                    println!("Fixed {} issue(s) in {:?}", fix_count, file_path.display());
                }
            }
        }
    }

    if dry_run {
        println!(
            "\nDry run: {} fix(es) would be applied across {} file(s)",
            total_fixes, files_fixed
        );
    } else if total_fixes > 0 {
        println!(
            "\nApplied {} fix(es) across {} file(s)",
            total_fixes, files_fixed
        );
    } else {
        println!("No auto-fixable issues found");
    }

    Ok(())
}

pub(crate) fn run_explain(code: &str) -> Result<(), Box<dyn std::error::Error>> {
    let explanation = get_error_explanation(code);
    match explanation {
        Some(text) => {
            println!("{}", text);
        }
        None => {
            eprintln!("Unknown error code: {}", code);
            eprintln!();
            eprintln!("Valid error codes:");
            eprintln!("  E0xxx  Syntax/parsing errors (E0001-E0011)");
            eprintln!("  E1xxx  Type errors (E1001-E1016)");
            eprintln!("  E2xxx  Effect errors (E2001-E2007)");
            eprintln!("  E3xxx  Contract violations (E3001-E3005)");
            eprintln!("  E4xxx  Runtime errors (E4001-E4008)");
            eprintln!("  W0xxx  Warnings (W0001-W0007)");
            std::process::exit(1);
        }
    }
    Ok(())
}

/// Get a detailed explanation for an error code.
pub(super) fn get_error_explanation(code: &str) -> Option<String> {
    let explanation = match code {
        // Syntax errors
        "E0001" => {
            r#"E0001: Unexpected token

The parser encountered a token that doesn't fit the expected grammar.

Example:
  fn add(a Int) -> Int {  # expected ':', found 'Int'
    a
  }

Fix: Add the missing punctuation or correct the syntax.
"#
        }
        "E0002" => {
            r#"E0002: Unterminated string literal

A string literal was opened with `"` but never closed.

Example:
  let s = "hello

Fix: Close the string with a matching `"`.
"#
        }
        "E0003" => {
            r#"E0003: Invalid number literal

A number literal contains invalid characters.

Example:
  let n = 123abc

Fix: Ensure numbers contain only digits (and optionally one `.` for floats).
"#
        }
        "E0004" => {
            r#"E0004: Missing closing delimiter

An opening bracket, brace, or parenthesis was not closed.

Example:
  fn foo() {
    let x = (1 + 2
  }

Fix: Add the matching closing delimiter `)`, `]`, or `}`.
"#
        }
        "E0005" => {
            r#"E0005: Invalid identifier

An identifier contains invalid characters or starts incorrectly.

Fix: Identifiers must start with a letter or underscore, followed by
letters, digits, or underscores.
"#
        }
        "E0006" => {
            r#"E0006: Reserved keyword used as identifier

A reserved keyword cannot be used as a variable or function name.

Example:
  let match = 5  # 'match' is reserved

Fix: Choose a different name that isn't a keyword.
"#
        }
        "E0007" => {
            r#"E0007: Invalid escape sequence

A string contains an unrecognized escape sequence.

Example:
  let s = "hello\q"  # \q is not valid

Valid escape sequences: \n, \r, \t, \\, \"

Fix: Use a valid escape sequence or remove the backslash.
"#
        }
        "E0008" => {
            r#"E0008: Unexpected end of file

The parser reached the end of the file while still expecting more tokens.

Example:
  fn foo() {

Fix: Ensure all blocks and expressions are properly completed.
"#
        }
        "E0009" => {
            r#"E0009: Invalid module declaration

The module declaration is malformed.

Example:
  module    # missing module name

Fix: Provide a valid module path like `module my_project.utils`.
"#
        }
        "E0010" => {
            r#"E0010: Duplicate module declaration

Two modules with the same name exist in the project.

Fix: Rename one of the modules to avoid the conflict.
"#
        }
        "E0011" => {
            r#"E0011: Module not found

An import refers to a module that doesn't exist.

Example:
  import std.nonexistent

Fix: Check the module name and ensure it exists. Available stdlib modules:
  std.core, std.list, std.math, std.option, std.result, std.string,
  std.collections, std.json, std.io, std.iter, std.error, std.prelude
"#
        }

        // Type errors
        "E1001" => {
            r#"E1001: Type mismatch

The type of an expression doesn't match what was expected.

Example:
  fn add(a: Int, b: Int) -> Int {
    "hello"  # expected Int, got Text
  }

Fix: Ensure the expression has the expected type. The compiler often
suggests the correct type in the error message.
"#
        }
        "E1002" => {
            r#"E1002: Unknown identifier

A name was used that hasn't been defined in the current scope.

Example:
  fn foo() -> Int {
    bar  # 'bar' is not defined
  }

Fix: Define the variable before use, check for typos, or import the
necessary module. The compiler may suggest similar names.

This diagnostic often includes an auto-fix suggestion that can be
applied with `astra fix`.
"#
        }
        "E1003" => {
            r#"E1003: Missing type annotation

A type annotation is required but was not provided.

Fix: Add an explicit type annotation where the compiler indicates.
"#
        }
        "E1004" => {
            r#"E1004: Non-exhaustive match

A `match` expression doesn't cover all possible cases.

Example:
  match opt {
    Some(x) => x
    # missing: None => ...
  }

Fix: Add the missing patterns. The compiler lists which cases are
missing. This diagnostic may include an auto-fix suggestion.
"#
        }
        "E1005" => {
            r#"E1005: Duplicate field

A record type or literal has the same field name twice.

Fix: Remove or rename the duplicate field.
"#
        }
        "E1006" => {
            r#"E1006: Unknown field

A field name was used that doesn't exist in the record type.

Fix: Check the field name for typos and verify it exists in the type definition.
"#
        }
        "E1007" => {
            r#"E1007: Wrong argument count

A function was called with the wrong number of arguments.

Example:
  fn add(a: Int, b: Int) -> Int { a + b }
  add(1)  # expected 2 args, got 1

Fix: Provide the correct number of arguments.
"#
        }
        "E1008" => {
            r#"E1008: Cannot infer type

The compiler cannot determine the type of an expression.

Fix: Add an explicit type annotation to help the compiler.
"#
        }
        "E1009" => {
            r#"E1009: Recursive type

A type definition is recursive in a way that creates an infinite type.

Fix: Use an enum with a base case to break the recursion (e.g., a linked list).
"#
        }
        "E1010" => {
            r#"E1010: Invalid type application

Type arguments were applied to a type that doesn't accept them.

Fix: Remove the type arguments or check the type definition.
"#
        }
        "E1011" => {
            r#"E1011: Duplicate type

A type with this name is already defined.

Fix: Rename one of the type definitions.
"#
        }
        "E1012" => {
            r#"E1012: Unknown type

A type name was used that hasn't been defined.

Fix: Define the type, check for typos, or import the necessary module.
"#
        }
        "E1013" => {
            r#"E1013: Expected function

A non-function value was called as if it were a function.

Fix: Ensure the expression being called is actually a function.
"#
        }
        "E1014" => {
            r#"E1014: Expected record

An expression was used in a record context but isn't a record type.

Fix: Ensure the expression is a record type with the expected fields.
"#
        }
        "E1015" => {
            r#"E1015: Expected enum

An expression was used in an enum context but isn't an enum type.

Fix: Ensure the expression is an enum type with the expected variants.
"#
        }
        "E1016" => {
            r#"E1016: Trait constraint not satisfied

A generic function requires a type to implement a trait, but the
concrete type used at the call site doesn't.

Example:
  fn sort[T: Ord](items: List[T]) -> List[T] { ... }
  sort(["a", "b"])  # Text doesn't implement Ord

Fix: Use a type that implements the required trait, or add an
`impl TraitName for YourType` block.
"#
        }

        // Effect errors
        "E2001" => {
            r#"E2001: Effect not declared

A function uses an effect that isn't listed in its `effects(...)` clause.

Example:
  fn greet() {        # missing effects(Console)
    println("hello")  # uses Console effect
  }

Fix: Add the missing effect to the function's effects clause.
This diagnostic includes an auto-fix suggestion.
"#
        }
        "E2002" => {
            r#"E2002: Unknown effect

An effect name was used that doesn't exist.

Fix: Check the effect name. Built-in effects: Console, Fs, Net, Clock, Rand, Env.
"#
        }
        "E2003" => {
            r#"E2003: Capability not available

A function requires an effect capability that isn't provided at runtime.

Fix: Ensure the capability is provided when running the program, or mock
it in test contexts with `using effects(EffectName = ...)`.
"#
        }
        "E2004" => {
            r#"E2004: Effectful call in pure context

An effectful function was called from a function that doesn't declare effects.

Fix: Add the necessary effects to the calling function's signature.
"#
        }
        "E2005" => {
            r#"E2005: Effect mismatch

The effects used by a function don't match its declaration.

Fix: Update the effects clause to match actual usage.
"#
        }
        "E2006" => {
            r#"E2006: Effect not mockable

An effect was used in a test's `using effects(...)` clause that can't be mocked.

Fix: Use the correct mock constructor (e.g., Clock.fixed(100), Rand.seeded(42)).
"#
        }
        "E2007" => {
            r#"E2007: Invalid capability injection

A capability was injected incorrectly in a `using effects(...)` clause.

Fix: Check the syntax and use the correct constructor.
"#
        }

        // Contract errors
        "E3001" => {
            r#"E3001: Precondition violation

A function's `requires` contract was violated at call time.

Example:
  fn divide(a: Int, b: Int) -> Int
    requires b != 0
  { a / b }

  divide(10, 0)  # E3001: requires b != 0

Fix: Ensure the arguments satisfy the precondition before calling.
"#
        }
        "E3002" => {
            r#"E3002: Postcondition violation

A function's `ensures` contract was violated on return.

Fix: The function's implementation doesn't satisfy its contract. Fix the
implementation to ensure the return value meets the postcondition.
"#
        }
        "E3003" => {
            r#"E3003: Invariant violation

A type's invariant was violated during construction.

Example:
  type Positive = Int invariant self > 0
  let p: Positive = -5  # E3003: invariant self > 0 violated

Fix: Ensure the value satisfies the type's invariant.
"#
        }
        "E3004" => {
            r#"E3004: Invalid contract expression

A contract expression (requires/ensures/invariant) is malformed.

Fix: Ensure the contract is a valid boolean expression.
"#
        }
        "E3005" => {
            r#"E3005: Contract binding unavailable

A contract references a variable that isn't in scope.

Fix: Only reference function parameters in `requires`, and `result` plus
parameters in `ensures`.
"#
        }

        // Runtime errors
        "E4001" => {
            r#"E4001: Division by zero

An integer or float division by zero was attempted.

Fix: Check that the divisor is non-zero before dividing.
Use a `requires` contract to enforce this statically.
"#
        }
        "E4002" => {
            r#"E4002: Index out of bounds

A list or string was accessed with an index outside its valid range.

Fix: Ensure the index is within bounds (0 to len-1).
"#
        }
        "E4003" => {
            r#"E4003: Contract violation

A contract check failed at runtime (general).

Fix: See E3001-E3003 for specific contract violation types.
"#
        }
        "E4004" => {
            r#"E4004: Resource limit exceeded

A resource limit (memory, recursion depth, etc.) was exceeded.

Fix: Reduce the size of the computation or optimize the algorithm.
"#
        }
        "E4005" => {
            r#"E4005: Capability denied

An effect capability was requested but not available.

Fix: Ensure the required capability is provided. When running with
`astra run`, all capabilities are available. In tests, mock them with
`using effects(...)`.
"#
        }
        "E4006" => {
            r#"E4006: Integer overflow

An integer operation overflowed the 64-bit range.

Fix: Use smaller values or check for overflow before the operation.
"#
        }
        "E4007" => {
            r#"E4007: Stack overflow

Too many nested function calls caused a stack overflow.

Fix: Use tail recursion (the compiler optimizes tail-recursive calls
automatically) or convert to an iterative approach.
"#
        }
        "E4008" => {
            r#"E4008: Assertion failed

An `assert` expression evaluated to false.

Example:
  assert(x > 0, "x must be positive")

Fix: Ensure the asserted condition holds, or fix the logic that
produces the incorrect value.
"#
        }

        // Warnings
        "W0001" => {
            r#"W0001: Unused variable

A variable was defined but never used.

Example:
  let x = 42  # x is never used

Fix: Remove the variable, or prefix its name with `_` to indicate
it's intentionally unused.

This warning includes an auto-fix suggestion (`astra fix`).
"#
        }
        "W0002" => {
            r#"W0002: Unused import

An import statement brings a name into scope that is never used.

Fix: Remove the unused import.

This warning includes an auto-fix suggestion (`astra fix`).
"#
        }
        "W0003" => {
            r#"W0003: Unreachable code

Code after a `return` statement can never be executed.

Example:
  fn foo() -> Int {
    return 42
    let x = 10  # unreachable
  }

Fix: Remove the unreachable code or restructure the control flow.
"#
        }
        "W0004" => {
            r#"W0004: Deprecated

A deprecated feature or function is being used.

Fix: Use the recommended replacement. Check the deprecation notice
for migration guidance.
"#
        }
        "W0005" => {
            r#"W0005: Wildcard match

A match expression uses a wildcard `_` pattern. While valid, this may
hide missing cases when new variants are added to an enum.

Fix: Consider matching all variants explicitly for better exhaustiveness.
"#
        }
        "W0006" => {
            r#"W0006: Shadowed binding

A new variable shadows an existing binding with the same name.

Fix: Rename the inner variable to avoid confusion, or prefix the
outer one with `_` if it's intentionally replaced.
"#
        }
        "W0007" => {
            r#"W0007: Redundant type annotation

A type annotation is provided but matches the inferred type exactly.

Fix: Remove the type annotation to reduce visual noise, or keep it
for documentation purposes.
"#
        }
        "W0008" => {
            r#"W0008: Unused function

A private function is defined but never called within its module.

Example:
  fn unused_helper() -> Int {
    42
  }

  fn main() -> Int {
    0  # unused_helper is never called
  }

Fix: Remove the function, prefix its name with `_` to indicate it's
intentionally unused, or make it `public` if it's part of the module's API.
"#
        }
        _ => return None,
    };
    Some(explanation.to_string())
}
