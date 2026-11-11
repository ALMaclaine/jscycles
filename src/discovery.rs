//! Package discovery for monorepo scanning.
//!
//! Handles finding packages by scanning directories for `package.json` files.

use std::cell::RefCell;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::DiscoveryError;
use crate::budget::{AnalysisBudget, Resource, units};
use crate::config::{Config, PackageConfig};
use crate::workspace::Workspace;

/// A discovered JavaScript/TypeScript package.
#[derive(Debug, Clone)]
pub struct Package {
    /// Package name from package.json "name" field.
    pub name: String,

    /// Directory containing package.json.
    pub path: PathBuf,

    /// Merged configuration for this package.
    pub config: PackageConfig,
}

/// Packages selected by discovery, plus directories no package should analyze.
///
/// Boundaries are canonical directories pruned by `scan.exclude` and the
/// directories of named packages removed by name filters. A package that
/// contains one of these directories skips it, so excluded trees are not
/// silently re-analyzed as part of an enclosing package.
#[derive(Debug, Clone, Default)]
#[non_exhaustive]
pub struct Discovered {
    /// Selected packages, as returned by the plain discovery methods.
    pub packages: Vec<Package>,
    /// Sorted, deduplicated canonical directories to leave out of every package.
    pub boundaries: Vec<PathBuf>,
}

/// Package.json structure (minimal).
#[derive(Debug, Deserialize)]
struct PackageJson {
    /// Package name.
    name: Option<String>,
}

/// Package discovery engine.
#[derive(Debug)]
pub struct PackageDiscovery {
    budget: AnalysisBudget,
    /// Recursive directory roots selected by scan.include for automatic scans.
    include_patterns: Vec<(glob::Pattern, bool)>,

    /// Patterns for directories to exclude from scanning.
    exclude_patterns: Vec<crate::input::PathPattern>,
    /// Absolute exclude patterns compare automatic (`.`-relative) walks
    /// against the cwd-joined spelling.
    cwd: Option<PathBuf>,

    /// Patterns for package names to include (--only).
    only_filters: Vec<glob::Pattern>,

    /// Patterns for package names to exclude (--exclude).
    exclude_filters: Vec<glob::Pattern>,

    /// Package-specific config overrides.
    package_configs: HashMap<String, PackageConfig>,

    /// Default extensions.
    default_extensions: Vec<String>,

    /// Default for leaving out type-only imports.
    default_skip_type_imports: bool,
}

impl PackageDiscovery {
    /// Create a new package discovery engine.
    ///
    /// # Errors
    ///
    /// Returns an error if any glob pattern is invalid, or a limit error if the
    /// configuration's own setup exceeds the default quotas.
    #[inline]
    pub fn new(
        config: &Config,
        only: &[String],
        exclude: &[String],
    ) -> Result<Self, DiscoveryError> {
        Self::new_with_budget(config, only, exclude, AnalysisBudget::new(config.limits)?)
    }

