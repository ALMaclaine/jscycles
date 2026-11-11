//! xtask - Build automation for agentic Rust development.
//!
//! This binary provides a single entry point for all CI/validation tasks.
//! Agents and developers should run `cargo xtask ci` to validate changes.
//!
//! # Available Commands
//!
//! - `ci` - Run fmt, check, clippy, test, doctest, deny, and doc
//! - `fmt` - Check code formatting
//! - `clippy` - Run clippy with all targets
//! - `test` - Run all tests
//! - `deny` - Check dependencies (licenses, security, duplicates)
//! - `doc` - Build documentation
//! - `install-tools` - Install pinned cargo tools or Miri
//! - `crap` - Reject major complexity/coverage findings
//! - `mutants` - Mutate and review every production Rust module
//! - `miri` - Interpret pure Rust boundary tests
//! - `profile` - Measure a checked workload with Hotpath

use std::env;
use std::process::ExitCode;
use xshell::{Shell, cmd};

const DENY_VERSION: &str = "0.19.8";
const COVERAGE_VERSION: &str = "0.9.1";

type Task = fn(&Shell) -> Result<(), ()>;
const TASKS: &[(&str, Task)] = &[
    ("ci", ci),
    ("fmt", fmt),
    ("check", check),
    ("fuzz-check", fuzz_check),
    ("clippy", clippy),
    ("test", test),
    ("doctest", doctest),
    ("deny", deny),
    ("doc", doc),
    ("coverage", coverage),
    ("crap", crap),
    ("mutants", mutants),
    ("miri", miri),
    ("profile", profile),
];

fn main() -> ExitCode {
    let args: Vec<String> = env::args().skip(1).collect();
    match execute(&args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(()) => ExitCode::FAILURE,
    }
}

fn valid_args(args: &[String]) -> bool {
    args.len() <= 1 || (args.first().is_some_and(|arg| arg == "install-tools") && args.len() == 2)
}

fn execute(args: &[String]) -> Result<(), ()> {
    if !valid_args(args) {
        eprintln!("xtask commands accept no extra arguments except install-tools <tool>");
        return Err(());
    }
    let sh = Shell::new().map_err(|err| eprintln!("Failed to create shell: {err}"))?;
    let task = args.first().map_or("help", String::as_str);
    dispatch(&sh, task, args.get(1).map(String::as_str))
}

fn dispatch(sh: &Shell, task: &str, selected: Option<&str>) -> Result<(), ()> {
    if task == "install-tools" {
        return install_tools(sh, selected);
    }
    if ["help", "-h", "--help"].contains(&task) {
        print_help();
        return Ok(());
    }
    if let Some((_, run)) = TASKS.iter().find(|(name, _)| *name == task) {
        return run(sh);
    }
    eprintln!("Unknown command: {task}");
    print_help();
    Err(())
}

fn print_help() {
    eprintln!(
        r"
xtask - Build automation for agentic Rust development

USAGE:
    cargo xtask <COMMAND>

COMMANDS:
    ci             Run fmt, check, clippy, test, doctest, deny, and doc
    fmt            Check code formatting
    check          Build-check the locked workspace and all target kinds
    clippy         Lint the locked workspace and all target kinds
    fuzz-check     Compile every fuzz target on stable (-D warnings)
    test           Run all tests
    doctest        Run library documentation examples
    deny           Check dependencies (licenses, security, duplicates)
    doc            Build documentation
    install-tools [deny|coverage|crap|mutants|miri]  Install the selected pinned tool (default: deny)
    coverage       Write advisory LCOV for workspace tests (no percentage gate)
    crap           Generate coverage and reject CRAP scores above 30
    mutants        Mutation-test every production Rust module in a copy
    miri           Interpret pure Rust graph, text, and quota boundaries
    profile        Run a checked workload through the Hotpath build
    help           Print this help message

EXAMPLES:
    cargo xtask ci           # Run full CI pipeline
    cargo xtask clippy       # Just run clippy
    cargo xtask install-tools # Install cargo-deny if missing
"
    );
}

/// Run the full CI pipeline.
fn ci(sh: &Shell) -> Result<(), ()> {
    eprintln!("\n=== Running full CI pipeline ===\n");

    let checks: &[Task] = &[fmt, check, fuzz_check, clippy, test, doctest, deny, doc];
    for check in checks {
        check(sh)?;
    }

    eprintln!("\n=== All checks passed ===\n");
    Ok(())
}

/// Check code formatting.
fn fmt(sh: &Shell) -> Result<(), ()> {
    eprintln!(">> cargo fmt --all --check");
    cmd!(sh, "cargo fmt --all --check")
        .run()
        .map_err(|err| eprintln!("fmt failed: {err}"))?;
    cmd!(sh, "cargo fmt --manifest-path fuzz/Cargo.toml --check")
        .run()
        .map_err(|err| eprintln!("fuzz formatting failed: {err}"))
}

