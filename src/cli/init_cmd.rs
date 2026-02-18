//! Handler for the `astra init` subcommand.

pub(crate) fn run_init(name: Option<&str>, lib: bool) -> Result<(), Box<dyn std::error::Error>> {
    let project_name = match name {
        Some(n) => n.to_string(),
        None => {
            let cwd = std::env::current_dir()?;
            cwd.file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("my_project")
                .to_string()
        }
    };

    // Determine project root
    let project_dir = if name.is_some() {
        let dir = std::env::current_dir()?.join(&project_name);
        std::fs::create_dir_all(&dir)?;
        dir
    } else {
        std::env::current_dir()?
    };

    // Create src directory
    let src_dir = project_dir.join("src");
    std::fs::create_dir_all(&src_dir)?;

    // Write astra.toml
    let manifest = format!(
        r#"[package]
name = "{name}"
version = "0.1.0"
description = ""
authors = []
license = "MIT"

[build]
target = "interpreter"

[lint]
level = "warn"
"#,
        name = project_name
    );
    std::fs::write(project_dir.join("astra.toml"), manifest)?;

    // Write main source file
    if lib {
        let lib_source = format!(
            r#"module {name}

## A library module for {name}.

public fn greet(who: Text) -> Text {{
  "Hello, ${{who}}!"
}}

test "greet works" {{
  assert_eq(greet("world"), "Hello, world!")
}}
"#,
            name = project_name
        );
        std::fs::write(src_dir.join("lib.astra"), lib_source)?;
    } else {
        let main_source = format!(
            r#"module {name}

fn main() effects(Console) {{
  println("Hello from {name}!")
}}

test "hello works" {{
  assert true
}}
"#,
            name = project_name
        );
        std::fs::write(src_dir.join("main.astra"), main_source)?;
    }

    // Write .gitignore
    let gitignore = "# Astra build artifacts\n/build/\n/.astra-cache/\n";
    std::fs::write(project_dir.join(".gitignore"), gitignore)?;

    // Write .claude/CLAUDE.md for AI agent onboarding
    let claude_dir = project_dir.join(".claude");
    std::fs::create_dir_all(&claude_dir)?;
    let claude_md = generate_claude_md(&project_name, lib);
    std::fs::write(claude_dir.join("CLAUDE.md"), claude_md)?;

    if name.is_some() {
        println!("Created new Astra project '{}'", project_name);
        println!("  cd {}", project_name);
    } else {
        println!("Initialized Astra project '{}'", project_name);
    }

    if lib {
        println!("  astra test          # Run tests");
        println!("  astra check         # Type check");
    } else {
        println!("  astra run src/main.astra   # Run the program");
        println!("  astra test                 # Run tests");
        println!("  astra check                # Type check");
    }

    Ok(())
}

pub(super) fn generate_claude_md(project_name: &str, is_lib: bool) -> String {
    let run_hint = if is_lib {
        ""
    } else {
        "astra run src/main.astra      # Run the program\n"
    };

    include_str!("claude_md_template.md")
        .replace("{{PROJECT_NAME}}", project_name)
        .replace("{{RUN_HINT}}", run_hint)
}