    /// Construct discovery sharing a whole-scan budget. The supplied budget selects limits.
    /// # Errors
    /// Returns invalid patterns or quota exhaustion.
    pub fn new_with_budget(
        config: &Config,
        only: &[String],
        exclude: &[String],
        budget: AnalysisBudget,
    ) -> Result<Self, DiscoveryError> {
        for value in config
            .scan
            .include
            .iter()
            .chain(&config.scan.exclude)
            .chain(only)
            .chain(exclude)
            .chain(config.packages.keys())
            .chain(&config.defaults.extensions)
        {
            budget.consume(Resource::Work, 1)?;
            budget.consume(Resource::GraphBytes, units(value.len()))?;
        }
        for package in config.packages.values() {
            for value in package.extensions.iter().chain(&package.ignore).flatten() {
                budget.consume(Resource::Work, 1)?;
                budget.consume(Resource::GraphBytes, units(value.len()))?;
            }
        }
        // Validate selectors even when no current package happens to match them.
        let include_patterns = config
            .scan
            .include
            .iter()
            .map(|pattern| {
                // Include roots are compared with root-relative directories, or
                // with the cwd-joined directory when absolute; `./libs/` means `libs`.
                glob::Pattern::new(&crate::input::pattern_spelling(pattern))
                    .map(|compiled| (compiled, Path::new(pattern).is_absolute()))
                    .map_err(|source| DiscoveryError::InvalidPattern {
                        pattern: pattern.clone(),
                        source,
                    })
            })
            .collect::<Result<Vec<_>, _>>()?;
        for pattern in config.packages.keys().chain(
            config
                .packages
                .values()
                .filter_map(|package| package.ignore.as_ref())
                .flatten(),
        ) {
            let _pattern =
                glob::Pattern::new(pattern).map_err(|source| DiscoveryError::InvalidPattern {
                    pattern: pattern.clone(),
                    source,
                })?;
        }
        let exclude_patterns = config
            .scan
            .exclude
            .iter()
            .map(|p| {
                crate::input::PathPattern::new(p).map_err(|source| DiscoveryError::InvalidPattern {
                    pattern: p.clone(),
                    source,
                })
            })
            .collect::<Result<Vec<_>, _>>()?;

        let only_filters = only
            .iter()
            .map(|p| {
                glob::Pattern::new(p).map_err(|source| DiscoveryError::InvalidPattern {
                    pattern: p.clone(),
                    source,
                })
            })
            .collect::<Result<Vec<_>, _>>()?;

        let exclude_filters = exclude
            .iter()
            .map(|p| {
                glob::Pattern::new(p).map_err(|source| DiscoveryError::InvalidPattern {
                    pattern: p.clone(),
                    source,
                })
            })
            .collect::<Result<Vec<_>, _>>()?;

        Ok(Self {
            budget,
            include_patterns,
            exclude_patterns,
            cwd: std::env::current_dir().ok(),
            only_filters,
            exclude_filters,
            package_configs: config.packages.clone(),
            default_extensions: config.defaults.extensions.clone(),
            default_skip_type_imports: config.defaults.skip_type_imports,
        })
    }

    /// Discover packages under scan.include's recursive roots, relative to root.
    ///
    /// # Errors
    ///
    /// Returns an error if directory reading or package.json parsing fails, a
    /// package name appears at two directories (`DuplicatePackage`), a package
    /// path is not UTF-8, or a quota is exhausted.
    #[inline]
    #[cfg_attr(feature = "profiling", hotpath::measure)]
    pub fn discover(&self, root: &Path) -> Result<Vec<Package>, DiscoveryError> {
        self.discover_with_boundaries(root)
            .map(|discovered| discovered.packages)
    }

    /// [`Self::discover`], also returning the directories enclosing packages
    /// must skip.
    ///
    /// # Errors
    ///
    /// As [`Self::discover`]; a boundary that cannot be canonicalized is a
    /// `ReadDir` error.
    pub fn discover_with_boundaries(&self, root: &Path) -> Result<Discovered, DiscoveryError> {
        let mut packages = Vec::new();
        let pruned = RefCell::new(Vec::new());
        let mut filtered = Vec::new();

        for walk_result in crate::input::walk(root, &self.budget, |relative, full| {
            self.prune(relative, full, &pruned)
        }) {
            let dir_entry = walk_result.map_err(|err| DiscoveryError::ReadDir {
                path: root.to_path_buf(),
                source: err,
            })?;

            if self.included(root, dir_entry.path())
                && let Some(package) = self.try_parse_package_entry(&dir_entry, &mut filtered)?
            {
                packages.push(package);
            }
        }

        filtered.extend(pruned.into_inner());
        Ok(Discovered {
            packages: self.finish_packages(packages)?,
            boundaries: self.finish_boundaries(filtered)?,
        })
    }

    /// Prune like [`Self::should_exclude_dir`], recording real directories
    /// that a configured pattern (not a built-in exclusion) removed.
    fn prune(&self, relative: &Path, full: &Path, pruned: &RefCell<Vec<PathBuf>>) -> bool {
        if !self.should_exclude_dir(relative, full) {
            return false;
        }
        if !crate::input::excluded_component(relative)
            && std::fs::symlink_metadata(full).is_ok_and(|metadata| metadata.is_dir())
        {
            pruned.borrow_mut().push(full.to_path_buf());
        }
        true
    }

    fn finish_boundaries(&self, paths: Vec<PathBuf>) -> Result<Vec<PathBuf>, DiscoveryError> {
        let mut boundaries = Vec::new();
        for path in paths {
            self.budget.consume(Resource::Work, 1)?;
            let canonical = std::fs::canonicalize(&path)
                .map_err(|source| DiscoveryError::ReadDir { path, source })?;
            self.budget
                .consume(Resource::GraphBytes, units(canonical.as_os_str().len()))?;
            boundaries.push(canonical);
        }
        crate::budget::sorting(boundaries.len(), &self.budget)?;
        boundaries.sort();
        boundaries.dedup();
        Ok(boundaries)
    }