/// Compile the fuzz targets on the stable toolchain so library or private
/// adapter changes that break them fail CI instead of the next manual campaign.
/// Running them still requires the pinned nightly and cargo-fuzz.
fn fuzz_check(sh: &Shell) -> Result<(), ()> {
    eprintln!(">> cargo check --manifest-path fuzz/Cargo.toml --locked --bins");
    let _deny_warnings = sh.push_env("RUSTFLAGS", "-D warnings");
    cmd!(
        sh,
        "cargo check --manifest-path fuzz/Cargo.toml --locked --bins"
    )
    .run()
    .map_err(|err| eprintln!("fuzz target check failed: {err}"))
}

/// Run clippy on all targets.
fn clippy(sh: &Shell) -> Result<(), ()> {
    eprintln!(">> cargo clippy --workspace --all-targets --all-features --locked");
    cmd!(
        sh,
        "cargo clippy --workspace --all-targets --all-features --locked -- -D warnings"
    )
    .run()
    .map_err(|err| eprintln!("clippy failed: {err}"))
}

/// Run all tests.
fn test(sh: &Shell) -> Result<(), ()> {
    eprintln!(">> cargo test --workspace --all-targets --all-features --locked");
    cmd!(
        sh,
        "cargo test --workspace --all-targets --all-features --locked"
    )
    .run()
    .map_err(|err| eprintln!("test failed: {err}"))
}

/// Check dependencies with cargo-deny.
fn deny(sh: &Shell) -> Result<(), ()> {
    eprintln!(">> cargo deny --workspace --all-features --locked check");

    let version = cmd!(sh, "cargo deny --version").read().map_err(|error| {
        eprintln!("cargo-deny unavailable: {error}; run cargo xtask install-tools");
    })?;
    if version.trim() != format!("cargo-deny {DENY_VERSION}") {
        eprintln!("Expected cargo-deny {DENY_VERSION}; run cargo xtask install-tools");
        return Err(());
    }

    cmd!(sh, "cargo deny --workspace --all-features --locked check")
        .run()
        .map_err(|err| eprintln!("deny failed: {err}"))?;
    let _scope = sh.push_dir("fuzz");
    cmd!(
        sh,
        "cargo deny --workspace --all-features --locked check --config ../deny.toml"
    )
    .run()
    .map_err(|err| eprintln!("fuzz dependency policy failed: {err}"))
}

/// Build documentation.
fn doc(sh: &Shell) -> Result<(), ()> {
    eprintln!(">> cargo doc --workspace --no-deps --all-features --locked");
    cmd!(
        sh,
        "cargo doc --workspace --no-deps --all-features --locked"
    )
    .run()
    .map_err(|err| eprintln!("doc failed: {err}"))
}

/// Install required cargo tools.
fn install_tools(sh: &Shell, selected: Option<&str>) -> Result<(), ()> {
    let tool = selected.unwrap_or("deny");
    if tool == "miri" {
        return install_miri(sh);
    }
    if tool == "coverage" {
        cmd!(sh, "rustup component add llvm-tools-preview")
            .run()
            .map_err(|error| eprintln!("LLVM tools installation failed: {error}"))?;
    }
    install_cargo_tool(sh, tool)
}

fn install_cargo_tool(sh: &Shell, tool: &str) -> Result<(), ()> {
    let pins = [
        ("deny", DENY_VERSION),
        ("llvm-cov", COVERAGE_VERSION),
        ("crap", "0.9.0"),
        ("mutants", "27.1.0"),
    ];
    let name = if tool == "coverage" { "llvm-cov" } else { tool };
    let (_, version) = pins.iter().find(|(pin, _)| *pin == name).ok_or_else(|| {
        eprintln!("Unknown tool: {tool}; select deny, coverage, crap, mutants, or miri");
    })?;
    let package = format!("cargo-{name}");
    cmd!(sh, "cargo install {package} --version {version} --locked")
        .run()
        .map_err(|error| eprintln!("install failed: {error}"))
}

fn install_miri(sh: &Shell) -> Result<(), ()> {
    cmd!(sh, "rustup toolchain install nightly-2026-10-19 --profile minimal --component miri --component rust-src")
        .run().map_err(|error| eprintln!("Miri installation failed: {error}"))?;
    cmd!(sh, "cargo +nightly-2026-10-19 miri setup")
        .run()
        .map_err(|error| eprintln!("Miri setup failed: {error}"))
}

