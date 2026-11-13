//! Check a repository for circular imports, as an embedding application
//! would: `cargo run --example check -- path/to/repo`.
#![allow(
    clippy::print_stdout,
    clippy::print_stderr,
    reason = "an example command prints its report"
)]

use std::path::PathBuf;
use std::process::ExitCode;

use jscycles::{Config, Scan};

fn main() -> ExitCode {
    let root = std::env::args_os()
        .nth(1)
        .map_or_else(|| PathBuf::from("."), PathBuf::from);
    let report = Config::try_load_or_default(&root.join("jscycles.yaml"))
        .map_err(jscycles::JscyclesError::from)
        .and_then(|config| Scan::new(&root).config(config).run());
    let report = match report {
        Ok(report) => report,
        Err(error) => {
            eprintln!("error: {error}");
            return ExitCode::from(2);
        },
    };
    for (path, reason) in report.ignored_tsconfigs() {
        eprintln!("warning: ignored {}: {reason}", path.display());
    }
    for package in &report.results().file_results {
        for cycle in &package.cycles {
            let files: Vec<_> = cycle
                .path
                .iter()
                .map(|path| path.display().to_string())
                .collect();
            println!("{}: {}", package.name, files.join(" -> "));
        }
    }
    for cycle in &report.results().package_cycles {
        println!("packages: {}", cycle.packages.join(" -> "));
    }
    if report.has_cycles() {
        ExitCode::from(1)
    } else {
        ExitCode::SUCCESS
    }
}
