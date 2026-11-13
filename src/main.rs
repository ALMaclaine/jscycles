//! jscycles - find circular imports in JavaScript and TypeScript projects.
//!
//! Reports file-level and workspace-package cycles in local checkouts.

use std::io::{self, IsTerminal as _, Read as _, Write as _};
use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Arg, ArgAction, Command};

use jscycles::output::CycleFilter;
use jscycles::scan::{Scan, ScanReport, TsConfigSource};
use jscycles::{Config, HumanFormatter, JscyclesError};

/// Build the CLI command.
fn build_cli() -> Command {
    Command::new("jscycles")
        .version(env!("CARGO_PKG_VERSION"))
        .about("Find circular imports in JavaScript and TypeScript projects")
        .arg(
            Arg::new("paths")
                .help("Paths to check (if empty, uses config scan paths)")
                .action(ArgAction::Append)
                .value_parser(clap::value_parser!(PathBuf)),
        )
        .arg(
            Arg::new("only")
                .long("only")
                .help("Only check packages matching glob pattern (can be repeated)")
                .action(ArgAction::Append),
        )
        .arg(
            Arg::new("exclude")
                .long("exclude")
                .help("Exclude packages matching glob pattern (can be repeated)")
                .action(ArgAction::Append),
        )
        .arg(
            Arg::new("extensions")
                .long("extensions")
                .help("File extensions to analyze")
                .default_value("ts,tsx,mts,cts,js,jsx,mjs,cjs")
                .value_parser(|value: &str| {
                    if !value.is_empty() && value.bytes().all(|byte| byte.is_ascii_alphanumeric()) { Ok(value.to_owned()) }
                    else { Err("extension must be a nonempty alphanumeric suffix without a leading dot") }
                })
                .value_delimiter(','),
        )
        .arg(
            Arg::new("tsconfig")
                .long("tsconfig")
                .help("Path to tsconfig.json")
                .value_parser(clap::value_parser!(PathBuf)),
        )
        .arg(
            Arg::new("no-tsconfig")
                .long("no-tsconfig")
                .help("Skip tsconfig.json auto-detection")
                .conflicts_with("tsconfig")
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new("skip-type-imports")
                .long("skip-type-imports")
                .help("Ignore type-only imports (import type, export type, declaration files)")
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new("json")
                .long("json")
                .help("Output as JSON")
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new("stdin")
                .long("stdin")
                .help("Read paths from stdin")
                .conflicts_with("paths")
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new("config")
                .short('c')
                .long("config")
                .help("Path to config file")
                .default_value("jscycles.yaml")
                .value_parser(clap::value_parser!(PathBuf)),
        )
        .arg(
            Arg::new("quiet")
                .short('q')
                .long("quiet")
                .help("Only output if cycles found")
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new("inner")
                .long("inner")
                .help("Show only file-level (inner) cycles")
                .action(ArgAction::SetTrue)
                .conflicts_with("outer"),
        )
        .arg(
            Arg::new("outer")
                .long("outer")
                .help("Show only package-level (outer) cycles")
                .action(ArgAction::SetTrue)
                .conflicts_with("inner"),
        )
        .arg(
            Arg::new("verbose")
                .short('v')
                .long("verbose")
                .help("Show all packages (including clean) and file-level details")
                .action(ArgAction::SetTrue),
        )
}

/// Selected-input context that preserves the parser or read error as its cause.
#[derive(Debug, thiserror::Error)]
#[error("{label} {}: {source}", path.display())]
struct InputContext {
    label: &'static str,
    path: PathBuf,
    #[source]
    source: Box<dyn std::error::Error + Send + Sync>,
}

/// Load the selected configuration, preserving its path and typed cause.
fn load_config(matches: &clap::ArgMatches) -> Result<Config, JscyclesError> {
    let default_config = PathBuf::from("jscycles.yaml");
    let path = matches
        .get_one::<PathBuf>("config")
        .unwrap_or(&default_config);
    let explicit = matches.value_source("config") == Some(clap::parser::ValueSource::CommandLine);
    let config = if explicit {
        Config::load(path)
    } else {
        Config::try_load_or_default(path)
    };
    config.map_err(|source| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            InputContext {
                label: "configuration",
                path: path.clone(),
                source: Box::new(source),
            },
        )
        .into()
    })
}

