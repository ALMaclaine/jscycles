//! Run a complete scan from Rust: the same analysis the `jscycles` command
//! performs, returning the results instead of printing them.
//!
//! ```no_run
//! use jscycles::scan::Scan;
//!
//! # fn main() -> Result<(), jscycles::JscyclesError> {
//! let report = Scan::new("path/to/repo").run()?;
//! for package in &report.results().file_results {
//!     for cycle in &package.cycles {
//!         println!("{}: {} files", package.name, cycle.path.len() - 1);
//!     }
//! }
//! for cycle in &report.results().package_cycles {
//!     println!("packages: {}", cycle.packages.join(" -> "));
//! }
//! if report.has_cycles() {
//!     std::process::exit(1);
//! }
//! # Ok(())
//! # }
//! ```
//!
//! A [`Scan`] holds the same choices as the command-line options; a
//! [`ScanReport`] holds the results, and renders them as the command's JSON or
//! human output. Errors are the conditions the command reports with exit
//! status 2: a scan that returns `Ok` built the whole selected graph.

use std::collections::HashMap;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use rayon::prelude::*;

use crate::budget::{AnalysisBudget, BoundedReport, Resource};
use crate::cycles::{CycleDetector, PackageCycleDetector};
use crate::imports::{ExtractionMode, Import, ImportExtractor};
use crate::output::{
    CycleFilter, HumanFormatter, JsonFormatter, PackageResult, UnifiedOutputFormatter as _,
    UnifiedResults,
};
use crate::{
    Config, Discovered, DiscoveryError, JscyclesError, Package, PackageDiscovery, TsConfig,
    TsConfigCache, Workspace,
};

/// Where a scan's TypeScript path aliases come from.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[non_exhaustive]
pub enum TsConfigSource {
    /// Find each package's nearest `tsconfig.json`, as the command does by
    /// default. A file that cannot be used is skipped and listed in
    /// [`ScanReport::ignored_tsconfigs`].
    #[default]
    Discover,
    /// Use this file for every package; a file that cannot be used is an
    /// error (`--tsconfig`).
    File(PathBuf),
    /// Apply no tsconfig aliases (`--no-tsconfig`).
    Disabled,
}

/// What to scan, from which directories.
#[derive(Debug, Clone)]
enum Selection {
    /// Discover packages under a root using the configuration's `scan` rules.
    Root(PathBuf),
    /// Check these package directories or `package.json` files.
    Packages(Vec<PathBuf>),
}

/// A configured scan, equivalent to one `jscycles` invocation.
///
/// Build one with [`Scan::new`] or [`Scan::packages`], adjust it with the
/// builder methods, then call [`Scan::run`]. Nothing is read until `run`.
#[derive(Debug, Clone)]
#[must_use]
pub struct Scan {
    selection: Selection,
    config: Config,
    only: Vec<String>,
    exclude: Vec<String>,
    extensions: Option<Vec<String>>,
    tsconfig: TsConfigSource,
    skip_type_imports: bool,
    filter: CycleFilter,
    verbose: bool,
}

