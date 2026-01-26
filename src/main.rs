//! Astra CLI - The Astra programming language toolchain

use astra::cli::Cli;
use std::process::ExitCode;

fn main() -> ExitCode {
    match Cli::run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("Error: {}", e);
            ExitCode::FAILURE
        }
    }
}