fn strings(matches: &clap::ArgMatches, id: &str) -> Vec<String> {
    matches
        .get_many::<String>(id)
        .map(|values| values.cloned().collect())
        .unwrap_or_default()
}

/// Translate the command line into a library [`Scan`].
fn scan_from(matches: &clap::ArgMatches) -> Result<Scan, JscyclesError> {
    let paths: Vec<PathBuf> = if matches.get_flag("stdin") {
        read_stdin_paths(io::stdin().lock())?
    } else {
        matches
            .get_many::<PathBuf>("paths")
            .map(|values| values.cloned().collect())
            .unwrap_or_default()
    };
    let scan = if paths.is_empty() {
        Scan::new(".")
    } else {
        Scan::packages(paths)
    };
    let mut scan = scan
        .config(load_config(matches)?)
        .only(strings(matches, "only"))
        .exclude(strings(matches, "exclude"))
        .skip_type_imports(matches.get_flag("skip-type-imports"))
        .verbose(matches.get_flag("verbose"))
        .filter(if matches.get_flag("inner") {
            CycleFilter::Inner
        } else if matches.get_flag("outer") {
            CycleFilter::Outer
        } else {
            CycleFilter::All
        })
        .tsconfig(if matches.get_flag("no-tsconfig") {
            TsConfigSource::Disabled
        } else if let Some(path) = matches.get_one::<PathBuf>("tsconfig") {
            TsConfigSource::File(path.clone())
        } else {
            TsConfigSource::Discover
        });
    // The default value matches the library default; only an explicit
    // selection overrides per-package settings.
    if matches.value_source("extensions") == Some(clap::parser::ValueSource::CommandLine) {
        scan = scan.extensions(strings(matches, "extensions"));
    }
    Ok(scan)
}

/// Automatic tsconfig discovery tolerates bad metadata (R10), but never
/// silently: the aliases it would have supplied can hide cycles.
fn warn_rejected_tsconfigs(report: &ScanReport) {
    let mut stderr = io::stderr().lock();
    for (path, error) in report.ignored_tsconfigs() {
        let first_line = error.lines().next().unwrap_or_default();
        // Debug-quote both so untrusted controls cannot forge terminal output.
        let shown = path.display().to_string();
        // A warning that cannot be written does not change the scan result.
        let _ = writeln!(
            stderr,
            "warning: ignored tsconfig {shown:?}: {first_line:?}; its path aliases were not applied (fix it, pass --tsconfig to require it, or --no-tsconfig)"
        );
    }
}

/// Run the cycle detection with one shared budget through discovery and publication.
#[cfg_attr(feature = "profiling", hotpath::measure)]
fn run(matches: &clap::ArgMatches) -> Result<bool, JscyclesError> {
    let report = scan_from(matches)?.run()?;
    warn_rejected_tsconfigs(&report);
    let has_cycles = report.has_cycles();
    if matches.get_flag("quiet") && !has_cycles {
        return Ok(false);
    }
    let bytes = if matches.get_flag("json") {
        report.to_json()?
    } else {
        report.to_human(io::stdout().is_terminal())?
    };
    let mut stdout = io::stdout().lock();
    stdout.write_all(&bytes)?;
    stdout.flush()?;
    Ok(has_cycles)
}

/// Read a bounded UTF-8 path list without discarding a partial read failure.
fn read_stdin_paths(reader: impl io::Read) -> io::Result<Vec<PathBuf>> {
    const MAX_BYTES: u64 = 1024 * 1024;
    const MAX_PATHS: usize = 10_000;
    let mut text = String::new();
    let _read_bytes = reader.take(MAX_BYTES + 1).read_to_string(&mut text)?;
    if u64::try_from(text.len()).unwrap_or(u64::MAX) > MAX_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "stdin exceeds 1 MiB",
        ));
    }
    // PowerShell and Windows editors may prefix a byte-order mark.
    let text = text.strip_prefix('\u{feff}').unwrap_or(&text);
    let paths: Vec<_> = text
        .lines()
        .filter(|line| !line.is_empty())
        .take(MAX_PATHS + 1)
        .map(PathBuf::from)
        .collect();
    if paths.is_empty() || paths.len() > MAX_PATHS {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "stdin requires 1 to 10000 paths",
        ));
    }
    Ok(paths)
}