/// Generate advisory line coverage; test/tool errors remain failures.
fn coverage(sh: &Shell) -> Result<(), ()> {
    let version = cmd!(sh, "cargo llvm-cov --version")
        .read()
        .map_err(|error| {
            eprintln!("coverage unavailable: {error}; run cargo xtask install-tools coverage");
        })?;
    if version.trim() != format!("cargo-llvm-cov {COVERAGE_VERSION}") {
        eprintln!(
            "Expected cargo-llvm-cov {COVERAGE_VERSION}; run cargo xtask install-tools coverage"
        );
        return Err(());
    }
    sh.create_dir("target/tooling/coverage")
        .map_err(|error| eprintln!("create coverage directory: {error}"))?;
    cmd!(sh, "cargo llvm-cov --workspace --all-targets --locked --lcov --output-path target/tooling/coverage/lcov.info")
        .run().map_err(|error| eprintln!("coverage failed: {error}"))
}

/// Check every workspace member, feature, and Cargo target kind.
fn check(sh: &Shell) -> Result<(), ()> {
    cmd!(
        sh,
        "cargo check --workspace --all-targets --all-features --locked"
    )
    .run()
    .map_err(|error| eprintln!("check failed: {error}"))
}

/// Execute public library examples separately from target tests.
fn doctest(sh: &Shell) -> Result<(), ()> {
    cmd!(sh, "cargo test --workspace --doc --all-features --locked")
        .run()
        .map_err(|error| eprintln!("doctest failed: {error}"))
}

/// Reject major complexity/coverage findings, including the maintenance runner.
fn crap(sh: &Shell) -> Result<(), ()> {
    require_cargo_tool(sh, "crap", "0.9.0")?;
    coverage(sh)?;
    cmd!(sh, "cargo crap --workspace --exclude fuzz/** --lcov target/tooling/coverage/lcov.info --threshold 30 --fail-above --format json --output target/tooling/coverage/crap.json")
        .run().map_err(|error| eprintln!("CRAP gate failed: {error}"))
}

/// Mutate owned Rust algorithms and input adapters in a copy; review exact outcomes.
fn mutants(sh: &Shell) -> Result<(), ()> {
    require_cargo_tool(sh, "mutants", "27.1.0")?;
    cmd!(sh, "python3 scripts/mutation.py")
        .run()
        .map_err(|error| eprintln!("mutation campaign failed: {error}"))
}

/// Interpret actual owned Rust code; native parser execution stays in native tests.
fn miri(sh: &Shell) -> Result<(), ()> {
    for filter in [
        "graph::tests",
        "syntax::",
        "tsconfig::selection_tests",
        "output::tests",
        "input::bounded_reader_tests",
        "imports::lexical_path_tests",
    ] {
        cmd!(
            sh,
            "cargo +nightly-2026-10-19 miri test --locked --lib {filter} -- --skip module_syntax::tests::"
        )
        .run()
        .map_err(|error| eprintln!("Miri failed: {error}"))?;
    }
    cmd!(
        sh,
        "cargo +nightly-2026-10-19 miri test --locked --test miri"
    )
    .run()
    .map_err(|error| eprintln!("Miri boundary tests failed: {error}"))
}

fn require_cargo_tool(sh: &Shell, tool: &str, expected: &str) -> Result<(), ()> {
    let actual = cmd!(sh, "cargo {tool} --version").read().map_err(|error| {
        eprintln!("cargo-{tool} unavailable: {error}; run cargo xtask install-tools {tool}");
    })?;
    if actual.trim() == format!("cargo-{tool} {expected}") {
        return Ok(());
    }
    eprintln!(
        "Expected cargo-{tool} {expected}, got {actual}; run cargo xtask install-tools {tool}"
    );
    Err(())
}

/// Build an instrumented CLI and exercise a checked reproducible workload.
fn profile(sh: &Shell) -> Result<(), ()> {
    cmd!(sh, "cargo build --release --features profiling --locked")
        .run()
        .map_err(|error| eprintln!("profiling build failed: {error}"))?;
    for (workload, arguments) in [
        ("fan-in", &[][..]),
        ("call-heavy", &[][..]),
        (
            "monorepo",
            &["--files", "1024", "--packages", "16", "--aliases", "512"][..],
        ),
        ("directory-index", &["--files", "1000"][..]),
    ] {
        profile_workload(sh, workload, arguments)?;
    }
    Ok(())
}

fn profile_workload(sh: &Shell, workload: &str, arguments: &[&str]) -> Result<(), ()> {
    sh.create_dir("target/tooling/hotpath")
        .map_err(|error| eprintln!("profile directory: {error}"))?;
    let report = sh
        .current_dir()
        .join(format!("target/tooling/hotpath/{workload}.json"));
    let output = format!("target/tooling/hotpath/{workload}-benchmark.json");
    let _report = sh.push_env("HOTPATH_OUTPUT_PATH", report);
    cmd!(sh, "python3 scripts/benchmark.py --workload {workload} {arguments...} --samples 3 --output {output}")
        .run()
        .map_err(|error| eprintln!("{workload} profiling workload failed: {error}"))
}
