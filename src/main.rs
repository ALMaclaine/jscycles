//! jscycles - Fast circular dependency detection for JS/TS.
//!
//! Reports file-level and workspace-package cycles in local checkouts.

use std::io::{self, IsTerminal as _, Read as _, Write as _};
use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::Arc;

use clap::{Arg, ArgAction, Command};
use rayon::prelude::*;

use jscycles::budget::{AnalysisBudget, BoundedReport, Resource};
use jscycles::cycles::PackageCycleDetector;
use jscycles::imports::{ExtractionMode, Import};
use jscycles::output::{CycleFilter, UnifiedOutputFormatter as _, UnifiedResults};
use jscycles::{
    Config, CycleDetector, Discovered, HumanFormatter, ImportExtractor, JscyclesError,
    JsonFormatter, Package, PackageDiscovery, PackageResult, TsConfig, TsConfigCache, Workspace,
};

/// Build the CLI command.
fn build_cli() -> Command {
    Command::new("jscycles")
        .version(env!("CARGO_PKG_VERSION"))
        .about("Fast circular dependency detection for JavaScript/TypeScript")
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
                .default_value("ts,tsx,js,jsx")
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

/// Options needed during package processing.
struct ProcessOptions {
    /// Preloaded explicit tsconfig.json, if provided.
    explicit_tsconfig: Option<Arc<TsConfig>>,
    /// Skip tsconfig.json auto-detection.
    no_tsconfig: bool,
    /// File extensions to analyze.
    extensions: Vec<String>,

    /// An explicit CLI extension selection takes precedence over package settings.
    extensions_override: bool,
    /// Workspace configuration for inter-package cycle detection.
    workspace: Option<Arc<Workspace>>,
    /// Shared tsconfig discovery/load cache.
    tsconfig_cache: Arc<TsConfigCache>,

    /// How much work to do while processing each package.
    mode: ProcessMode,
    budget: AnalysisBudget,
}

/// Result of processing a package (includes imports for package graph).
struct ProcessedPackage {
    /// File-level cycle detection result.
    result: PackageResult,
    /// All imports from this package (for building package graph).
    imports: Vec<Import>,
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

/// Controls how much package processing is required for a given CLI mode.
#[derive(Clone, Copy, PartialEq, Eq)]
enum ProcessMode {
    /// Full import extraction plus file-level cycle detection.
    Full,
    /// Only collect workspace-package imports for package-level cycles.
    WorkspaceOnly,
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

fn scan_paths(matches: &clap::ArgMatches) -> Result<(Vec<PathBuf>, bool), JscyclesError> {
    if matches.get_flag("stdin") {
        return Ok((read_stdin_paths(io::stdin().lock())?, true));
    }
    let paths: Vec<PathBuf> = matches
        .get_many::<PathBuf>("paths")
        .map(|values| values.cloned().collect())
        .unwrap_or_default();
    if paths.is_empty() {
        Ok((vec![PathBuf::from(".")], false))
    } else {
        Ok((paths, true))
    }
}

fn discover_workspace(
    paths: &[PathBuf],
    filter: CycleFilter,
    budget: &AnalysisBudget,
) -> Result<Option<Workspace>, JscyclesError> {
    if filter == CycleFilter::Inner {
        return Ok(None);
    }
    let canonical_scan_path = |path: &PathBuf| {
        path.canonicalize().map_err(|source| {
            io::Error::new(
                source.kind(),
                InputContext {
                    label: "scan path",
                    path: path.clone(),
                    source: Box::new(source),
                },
            )
        })
    };
    let root = match paths.first() {
        Some(path) => canonical_scan_path(path)?,
        None => PathBuf::from("."),
    };
    let workspace = Workspace::discover_without_aliases(&root, budget)?;
    for path in paths.iter().skip(1) {
        let canonical = canonical_scan_path(path)?;
        // Membership was expanded once for the first operand; later operands
        // only need to identify the same root.
        let candidate = Workspace::discover_root(&canonical, budget)?;
        if candidate.as_ref() != workspace.as_ref().map(|found| &found.root) {
            return Err(io::Error::new(io::ErrorKind::InvalidInput,
                "package analysis supports one workspace per invocation; scan different workspaces separately").into());
        }
    }
    Ok(workspace)
}

fn processing_options(
    matches: &clap::ArgMatches,
    mut workspace: Option<Workspace>,
    budget: &AnalysisBudget,
) -> Result<ProcessOptions, JscyclesError> {
    let no_tsconfig = matches.get_flag("no-tsconfig");
    let explicit_tsconfig = matches
        .get_one::<PathBuf>("tsconfig")
        .map(|path| {
            TsConfig::load_with_budget(path, budget)
                .map(Arc::new)
                .map_err(|source| {
                    io::Error::new(
                        io::ErrorKind::InvalidData,
                        InputContext {
                            label: "tsconfig",
                            path: path.clone(),
                            source: Box::new(source),
                        },
                    )
                })
        })
        .transpose()?;
    if let Some(found) = &mut workspace
        && !no_tsconfig
    {
        found.load_path_aliases(explicit_tsconfig.as_deref(), budget)?;
    }
    let extensions = matches.get_many::<String>("extensions").map_or_else(
        || ["ts", "tsx", "js", "jsx"].map(str::to_owned).to_vec(),
        |values| values.cloned().collect(),
    );
    Ok(ProcessOptions {
        explicit_tsconfig,
        no_tsconfig,
        extensions,
        extensions_override: matches.value_source("extensions")
            == Some(clap::parser::ValueSource::CommandLine),
        workspace: workspace.map(Arc::new),
        tsconfig_cache: Arc::new(TsConfigCache::new().with_budget(budget.clone())),
        mode: if matches.get_flag("outer") {
            ProcessMode::WorkspaceOnly
        } else {
            ProcessMode::Full
        },
        budget: budget.clone(),
    })
}

#[cfg_attr(feature = "profiling", hotpath::measure)]

fn analyze_packages(
    discovered: &Discovered,
    opts: &ProcessOptions,
    verbose: bool,
) -> Result<UnifiedResults, JscyclesError> {
    // Each file belongs to its nearest package: a package that contains another
    // package's directory, or a directory discovery excluded or filtered out,
    // leaves that subtree alone.
    let packages = discovered.packages.as_slice();
    let roots = packages
        .iter()
        .map(|package| canonical_package_root(package, &opts.budget))
        .collect::<Result<Vec<_>, _>>()?;
    let owners: Arc<[PathBuf]> = roots
        .iter()
        .chain(&discovered.boundaries)
        .cloned()
        .collect::<Vec<_>>()
        .into();
    let processed: Vec<ProcessedPackage> = packages
        .par_iter()
        .zip(&roots)
        .map(|(package, root)| {
            opts.budget.consume(
                Resource::Work,
                u64::try_from(roots.len()).unwrap_or(u64::MAX),
            )?;
            opts.budget.consume(
                Resource::Work,
                u64::try_from(discovered.boundaries.len()).unwrap_or(u64::MAX),
            )?;
            let nested = roots
                .iter()
                .chain(&discovered.boundaries)
                // The walk never prunes its own root, so `root` itself may stay.
                .filter(|other| other.starts_with(root))
                .cloned();
            process_package_with_imports(package, opts, nested, Arc::clone(&owners))
        })
        .collect::<Result<Vec<_>, JscyclesError>>()?;
    let (package_cycles, package_cycles_with_files) = if opts.workspace.is_some() {
        let groups = processed
            .iter()
            .map(|proc| (proc.result.name.as_str(), proc.imports.as_slice()));
        if verbose {
            (
                Vec::new(),
                PackageCycleDetector::try_detect_imports_with_files(groups, &opts.budget)?,
            )
        } else {
            (
                PackageCycleDetector::try_detect_imports(groups, &opts.budget)?,
                Vec::new(),
            )
        }
    } else {
        (Vec::new(), Vec::new())
    };
    Ok(UnifiedResults {
        file_results: processed.into_iter().map(|proc| proc.result).collect(),
        package_cycles,
        package_cycles_with_files,
    })
}

/// Automatic tsconfig discovery tolerates bad metadata (R10), but never
/// silently: the aliases it would have supplied can hide cycles.
fn warn_rejected_tsconfigs(cache: &TsConfigCache) {
    let mut stderr = io::stderr().lock();
    for (path, error) in cache.rejected() {
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

fn canonical_package_root(
    package: &Package,
    budget: &AnalysisBudget,
) -> Result<PathBuf, JscyclesError> {
    let root =
        std::fs::canonicalize(&package.path).map_err(|source| jscycles::ExtractError::IoError {
            path: package.path.clone(),
            source,
        })?;
    budget.consume(
        Resource::GraphBytes,
        u64::try_from(root.as_os_str().len()).unwrap_or(u64::MAX),
    )?;
    Ok(root)
}

fn publish_results(
    unified: &UnifiedResults,
    filter: CycleFilter,
    matches: &clap::ArgMatches,
    budget: &AnalysisBudget,
) -> Result<bool, JscyclesError> {
    budget.check()?;
    let has_cycles = match filter {
        CycleFilter::All => unified.has_any_cycles(),
        CycleFilter::Inner => unified.has_file_cycles(),
        CycleFilter::Outer => unified.has_package_cycles(),
    };
    if matches.get_flag("quiet") && !has_cycles {
        return Ok(false);
    }
    let mut report = BoundedReport::new(budget.clone());
    if matches.get_flag("json") {
        JsonFormatter::new().format_unified(unified, filter, &mut report)?;
    } else {
        HumanFormatter::new(io::stdout().is_terminal())
            .with_verbose(matches.get_flag("verbose"))
            .format_unified(unified, filter, &mut report)?;
    }
    let bytes = report.finish()?;
    let mut stdout = io::stdout().lock();
    stdout.write_all(&bytes)?;
    stdout.flush()?;
    Ok(has_cycles)
}

/// Run the cycle detection with one shared budget through discovery and publication.
#[cfg_attr(feature = "profiling", hotpath::measure)]
fn run(matches: &clap::ArgMatches) -> Result<bool, JscyclesError> {
    let config = load_config(matches)?;
    let budget = AnalysisBudget::new(config.limits)?;
    let only: Vec<String> = matches
        .get_many::<String>("only")
        .map(|values| values.cloned().collect())
        .unwrap_or_default();
    let exclude: Vec<String> = matches
        .get_many::<String>("exclude")
        .map(|values| values.cloned().collect())
        .unwrap_or_default();
    let discovery = PackageDiscovery::new_with_budget(&config, &only, &exclude, budget.clone())?;
    let filter = if matches.get_flag("inner") {
        CycleFilter::Inner
    } else if matches.get_flag("outer") {
        CycleFilter::Outer
    } else {
        CycleFilter::All
    };
    let (paths, explicit) = scan_paths(matches)?;
    let workspace = discover_workspace(&paths, filter, &budget)?;
    let opts = processing_options(matches, workspace, &budget)?;
    let discovered = if explicit {
        discovery.discover_explicit_with_boundaries(&paths)?
    } else {
        discovery.discover_with_boundaries(&PathBuf::from("."))?
    };
    let unified = analyze_packages(&discovered, &opts, matches.get_flag("verbose"))?;
    warn_rejected_tsconfigs(&opts.tsconfig_cache);
    publish_results(&unified, filter, matches, &budget)
}

/// Process a single package, returning both file cycles and imports.
#[cfg_attr(feature = "profiling", hotpath::measure)]
fn process_package_with_imports(
    package: &Package,
    opts: &ProcessOptions,
    nested: impl Iterator<Item = PathBuf>,
    owners: Arc<[PathBuf]>,
) -> Result<ProcessedPackage, JscyclesError> {
    // Determine tsconfig
    let tsconfig = if opts.mode == ProcessMode::WorkspaceOnly || opts.no_tsconfig {
        None
    } else if let Some(config) = &opts.explicit_tsconfig {
        Some(Arc::clone(config))
    } else {
        opts.tsconfig_cache.discover_shared(&package.path)
    };

    // Get extensions for this package
    let extensions = if opts.extensions_override {
        opts.extensions.clone()
    } else {
        package
            .config
            .extensions
            .clone()
            .unwrap_or_else(|| opts.extensions.clone())
    };

    // Create import extractor with workspace context
    let mut extractor = ImportExtractor::new(extensions, None)
        .with_shared_tsconfig(tsconfig)
        .with_shared_workspace(opts.workspace.as_ref().map(Arc::clone))
        .with_budget(opts.budget.clone())
        .with_nested_packages(nested)
        .with_owners(owners)
        .with_mode(match opts.mode {
            ProcessMode::Full => ExtractionMode::All,
            ProcessMode::WorkspaceOnly => ExtractionMode::WorkspaceOnly,
        });
    if let Some(ignore) = &package.config.ignore {
        extractor = extractor.with_ignore_patterns(ignore.clone());
    }

    // Both output modes analyze actual imports. WorkspaceOnly skips file resolution.
    let imports = extractor.extract(&package.path)?;

    let mut cycles = if opts.mode == ProcessMode::WorkspaceOnly {
        Vec::new()
    } else {
        CycleDetector::try_detect_imports(&imports, &opts.budget)?
    };

    // Keep CLI paths in the caller's spelling while analysis uses canonical nodes.
    let canonical_root = std::fs::canonicalize(&package.path)?;
    for cycle in &mut cycles {
        for path in &mut cycle.path {
            if let Ok(relative) = path.strip_prefix(&canonical_root) {
                opts.budget.consume(
                    Resource::CycleBytes,
                    u64::try_from(package.path.as_os_str().len()).unwrap_or(u64::MAX),
                )?;
                opts.budget.consume(
                    Resource::CycleBytes,
                    u64::try_from(relative.as_os_str().len()).unwrap_or(u64::MAX),
                )?;
                *path = package.path.join(relative);
            }
        }
    }

    let result = PackageResult {
        name: package.name.clone(),
        path: package.path.clone(),
        cycles,
    };

    Ok(ProcessedPackage { result, imports })
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
    let cli = build_cli();
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