/// Successful parsing keeps the original native paths. On an error, reparse
/// visible argument labels so Clap can retain its layout and styles without
/// inserting raw controls into diagnostics or generated suggestions.
fn cli_matches(arguments: &[std::ffi::OsString]) -> Result<clap::ArgMatches, clap::Error> {
    // Usage and errors name `jscycles` on every platform, not `jscycles.exe`
    // or whatever path launched it.
    let cli = build_cli().bin_name("jscycles");
    cli.clone()
        .try_get_matches_from(arguments)
        .map_err(|original| {
            let visible = arguments
                .iter()
                .map(|value| HumanFormatter::escape_label(&value.to_string_lossy()));
            match cli.try_get_matches_from(visible) {
                Err(error) => error,
                Ok(_) => clap::Error::raw(
                    original.kind(),
                    HumanFormatter::escape_label(&original.to_string()),
                ),
            }
        })
}

fn main() -> ExitCode {
    #[cfg(feature = "profiling")]
    let _profile = std::env::var_os("HOTPATH_OUTPUT_PATH").map(|path| {
        hotpath::HotpathGuardBuilder::new("jscycles")
            .format(hotpath::Format::JsonPretty)
            .output_path(path)
            .functions_limit(100)
            .build()
    });
    let matches = match cli_matches(&std::env::args_os().collect::<Vec<_>>()) {
        Ok(parsed) => parsed,
        Err(error) => {
            let informational = matches!(
                error.kind(),
                clap::error::ErrorKind::DisplayHelp | clap::error::ErrorKind::DisplayVersion
            );
            let printed = error.print().and_then(|()| {
                if informational {
                    io::stdout().flush()
                } else {
                    io::stderr().flush()
                }
            });
            if let Err(failure) = printed {
                drop(writeln!(
                    io::stderr(),
                    "Error writing command output: {}",
                    failure.to_string().escape_debug()
                ));
                return ExitCode::from(2);
            }
            return if informational {
                ExitCode::SUCCESS
            } else {
                ExitCode::from(2)
            };
        },
    };

    match run(&matches) {
        Ok(has_cycles) => {
            if has_cycles {
                ExitCode::from(1)
            } else {
                ExitCode::SUCCESS
            }
        },
        Err(err) => {
            drop(writeln!(
                io::stderr(),
                "Error: {}",
                err.to_string().escape_debug()
            ));
            ExitCode::from(2)
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stdin_read_error_after_a_valid_path_is_propagated() {
        struct FailedRead;
        impl io::Read for FailedRead {
            fn read(&mut self, _buffer: &mut [u8]) -> io::Result<usize> {
                Err(io::Error::other("injected read failure"))
            }
        }
        let reader = io::Cursor::new(b".\n").chain(FailedRead);
        assert!(
            read_stdin_paths(reader).is_err(),
            "partial input must not become success"
        );
    }

    #[test]
    fn stdin_byte_and_path_limits_are_checked() {
        let exact = vec![b'x'; 1024 * 1024];
        assert_eq!(
            read_stdin_paths(exact.as_slice())
                .expect("exact byte limit")
                .len(),
            1,
            "one path at the byte limit is accepted"
        );
        assert!(
            read_stdin_paths(vec![b'x'; 1024 * 1024 + 1].as_slice()).is_err(),
            "first excessive byte is rejected"
        );
        assert_eq!(
            read_stdin_paths(".\n".repeat(10_000).as_bytes())
                .expect("exact path limit")
                .len(),
            10_000,
            "exact path limit is accepted"
        );
        assert!(
            read_stdin_paths(".\n".repeat(10_001).as_bytes()).is_err(),
            "first excessive path is rejected"
        );
        assert!(
            read_stdin_paths(b"".as_slice()).is_err(),
            "empty stdin is an error"
        );
    }
}