    /// A configured directory includes its descendants; `.` includes the whole root.
    fn included(&self, root: &Path, path: &Path) -> bool {
        let Some(directory) = path
            .parent()
            .and_then(|parent| parent.strip_prefix(root).ok())
        else {
            return false;
        };
        directory.ancestors().any(|ancestor| {
            if self
                .budget
                .consume(Resource::Work, units(self.include_patterns.len()))
                .is_err()
            {
                return false;
            }
            let relative = if ancestor.as_os_str().is_empty() {
                Path::new(".")
            } else {
                ancestor
            };
            let options = glob::MatchOptions {
                require_literal_separator: true,
                ..glob::MatchOptions::default()
            };
            let portable = relative.to_string_lossy().replace('\\', "/");
            self.include_patterns.iter().any(|(pattern, is_absolute)| {
                if *is_absolute {
                    self.absolute_spelling(&root.join(ancestor))
                        .is_some_and(|spelled| {
                            let spelled = spelled.to_string_lossy().replace('\\', "/");
                            pattern.matches_with(&spelled, options)
                        })
                } else {
                    pattern.matches_with(&portable, options)
                }
            })
        })
    }

    /// Try to parse a package from a walkdir entry if it's a package.json.
    fn try_parse_package_entry(
        &self,
        entry: &walkdir::DirEntry,
        filtered: &mut Vec<PathBuf>,
    ) -> Result<Option<Package>, DiscoveryError> {
        if entry.file_type().is_dir() || entry.file_name() != "package.json" {
            return Ok(None);
        }
        let Some(package) = self.parse_package(entry.path())? else {
            return Ok(None);
        };
        if self.matches_filters(&package.name) {
            return Ok(Some(package));
        }
        filtered.push(package.path);
        Ok(None)
    }

    /// Discover packages recursively from explicit directories, bypassing scan.include.
    /// A package.json operand selects that manifest alone.
    ///
    /// # Errors
    ///
    /// Returns an error if an operand is unreadable or is neither a directory nor
    /// a package.json, a selected manifest has no `name` (`MissingPackageName`)
    /// or fails to parse, a name appears at two directories, a path is not
    /// UTF-8, or a quota is exhausted.
    #[inline]
    #[cfg_attr(feature = "profiling", hotpath::measure)]
    pub fn discover_explicit(&self, paths: &[PathBuf]) -> Result<Vec<Package>, DiscoveryError> {
        self.discover_explicit_with_boundaries(paths)
            .map(|discovered| discovered.packages)
    }

    /// [`Self::discover_explicit`], also returning the directories enclosing
    /// packages must skip.
    ///
    /// # Errors
    ///
    /// As [`Self::discover_explicit`]; a boundary that cannot be canonicalized
    /// is a `ReadDir` error.
    pub fn discover_explicit_with_boundaries(
        &self,
        paths: &[PathBuf],
    ) -> Result<Discovered, DiscoveryError> {
        let mut packages = Vec::new();
        let mut boundaries = Vec::new();

        for path in paths {
            self.discover_from_path(path, &mut packages, &mut boundaries)?;
        }

        Ok(Discovered {
            packages: self.finish_packages(packages)?,
            boundaries: self.finish_boundaries(boundaries)?,
        })
    }