impl Scan {
    /// Discover and check every package under `root`, like running
    /// `jscycles` with no paths from that directory.
    ///
    /// The scan uses [`Config::default`]. To apply the repository's
    /// `jscycles.yaml`, as the command does, load it and pass it to
    /// [`Scan::config`]:
    ///
    /// ```no_run
    /// use jscycles::{scan::Scan, Config};
    /// use std::path::Path;
    ///
    /// # fn main() -> Result<(), jscycles::JscyclesError> {
    /// let root = Path::new("path/to/repo");
    /// let config = Config::try_load_or_default(&root.join("jscycles.yaml"))?;
    /// let report = Scan::new(root).config(config).run()?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self::with_selection(Selection::Root(root.into()))
    }

    /// Check exactly these package directories or `package.json` files, like
    /// `jscycles <paths>...`. They must all belong to the same workspace, if
    /// any.
    pub fn packages<I, P>(paths: I) -> Self
    where
        I: IntoIterator<Item = P>,
        P: Into<PathBuf>,
    {
        Self::with_selection(Selection::Packages(
            paths.into_iter().map(Into::into).collect(),
        ))
    }

    fn with_selection(selection: Selection) -> Self {
        Self {
            selection,
            config: Config::default(),
            only: Vec::new(),
            exclude: Vec::new(),
            extensions: None,
            tsconfig: TsConfigSource::Discover,
            skip_type_imports: false,
            filter: CycleFilter::All,
            verbose: false,
        }
    }

    /// Use this configuration: scan rules, per-package settings and resource
    /// limits (`--config`).
    pub fn config(mut self, config: Config) -> Self {
        self.config = config;
        self
    }

    /// Check only packages whose name matches one of these globs (`--only`).
    pub fn only<I, S>(mut self, patterns: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.only = patterns.into_iter().map(Into::into).collect();
        self
    }

    /// Skip packages whose name matches one of these globs (`--exclude`).
    pub fn exclude<I, S>(mut self, patterns: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.exclude = patterns.into_iter().map(Into::into).collect();
        self
    }

    /// Analyze these file extensions, without leading dots, in every package,
    /// overriding per-package settings (`--extensions`).
    pub fn extensions<I, S>(mut self, extensions: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.extensions = Some(extensions.into_iter().map(Into::into).collect());
        self
    }

    /// Choose where TypeScript path aliases come from (`--tsconfig`,
    /// `--no-tsconfig`).
    pub fn tsconfig(mut self, source: TsConfigSource) -> Self {
        self.tsconfig = source;
        self
    }

    /// Ignore type-only imports in every package (`--skip-type-imports`).
    pub const fn skip_type_imports(mut self, skip: bool) -> Self {
        self.skip_type_imports = skip;
        self
    }

    /// Look for file cycles only ([`CycleFilter::Inner`], `--inner`), package
    /// cycles only ([`CycleFilter::Outer`], `--outer`), or both (the default).
    pub const fn filter(mut self, filter: CycleFilter) -> Self {
        self.filter = filter;
        self
    }

    /// Record the files behind each package cycle, in
    /// [`UnifiedResults::package_cycles_with_files`] instead of
    /// [`UnifiedResults::package_cycles`], and list clean packages in human
    /// output (`--verbose`).
    pub const fn verbose(mut self, verbose: bool) -> Self {
        self.verbose = verbose;
        self
    }

    /// Run the scan.
    ///
    /// # Errors
    ///
    /// Everything the command reports with exit status 2: an invalid
    /// configuration or glob, an unreadable path or source file, a syntax
    /// error, a `--tsconfig` file that cannot be used, paths in different
    /// workspaces, an exhausted resource limit, or a selection with no
    /// packages ([`DiscoveryError::NoPackages`]).
    #[cfg_attr(feature = "profiling", hotpath::measure)]
    pub fn run(&self) -> Result<ScanReport, JscyclesError> {
        self.config.validate()?;
        if let Some(bad) = self.extensions.iter().flatten().find(|extension| {
            extension.is_empty() || !extension.bytes().all(|byte| byte.is_ascii_alphanumeric())
        }) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "extension {bad:?} must be a nonempty alphanumeric suffix without a leading dot"
                ),
            )
            .into());
        }
        let budget = AnalysisBudget::new(self.config.limits)?;
        let discovery = PackageDiscovery::new_with_budget(
            &self.config,
            &self.only,
            &self.exclude,
            budget.clone(),
        )?;
        let operands = match &self.selection {
            Selection::Root(root) => std::slice::from_ref(root),
            Selection::Packages(paths) => paths.as_slice(),
        };
        let workspace = discover_workspace(operands, self.filter, &budget)?;
        let opts = self.processing_options(workspace, &budget)?;
        let discovered = match &self.selection {
            Selection::Root(root) => discovery.discover_with_boundaries(root)?,
            Selection::Packages(paths) => discovery.discover_explicit_with_boundaries(paths)?,
        };
        if discovered.packages.is_empty() {
            // An empty selection checked nothing; reporting it as clean would
            // let a CI job started in the wrong directory pass.
            let filtered = !(self.only.is_empty() && self.exclude.is_empty());
            return Err(DiscoveryError::NoPackages { filtered }.into());
        }
        let results = analyze_packages(&discovered, &opts, self.verbose)?;
        budget.check()?;
        Ok(ScanReport {
            results,
            filter: self.filter,
            verbose: self.verbose,
            ignored_tsconfigs: opts.tsconfig_cache.rejected(),
            budget,
        })
    }

    fn processing_options(
        &self,
        mut workspace: Option<Workspace>,
        budget: &AnalysisBudget,
    ) -> Result<ProcessOptions, JscyclesError> {
        let explicit_tsconfig = match &self.tsconfig {
            TsConfigSource::File(path) => Some(
                TsConfig::load_with_budget(path, budget)
                    .map(Arc::new)
                    .map_err(|source| {
                        input_error(io::ErrorKind::InvalidData, "tsconfig", path, source)
                    })?,
            ),
            TsConfigSource::Discover | TsConfigSource::Disabled => None,
        };
        let no_tsconfig = self.tsconfig == TsConfigSource::Disabled;
        if let Some(found) = &mut workspace
            && !no_tsconfig
        {
            found.load_path_aliases(explicit_tsconfig.as_deref(), budget)?;
        }
        Ok(ProcessOptions {
            explicit_tsconfig,
            no_tsconfig,
            skip_type_imports: self.skip_type_imports,
            extensions: self
                .extensions
                .clone()
                .unwrap_or_else(|| crate::DEFAULT_EXTENSIONS.map(str::to_owned).to_vec()),
            extensions_override: self.extensions.is_some(),
            workspace: workspace.map(Arc::new),
            tsconfig_cache: Arc::new(TsConfigCache::new().with_budget(budget.clone())),
            mode: match self.filter {
                CycleFilter::All => ProcessMode::Full,
                CycleFilter::Inner => ProcessMode::FilesOnly,
                CycleFilter::Outer => ProcessMode::WorkspaceOnly,
            },
            budget: budget.clone(),
        })
    }
}

/// The results of a successful [`Scan::run`].
#[derive(Debug, Clone)]
pub struct ScanReport {
    results: UnifiedResults,
    filter: CycleFilter,
    verbose: bool,
    ignored_tsconfigs: Vec<(PathBuf, String)>,
    budget: AnalysisBudget,
}

impl ScanReport {
    /// File cycles per package, and package cycles. Cycle paths start and
    /// end with the same node; file paths keep the spelling of the scanned
    /// paths.
    #[must_use]
    pub const fn results(&self) -> &UnifiedResults {
        &self.results
    }

    /// Take ownership of the results.
    #[must_use]
    pub fn into_results(self) -> UnifiedResults {
        self.results
    }

    /// Whether the scan found a cycle of the kinds it looked for; the
    /// command exits 1 exactly when this is true.
    #[must_use]
    pub fn has_cycles(&self) -> bool {
        match self.filter {
            CycleFilter::All => self.results.has_any_cycles(),
            CycleFilter::Inner => self.results.has_file_cycles(),
            CycleFilter::Outer => self.results.has_package_cycles(),
        }
    }

    /// The kinds of cycle the scan looked for.
    #[must_use]
    pub const fn filter(&self) -> CycleFilter {
        self.filter
    }

    /// Automatically discovered `tsconfig.json` files that could not be used,
    /// with the reason. Their path aliases were not applied, which can hide a
    /// cycle, so surface these rather than treat the result as fully clean.
    #[must_use]
    pub fn ignored_tsconfigs(&self) -> &[(PathBuf, String)] {
        &self.ignored_tsconfigs
    }

    /// The report as the command's `--json` output, byte for byte.
    ///
    /// # Errors
    ///
    /// The output-size limit was exceeded.
    pub fn to_json(&self) -> Result<Vec<u8>, JscyclesError> {
        let mut report = BoundedReport::new(self.budget.clone());
        JsonFormatter::new().format_unified(&self.results, self.filter, &mut report)?;
        Ok(report.finish()?)
    }

    /// The report as the command's human-readable output; `color` adds
    /// terminal colors.
    ///
    /// # Errors
    ///
    /// The output-size limit was exceeded.
    pub fn to_human(&self, color: bool) -> Result<Vec<u8>, JscyclesError> {
        let mut report = BoundedReport::new(self.budget.clone());
        HumanFormatter::new(color)
            .with_verbose(self.verbose)
            .format_unified(&self.results, self.filter, &mut report)?;
        Ok(report.finish()?)
    }
}