    /// Canonical identity makes overlapping selections idempotent. A name at
    /// distinct directories is ambiguous even when no workspace file is present.
    #[cfg_attr(feature = "profiling", hotpath::measure)]
    fn finish_packages(&self, mut packages: Vec<Package>) -> Result<Vec<Package>, DiscoveryError> {
        crate::budget::sorting(packages.len(), &self.budget)?;
        packages.sort_by(|left, right| (&left.name, &left.path).cmp(&(&right.name, &right.path)));
        let mut unique: Vec<Package> = Vec::new();
        let mut previous_path = PathBuf::new();
        for package in packages {
            self.budget.consume(Resource::Work, 1)?;
            let canonical =
                std::fs::canonicalize(&package.path).map_err(|source| DiscoveryError::ReadDir {
                    path: package.path.clone(),
                    source,
                })?;
            if canonical.to_str().is_none() {
                return Err(DiscoveryError::ReadDir {
                    path: canonical,
                    source: std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "package paths must be representable as UTF-8",
                    ),
                });
            }
            self.budget
                .consume(Resource::GraphBytes, units(canonical.as_os_str().len()))?;
            if unique
                .last()
                .is_some_and(|prior| prior.name == package.name)
            {
                if previous_path != canonical {
                    return Err(DiscoveryError::DuplicatePackage {
                        name: package.name,
                        first: previous_path,
                        second: canonical,
                    });
                }
                continue;
            }
            previous_path = canonical;
            unique.push(package);
        }
        self.budget.check()?;
        Ok(unique)
    }

    /// Discover packages from an already-expanded workspace package map.
    ///
    /// This avoids recursively scanning the filesystem a second time when
    /// workspace discovery has already found the package roots. This legacy
    /// infallible selector is unmetered and does not consume or partially fail a
    /// shared budget. Use fallible filesystem discovery for bounded CLI input.
    #[inline]
    #[must_use]
    pub fn discover_workspace_packages(
        &self,
        workspace: &Workspace,
        scan_paths: &[PathBuf],
    ) -> Vec<Package> {
        let normalized_paths: Vec<_> = scan_paths
            .iter()
            .map(|path| Self::normalize_scan_path(path))
            .collect();

        let mut packages: Vec<_> = workspace
            .packages
            .iter()
            .filter(|(_, path)| Self::matches_scan_paths(path, &normalized_paths))
            .filter(|(name, _)| self.matches_filters_unmetered(name))
            .map(|(name, path)| self.build_package_unbounded(name.clone(), path.clone()))
            .collect();

        packages.sort_by(|left, right| left.name.cmp(&right.name));
        packages
    }

    /// Discover packages from a single path.
    fn discover_from_path(
        &self,
        path: &Path,
        packages: &mut Vec<Package>,
        boundaries: &mut Vec<PathBuf>,
    ) -> Result<(), DiscoveryError> {
        if path.file_name().is_some_and(|name| name == "package.json") {
            self.add_package_if_matches(path, packages, boundaries)?;
            return Ok(());
        }

        let metadata = std::fs::metadata(path).map_err(|source| DiscoveryError::ReadDir {
            path: path.to_path_buf(),
            source,
        })?;
        if !metadata.is_dir() {
            return Err(DiscoveryError::ReadDir {
                path: path.to_path_buf(),
                source: std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    "scan operands must be directories or package.json files",
                ),
            });
        }

        // Search for package.json in subdirectories
        self.scan_directory_for_packages(path, packages, boundaries)
    }

    /// Scan a directory for packages.
    fn scan_directory_for_packages(
        &self,
        path: &Path,
        packages: &mut Vec<Package>,
        boundaries: &mut Vec<PathBuf>,
    ) -> Result<(), DiscoveryError> {
        let pruned = RefCell::new(Vec::new());
        for entry in crate::input::walk(path, &self.budget, |relative, full| {
            self.prune(relative, full, &pruned)
        }) {
            let entry = entry.map_err(|error| DiscoveryError::ReadDir {
                path: path.to_path_buf(),
                source: error,
            })?;
            if let Some(package) = self.try_parse_package_entry(&entry, boundaries)? {
                packages.push(package);
            }
        }
        boundaries.extend(pruned.into_inner());
        Ok(())
    }

    /// Add a package from a package.json path if it matches filters.
    fn add_package_if_matches(
        &self,
        package_json_path: &Path,
        packages: &mut Vec<Package>,
        filtered: &mut Vec<PathBuf>,
    ) -> Result<(), DiscoveryError> {
        // An explicitly selected manifest must define a package; skipping it
        // would turn a mistaken operand into a clean "no packages" result.
        let package = self
            .parse_package(package_json_path)?
            .ok_or_else(|| DiscoveryError::MissingPackageName(package_json_path.to_path_buf()))?;
        if self.matches_filters(&package.name) {
            packages.push(package);
        } else {
            filtered.push(package.path);
        }
        Ok(())
    }

    /// Normalize a scan path for prefix matching against workspace package roots.
    fn normalize_scan_path(path: &Path) -> PathBuf {
        let base = if path.file_name().is_some_and(|name| name == "package.json") {
            path.parent()
                .filter(|parent| !parent.as_os_str().is_empty())
                .unwrap_or_else(|| Path::new("."))
        } else {
            path
        };

        let absolute = if base.is_absolute() {
            base.to_path_buf()
        } else {
            std::env::current_dir().map_or_else(|_| base.to_path_buf(), |cwd| cwd.join(base))
        };
        // Member roots are canonical (and verbatim on Windows); compare like
        // with like. This legacy infallible selector keeps the spelled path
        // when canonicalization fails, which can only select fewer members.
        std::fs::canonicalize(&absolute).unwrap_or(absolute)
    }

    /// Check whether a package path falls under any requested scan path.
    fn matches_scan_paths(package_path: &Path, scan_paths: &[PathBuf]) -> bool {
        scan_paths.is_empty()
            || scan_paths
                .iter()
                .any(|scan_path| package_path.starts_with(scan_path))
    }

    /// Check if a directory should be excluded from scanning.
    fn should_exclude_dir(&self, relative: &Path, full: &Path) -> bool {
        if self
            .budget
            .consume(Resource::Work, units(self.exclude_patterns.len()))
            .is_err()
        {
            return true;
        }
        if crate::input::excluded_component(relative) {
            return true;
        }
        // Relative patterns match from the walked root: the cwd for automatic
        // scans, the operand for explicit ones (whose root is never pruned).
        // Automatic scans walk from `.`, so an absolute pattern needs the
        // walked path joined onto the cwd; that is built only when needed.
        self.exclude_patterns.iter().any(|pattern| {
            if pattern.is_absolute() {
                // Operands may be spelled with `..`; compare the lexical path.
                self.absolute_spelling(full)
                    .is_some_and(|path| pattern.matches_path(&path))
            } else {
                pattern.matches_path(relative)
            }
        })
    }

    /// The walked path as a lexically normalized absolute path, joined onto
    /// the cwd when relative.
    fn absolute_spelling(&self, path: &Path) -> Option<PathBuf> {
        let joined = if path.is_absolute() {
            path.to_path_buf()
        } else {
            self.cwd.as_deref()?.join(path)
        };
        Some(crate::input::absolute_form(&joined))
    }

    /// Parse a package.json file and create a Package struct.
    fn parse_package(&self, package_json_path: &Path) -> Result<Option<Package>, DiscoveryError> {
        let contents = crate::input::read_text_budgeted(package_json_path, &self.budget).map_err(
            |source| DiscoveryError::ReadDir {
                path: package_json_path.to_path_buf(),
                source,
            },
        )?;

        let package_json: PackageJson =
            serde_json::from_str(&contents).map_err(|source| DiscoveryError::ParsePackageJson {
                path: package_json_path.to_path_buf(),
                source,
            })?;

        let Some(name) = package_json.name else {
            // Skip packages without a name
            return Ok(None);
        };

        // `Path::new("package.json").parent()` is `Some("")`, not `None`.
        let package_dir = package_json_path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .map_or_else(|| PathBuf::from("."), Path::to_path_buf);

        Ok(Some(self.build_package(name, package_dir)?))
    }

    /// Build a package with config defaults applied.
    fn build_package(&self, name: String, package_dir: PathBuf) -> Result<Package, DiscoveryError> {
        if package_dir.to_str().is_none() {
            return Err(DiscoveryError::ReadDir {
                path: package_dir,
                source: std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "package paths must be representable as UTF-8",
                ),
            });
        }
        self.budget
            .consume(Resource::Work, units(self.package_configs.len()))?;
        crate::budget::sorting(self.package_configs.len(), &self.budget)?;
        let selected = self.find_package_config(&name);
        self.budget
            .consume(Resource::GraphBytes, units(name.len()))?;
        self.budget.consume(
            Resource::GraphBytes,
            units(package_dir.as_os_str().as_encoded_bytes().len()),
        )?;
        let extensions = selected
            .and_then(|value| value.extensions.as_ref())
            .unwrap_or(&self.default_extensions);
        for value in extensions.iter().chain(
            selected
                .and_then(|value| value.ignore.as_ref())
                .into_iter()
                .flatten(),
        ) {
            self.budget
                .consume(Resource::GraphBytes, units(value.len()))?;
        }
        Ok(self.package_with_config(name, package_dir, selected))
    }

    fn build_package_unbounded(&self, name: String, package_dir: PathBuf) -> Package {
        let selected = self.find_package_config(&name);
        self.package_with_config(name, package_dir, selected)
    }

    fn package_with_config(
        &self,
        name: String,
        package_dir: PathBuf,
        selected: Option<&PackageConfig>,
    ) -> Package {
        let mut config = selected.cloned().unwrap_or_default();

        if config.extensions.is_none() {
            config.extensions = Some(self.default_extensions.clone());
        }
        if config.skip_type_imports.is_none() {
            config.skip_type_imports = Some(self.default_skip_type_imports);
        }

        Package {
            name,
            path: package_dir,
            config,
        }
    }

    /// Find the matching package config for a package name.
    fn find_package_config(&self, name: &str) -> Option<&PackageConfig> {
        let mut keys: Vec<_> = self.package_configs.keys().collect();
        keys.sort();
        for pattern in keys {
            if !Self::pattern_matches_name(pattern, name) {
                continue;
            }
            if let Some(cfg) = self.package_configs.get(pattern) {
                return Some(cfg);
            }
        }
        None
    }

    /// Check if a glob pattern matches a package name.
    fn pattern_matches_name(pattern: &str, name: &str) -> bool {
        glob::Pattern::new(pattern).is_ok_and(|p| p.matches(name))
    }

    /// Check if a package name matches the only/exclude filters.
    fn matches_filters(&self, name: &str) -> bool {
        if self
            .budget
            .consume(Resource::Work, units(self.only_filters.len()))
            .is_err()
        {
            return false;
        }
        if self
            .budget
            .consume(Resource::Work, units(self.exclude_filters.len()))
            .is_err()
        {
            return false;
        }
        self.matches_filters_unmetered(name)
    }

    fn matches_filters_unmetered(&self, name: &str) -> bool {
        // If --only patterns specified, package must match at least one
        if !self.only_filters.is_empty() {
            let matches_only = self.only_filters.iter().any(|p| p.matches(name));
            if !matches_only {
                return false;
            }
        }

        // If --exclude patterns specified, package must not match any
        for pattern in &self.exclude_filters {
            if pattern.matches(name) {
                return false;
            }
        }

        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    use crate::workspace::WorkspaceFormat;

    #[test]
    fn test_matches_filters_empty() {
        let config = Config::default();
        let discovery = PackageDiscovery::new(&config, &[], &[])
            .expect("test discovery should be created successfully");
        assert!(discovery.matches_filters("@myorg/pkg"));
    }

    #[test]
    fn test_matches_filters_only() {
        let config = Config::default();
        let discovery = PackageDiscovery::new(&config, &["@myorg/*".to_owned()], &[])
            .expect("test discovery should be created successfully");
        assert!(discovery.matches_filters("@myorg/pkg"));
        assert!(!discovery.matches_filters("@other/pkg"));
    }

    #[test]
    fn test_matches_filters_exclude() {
        let config = Config::default();
        let discovery = PackageDiscovery::new(&config, &[], &["*-legacy".to_owned()])
            .expect("test discovery should be created successfully");
        assert!(discovery.matches_filters("@myorg/pkg"));
        assert!(!discovery.matches_filters("@myorg/pkg-legacy"));
    }

    #[test]
    fn test_discover_workspace_packages_filters_by_scan_path() {
        let config = Config::default();
        let discovery = PackageDiscovery::new(&config, &[], &[])
            .expect("test discovery should be created successfully");

        // Real canonical directories: member roots are canonical, and a
        // spelled `/repo` is not absolute on Windows.
        let directory = tempfile::tempdir().expect("temporary directory");
        let root = std::fs::canonicalize(directory.path()).expect("canonical root");
        let app = root.join("apps").join("app");
        let lib = root.join("libs").join("lib");
        for member in [&app, &lib] {
            std::fs::create_dir_all(member).expect("member directory");
        }

        let mut packages = HashMap::new();
        let _ = packages.insert("@demo/app".to_owned(), app.clone());
        let _ = packages.insert("@demo/lib".to_owned(), lib);

        let workspace = Workspace {
            root: root.clone(),
            format: WorkspaceFormat::Pnpm,
            packages,
            aliases: HashMap::new(),
        };

        let discovered_packages =
            discovery.discover_workspace_packages(&workspace, &[root.join("apps")]);

        assert_eq!(
            discovered_packages.len(),
            1,
            "should only include packages under scan path"
        );
        assert_eq!(discovered_packages[0].name, "@demo/app");
        assert_eq!(discovered_packages[0].path, app);
    }

    #[test]
    fn test_should_exclude_next_build_output() {
        let config = Config::default();
        let discovery = PackageDiscovery::new(&config, &[], &[])
            .expect("test discovery should be created successfully");

        assert!(
            discovery.should_exclude_dir(
                Path::new("apps/auth/.next"),
                Path::new("/repo/apps/auth/.next")
            ),
            ".next should be excluded from package discovery"
        );
        assert!(
            !discovery.should_exclude_dir(Path::new("apps/auth"), Path::new("/build/apps/auth")),
            "an excluded name above the walk root is not an exclusion"
        );
    }
}