/// Options needed during package processing.
struct ProcessOptions {
    /// Preloaded explicit tsconfig.json, if provided.
    explicit_tsconfig: Option<Arc<TsConfig>>,
    /// Skip tsconfig.json auto-detection.
    no_tsconfig: bool,
    /// `--skip-type-imports`, overriding the configuration.
    skip_type_imports: bool,
    /// File extensions to analyze.
    extensions: Vec<String>,
    /// An explicit extension selection takes precedence over package settings.
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

fn input_error(
    kind: io::ErrorKind,
    label: &'static str,
    path: &Path,
    source: impl std::error::Error + Send + Sync + 'static,
) -> io::Error {
    io::Error::new(
        kind,
        InputContext {
            label,
            path: path.to_path_buf(),
            source: Box::new(source),
        },
    )
}

/// How much package processing a scan's filter requires.
#[derive(Clone, Copy, PartialEq, Eq)]
enum ProcessMode {
    /// Full import extraction, file cycles and package cycles.
    Full,
    /// `--inner`: file cycles only.
    FilesOnly,
    /// `--outer`: only package imports, for package cycles.
    WorkspaceOnly,
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
        path.canonicalize()
            .map_err(|source| input_error(source.kind(), "scan path", path, source))
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
    let names: Arc<HashMap<PathBuf, String>> = Arc::new(
        roots
            .iter()
            .cloned()
            .zip(packages.iter().map(|package| package.name.clone()))
            .collect(),
    );
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
            process_package_with_imports(
                package,
                opts,
                nested,
                Arc::clone(&owners),
                Arc::clone(&names),
            )
        })
        .collect::<Result<Vec<_>, JscyclesError>>()?;
    // Package edges come from imports into another package's files too, so
    // package cycles need no workspace.
    let groups = processed
        .iter()
        .map(|proc| (proc.result.name.as_str(), proc.imports.as_slice()));
    let (package_cycles, package_cycles_with_files) = if opts.mode == ProcessMode::FilesOnly {
        (Vec::new(), Vec::new())
    } else if verbose {
        (
            Vec::new(),
            PackageCycleDetector::try_detect_imports_with_files(groups, &opts.budget)?,
        )
    } else {
        (
            PackageCycleDetector::try_detect_imports(groups, &opts.budget)?,
            Vec::new(),
        )
    };
    Ok(UnifiedResults {
        file_results: processed.into_iter().map(|proc| proc.result).collect(),
        package_cycles,
        package_cycles_with_files,
    })
}

fn canonical_package_root(
    package: &Package,
    budget: &AnalysisBudget,
) -> Result<PathBuf, JscyclesError> {
    let root =
        std::fs::canonicalize(&package.path).map_err(|source| crate::ExtractError::IoError {
            path: package.path.clone(),
            source,
        })?;
    budget.consume(
        Resource::GraphBytes,
        u64::try_from(root.as_os_str().len()).unwrap_or(u64::MAX),
    )?;
    Ok(root)
}

/// Process a single package, returning both file cycles and imports.
#[cfg_attr(feature = "profiling", hotpath::measure)]
fn process_package_with_imports(
    package: &Package,
    opts: &ProcessOptions,
    nested: impl Iterator<Item = PathBuf>,
    owners: Arc<[PathBuf]>,
    names: Arc<HashMap<PathBuf, String>>,
) -> Result<ProcessedPackage, JscyclesError> {
    // `--outer` resolves file aliases too: an alias into another member is a
    // package edge.
    let tsconfig = if opts.no_tsconfig {
        None
    } else if let Some(config) = &opts.explicit_tsconfig {
        Some(Arc::clone(config))
    } else {
        opts.tsconfig_cache.discover_shared(&package.path)
    };

    let extensions = if opts.extensions_override {
        opts.extensions.clone()
    } else {
        package
            .config
            .extensions
            .clone()
            .unwrap_or_else(|| opts.extensions.clone())
    };

    let skip_type_imports =
        opts.skip_type_imports || package.config.skip_type_imports.unwrap_or(false);
    let mut extractor = ImportExtractor::new(extensions, None)
        .with_skip_type_imports(skip_type_imports)
        .with_shared_tsconfig(tsconfig)
        .with_shared_workspace(opts.workspace.as_ref().map(Arc::clone))
        .with_budget(opts.budget.clone())
        .with_nested_packages(nested)
        .with_owners(owners)
        .with_package_names(names)
        .with_mode(match opts.mode {
            ProcessMode::Full | ProcessMode::FilesOnly => ExtractionMode::All,
            ProcessMode::WorkspaceOnly => ExtractionMode::WorkspaceOnly,
        });
    if let Some(ignore) = &package.config.ignore {
        extractor = extractor.with_ignore_patterns(ignore.clone());
    }

    // Both output modes analyze actual imports. WorkspaceOnly keeps package
    // edges only and skips file-cycle search.
    let imports = extractor.extract(&package.path)?;

    let mut cycles = if opts.mode == ProcessMode::WorkspaceOnly {
        Vec::new()
    } else {
        CycleDetector::try_detect_imports(&imports, &opts.budget)?
    };

    // Keep paths in the caller's spelling while analysis uses canonical nodes.
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
