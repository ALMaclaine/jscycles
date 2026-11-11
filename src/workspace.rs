//! Workspace detection and parsing for monorepos.
//!
//! Supports npm/yarn workspaces, pnpm workspaces, and TypeScript project references.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::WorkspaceError;
use crate::budget::{AnalysisBudget, Resource, units};

/// Workspace configuration format.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkspaceFormat {
    /// npm/yarn/bun: package.json "workspaces" field.
    Npm,
    /// pnpm: pnpm-workspace.yaml file.
    Pnpm,
    /// TypeScript: tsconfig.json "references" field.
    TypeScript,
}

/// Whether discovery expands package membership or only identifies the root.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Membership {
    Expand,
    RootOnly,
}

/// A detected monorepo workspace.
#[derive(Debug, Clone)]
pub struct Workspace {
    /// Root directory of the workspace.
    pub root: PathBuf,

    /// Detected workspace format.
    pub format: WorkspaceFormat,

    /// Mapping from package name to package directory path.
    pub packages: HashMap<String, PathBuf>,

    /// Mapping from tsconfig path aliases to package names.
    /// e.g., `@ender/shared-utils-system-test` -> `@ender/shared-utils-test`
    pub aliases: HashMap<String, String>,
}

/// npm/yarn package.json with workspaces field.
#[derive(Debug, Deserialize)]
struct NpmPackageJson {
    /// Workspaces can be an array or object with packages field.
    workspaces: Option<WorkspacesField>,
}

/// Workspaces field can be array or object.
#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum WorkspacesField {
    /// Simple array of glob patterns.
    Array(Vec<String>),
    /// Object with packages array.
    Object { packages: Vec<String> },
}

/// pnpm-workspace.yaml structure.
#[derive(Debug, Deserialize)]
struct PnpmWorkspace {
    /// Package glob patterns.
    packages: Option<Vec<String>>,
}

/// tsconfig.json with references.
#[derive(Debug, Deserialize)]
struct TsConfigWithRefs {
    /// Project references.
    references: Option<Vec<TsReference>>,
}

/// TypeScript project reference.
#[derive(Debug, Deserialize)]
struct TsReference {
    /// Path to referenced project.
    path: String,
}

/// Minimal package.json for reading name.
#[derive(Debug, Deserialize)]
struct MinimalPackageJson {
    /// Package name.
    name: Option<String>,
}

impl Workspace {
    /// Discover workspace configuration starting from a directory.
    ///
    /// Searches upward for the workspace root, checking at each directory:
    /// 1. pnpm-workspace.yaml
    /// 2. package.json with workspaces field
    /// 3. tsconfig.json with references field
    ///
    /// A references project is provisional: a pnpm/npm workspace further up
    /// wins only when the project lies inside one of its member packages.
    ///
    /// # Errors
    ///
    /// Returns an error if workspace metadata cannot be read or parsed, a
    /// member glob or reference is invalid, or two members share a name.
    #[inline]
    pub fn discover(start: &Path) -> Result<Option<Self>, WorkspaceError> {
        Self::discover_with_budget(start, &AnalysisBudget::default())
    }

    /// Discover workspace metadata under a shared analysis budget.
    /// # Errors
    /// Reports read/parse failures and any quota exhaustion, including optional alias reads.
    pub fn discover_with_budget(
        start: &Path,
        budget: &AnalysisBudget,
    ) -> Result<Option<Self>, WorkspaceError> {
        let mut workspace = Self::discover_without_aliases(start, budget)?;
        if let Some(found) = &mut workspace {
            found.load_path_aliases(None, budget)?;
        }
        Ok(workspace)
    }

    /// Discover package membership/project references without automatically loading
    /// optional path aliases. Use this before applying an explicitly selected config.
    /// # Errors
    /// Returns workspace input errors or quota exhaustion.
    pub fn discover_without_aliases(
        start: &Path,
        budget: &AnalysisBudget,
    ) -> Result<Option<Self>, WorkspaceError> {
        let result = Self::discover_inner(start, budget, Membership::Expand);
        budget.check()?;
        result
    }

    /// Locate the workspace root that `discover_without_aliases` would select,
    /// reading only the detecting metadata and never expanding membership.
    /// Callers comparing many operands against one discovered workspace use
    /// this to avoid repeating the full glob walk per operand.
    /// # Errors
    /// Returns detecting-metadata input errors or quota exhaustion.
    pub fn discover_root(
        start: &Path,
        budget: &AnalysisBudget,
    ) -> Result<Option<PathBuf>, WorkspaceError> {
        let result = Self::discover_inner(start, budget, Membership::RootOnly)
            .map(|found| found.map(|workspace| workspace.root));
        budget.check()?;
        result
    }

    /// Replace alias mappings using a selected parsed config, or automatic root/
    /// base-file discovery for None. Ordinary automatic metadata failures remain
    /// best-effort. On quota failure the previous mappings remain intact.
    /// # Errors
    /// Returns any recorded shared-budget exhaustion.
    pub fn load_path_aliases(
        &mut self,
        selected: Option<&crate::TsConfig>,
        budget: &AnalysisBudget,
    ) -> Result<(), crate::budget::LimitError> {
        let aliases = match selected {
            Some(config) => Self::map_aliases(config, &self.packages, budget),
            None => Self::load_aliases(&self.root, &self.packages, budget),
        };
        budget.check()?;
        self.aliases = aliases;
        Ok(())
    }

    /// The absolute directory an upward search starts from: a file operand
    /// searches from its parent; anything else that is not a directory is an
    /// error.
    fn search_start(start: &Path, budget: &AnalysisBudget) -> Result<PathBuf, WorkspaceError> {
        let mut current = if start.is_absolute() {
            start.to_path_buf()
        } else {
            std::env::current_dir()
                .map_err(|err| WorkspaceError::Read {
                    path: start.to_path_buf(),
                    source: err,
                })?
                .join(start)
        };
        budget.consume(Resource::Work, 1)?;
        let metadata = std::fs::metadata(&current).map_err(|source| WorkspaceError::Read {
            path: current.clone(),
            source,
        })?;
        if metadata.is_file() {
            let _removed = current.pop();
        } else if !metadata.is_dir() {
            return Err(WorkspaceError::Read {
                path: current,
                source: std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    "workspace discovery requires a directory or regular file",
                ),
            });
        }
        Ok(current)
    }

    fn discover_inner(
        start: &Path,
        budget: &AnalysisBudget,
        membership: Membership,
    ) -> Result<Option<Self>, WorkspaceError> {
        let mut current = Self::search_start(start, budget)?;

        // Search upward. pnpm, then npm/yarn, then TypeScript `references` are
        // tried at each directory. A references project found first is only
        // provisional. The search continues only to the first package-manager
        // workspace above it, and that workspace wins only when one of its
        // member patterns selects a directory containing the project (a
        // member's own tsconfig must not capture the root). Otherwise, as
        // before this rule existed, the references project is the workspace.
        let mut provisional: Option<Self> = None;
        loop {
            budget.consume(Resource::Work, 1)?;
            if let Some(project) = provisional.take() {
                match Self::selects_member(&current, &project.root, budget) {
                    Ok(Some(true)) => {
                        // Expand the containing workspace normally.
                        let manager = match Self::try_pnpm(&current, budget, membership)? {
                            Some(found) => Some(found),
                            None => Self::try_npm(&current, budget, membership)?,
                        };
                        return Ok(manager.or(Some(project)));
                    },
                    Ok(None) => provisional = Some(project),
                    // Quotas always fail.
                    Err(WorkspaceError::Limit(error)) => return Err(error.into()),
                    // A workspace that does not select the project, or whose
                    // metadata is unreadable, is one this scan was never shown
                    // to belong to; keep the project, as before this rule.
                    Ok(Some(false)) | Err(_) => return Ok(Some(project)),
                }
            } else {
                let manager = match Self::try_pnpm(&current, budget, membership)? {
                    Some(found) => Some(found),
                    None => Self::try_npm(&current, budget, membership)?,
                };
                if manager.is_some() {
                    return Ok(manager);
                }
                provisional = Self::try_typescript(&current, budget, membership)?;
            }
            let Some(parent) = current.parent() else {
                break;
            };
            current = parent.to_path_buf();
        }

        Ok(provisional)
    }

    /// Package-manager member patterns declared at `dir`, without expansion:
    /// `None` when `dir` declares no pnpm/npm workspace.
    fn manager_patterns(
        dir: &Path,
        budget: &AnalysisBudget,
    ) -> Result<Option<Vec<String>>, WorkspaceError> {
        let yaml_path = dir.join("pnpm-workspace.yaml");
        if Self::optional_metadata(&yaml_path)?.is_some() {
            let contents =
                crate::input::read_text_budgeted(&yaml_path, budget).map_err(|source| {
                    WorkspaceError::Read {
                        path: yaml_path.clone(),
                        source,
                    }
                })?;
            let config: PnpmWorkspace = serde_saphyr::from_str(&contents).map_err(|source| {
                WorkspaceError::Yaml(Box::new(crate::WorkspaceYamlError {
                    path: yaml_path,
                    source,
                }))
            })?;
            return Ok(Some(config.packages.unwrap_or_default()));
        }
        let pkg_path = dir.join("package.json");
        if Self::optional_metadata(&pkg_path)?.is_none() {
            return Ok(None);
        }
        let contents = crate::input::read_text_budgeted(&pkg_path, budget).map_err(|source| {
            WorkspaceError::Read {
                path: pkg_path.clone(),
                source,
            }
        })?;
        let pkg: NpmPackageJson =
            serde_json::from_str(&contents).map_err(|source| WorkspaceError::Json {
                path: pkg_path,
                source,
            })?;
        Ok(pkg.workspaces.map(|workspaces| match workspaces {
            WorkspacesField::Array(patterns) | WorkspacesField::Object { packages: patterns } => {
                patterns
            },
        }))
    }

    /// Whether the workspace declared at `dir` selects a member directory that
    /// contains `project` (the project itself or one of its ancestors below
    /// `dir`). Membership is decided by the declared patterns, so a member
    /// without a `name` still counts. `None` means `dir` is no workspace.
    fn selects_member(
        dir: &Path,
        project: &Path,
        budget: &AnalysisBudget,
    ) -> Result<Option<bool>, WorkspaceError> {
        let Some(patterns) = Self::manager_patterns(dir, budget)? else {
            return Ok(None);
        };
        let Ok(relative) = project.strip_prefix(dir) else {
            return Ok(Some(false));
        };
        let options = glob::MatchOptions {
            require_literal_separator: true,
            ..glob::MatchOptions::default()
        };
        let mut includes = Vec::new();
        let mut excludes = Vec::new();
        for pattern in &patterns {
            budget.consume(Resource::Work, 1)?;
            let negated = pattern.strip_prefix('!');
            let mut stripped = negated.unwrap_or(pattern);
            while let Some(rest) = stripped.strip_prefix("./") {
                stripped = rest.trim_start_matches('/');
            }
            let compiled = Self::manifest_pattern(stripped)?;
            if negated.is_some() {
                excludes.push(compiled);
            } else {
                includes.push(compiled);
            }
        }
        for candidate in relative.ancestors() {
            if candidate.as_os_str().is_empty() {
                break;
            }
            budget.consume(Resource::Work, units(patterns.len()))?;
            if crate::input::excluded_component(candidate) {
                continue;
            }
            let manifest = format!(
                "{}/package.json",
                candidate.to_string_lossy().replace('\\', "/")
            );
            if includes
                .iter()
                .any(|pattern| pattern.matches_with(&manifest, options))
                && !excludes
                    .iter()
                    .any(|pattern| pattern.matches_with(&manifest, options))
                && Self::optional_metadata(&dir.join(candidate).join("package.json"))?.is_some()
            {
                return Ok(Some(true));
            }
        }
        Ok(Some(false))
    }

    /// Try to parse pnpm-workspace.yaml.
    fn try_pnpm(
        dir: &Path,
        budget: &AnalysisBudget,
        membership: Membership,
    ) -> Result<Option<Self>, WorkspaceError> {
        let yaml_path = dir.join("pnpm-workspace.yaml");
        if Self::optional_metadata(&yaml_path)?.is_none() {
            return Ok(None);
        }

        let contents = crate::input::read_text_budgeted(&yaml_path, budget).map_err(|err| {
            WorkspaceError::Read {
                path: yaml_path.clone(),
                source: err,
            }
        })?;

        let config: PnpmWorkspace = serde_saphyr::from_str(&contents).map_err(|source| {
            WorkspaceError::Yaml(Box::new(crate::WorkspaceYamlError {
                path: yaml_path,
                source,
            }))
        })?;

        let patterns = config.packages.unwrap_or_default();
        let packages = match membership {
            Membership::Expand => Self::expand_patterns(dir, &patterns, budget)?,
            Membership::RootOnly => HashMap::new(),
        };
        let aliases = HashMap::new();

        Ok(Some(Self {
            root: dir.to_path_buf(),
            format: WorkspaceFormat::Pnpm,
            packages,
            aliases,
        }))
    }

    /// Try to parse npm/yarn workspaces from package.json.
    fn try_npm(
        dir: &Path,
        budget: &AnalysisBudget,
        membership: Membership,
    ) -> Result<Option<Self>, WorkspaceError> {
        let pkg_path = dir.join("package.json");
        if Self::optional_metadata(&pkg_path)?.is_none() {
            return Ok(None);
        }

        let contents = crate::input::read_text_budgeted(&pkg_path, budget).map_err(|err| {
            WorkspaceError::Read {
                path: pkg_path.clone(),
                source: err,
            }
        })?;

        let pkg: NpmPackageJson =
            serde_json::from_str(&contents).map_err(|source| WorkspaceError::Json {
                path: pkg_path,
                source,
            })?;

        let Some(workspaces) = pkg.workspaces else {
            return Ok(None);
        };

        let patterns = match workspaces {
            WorkspacesField::Array(arr) => arr,
            WorkspacesField::Object { packages } => packages,
        };

        let packages = match membership {
            Membership::Expand => Self::expand_patterns(dir, &patterns, budget)?,
            Membership::RootOnly => HashMap::new(),
        };
        let aliases = HashMap::new();

        Ok(Some(Self {
            root: dir.to_path_buf(),
            format: WorkspaceFormat::Npm,
            packages,
            aliases,
        }))
    }

    /// Try to parse TypeScript project references from tsconfig.json.
    fn try_typescript(
        dir: &Path,
        budget: &AnalysisBudget,
        membership: Membership,
    ) -> Result<Option<Self>, WorkspaceError> {
        let ts_path = dir.join("tsconfig.json");
        if Self::optional_metadata(&ts_path)?.is_none() {
            return Ok(None);
        }

        let contents = crate::input::read_text_budgeted(&ts_path, budget).map_err(|err| {
            WorkspaceError::Read {
                path: ts_path.clone(),
                source: err,
            }
        })?;

        // Strip comments from JSON (tsconfig allows them)
        let stripped =
            crate::syntax::jsonc(&contents).map_err(|message| WorkspaceError::Parse {
                path: ts_path.clone(),
                message: message.to_owned(),
            })?;

        let config: TsConfigWithRefs =
            serde_json::from_str(&stripped).map_err(|source| WorkspaceError::Json {
                path: ts_path,
                source,
            })?;

        let Some(refs) = config.references else {
            return Ok(None);
        };

        if refs.is_empty() {
            return Ok(None);
        }

        let paths: Vec<String> = refs.into_iter().map(|r| r.path).collect();
        let packages = match membership {
            Membership::Expand => Self::resolve_ts_references(dir, &paths, budget)?,
            Membership::RootOnly => HashMap::new(),
        };
        let aliases = HashMap::new();

        Ok(Some(Self {
            root: dir.to_path_buf(),
            format: WorkspaceFormat::TypeScript,
            packages,
            aliases,
        }))
    }

    /// Expand glob patterns to package name -> path mappings.
    fn expand_patterns(
        root: &Path,
        patterns: &[String],
        budget: &AnalysisBudget,
    ) -> Result<HashMap<String, PathBuf>, WorkspaceError> {
        let mut packages = HashMap::new();
        let mut include_patterns = Vec::new();
        let mut exclude_patterns = Vec::new();

        for pattern in patterns {
            budget.consume(Resource::Work, 1)?;
            budget.consume(Resource::GraphBytes, units(pattern.len()))?;
            let negated = pattern.strip_prefix('!');
            // Manifests are matched by root-relative paths, which never start
            // with `./`; package managers treat `./packages/*` as `packages/*`.
            let mut stripped = negated.unwrap_or(pattern);
            while let Some(rest) = stripped.strip_prefix("./") {
                stripped = rest.trim_start_matches('/');
            }
            let manifest_pattern = Self::manifest_pattern(stripped)?;
            if negated.is_some() {
                exclude_patterns.push(manifest_pattern);
            } else {
                include_patterns.push((stripped.to_owned(), manifest_pattern));
            }
        }

        for (pattern, manifest_pattern) in include_patterns {
            Self::expand_single_pattern(
                root,
                &pattern,
                &manifest_pattern,
                &exclude_patterns,
                &mut packages,
                budget,
            )?;
        }

        Ok(packages)
    }

    /// Expand a single glob pattern.
    fn expand_single_pattern(
        root: &Path,
        pattern: &str,
        manifest_pattern: &glob::Pattern,
        exclude_patterns: &[glob::Pattern],
        packages: &mut HashMap<String, PathBuf>,
        budget: &AnalysisBudget,
    ) -> Result<(), WorkspaceError> {
        let search_root = root.join(Self::pattern_base(pattern));
        let Some(metadata) = Self::optional_metadata(&search_root)? else {
            return Ok(());
        };
        if !metadata.is_dir() {
            return Err(WorkspaceError::Read {
                path: search_root,
                source: std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    "workspace pattern prefix must be a directory",
                ),
            });
        }

        for entry in crate::input::walk(&search_root, budget, |relative, _| {
            Self::is_in_excluded_dir(relative)
        }) {
            let entry = entry.map_err(|error| WorkspaceError::Read {
                path: search_root.clone(),
                source: error,
            })?;
            if entry.file_type().is_dir() || entry.file_name() != "package.json" {
                continue;
            }

            let Ok(relative_manifest_path) = entry.path().strip_prefix(root) else {
                continue;
            };
            let relative_manifest = relative_manifest_path.to_string_lossy().replace('\\', "/");
            budget.consume(Resource::Work, units(exclude_patterns.len()))?;

            let options = glob::MatchOptions {
                require_literal_separator: true,
                ..glob::MatchOptions::default()
            };
            if !manifest_pattern.matches_with(&relative_manifest, options)
                || exclude_patterns
                    .iter()
                    .any(|exclude| exclude.matches_with(&relative_manifest, options))
            {
                continue;
            }

            if let Some((name, path)) = Self::read_package_name(entry.path(), budget)? {
                Self::insert_package(packages, name, &path, budget)?;
            }
        }

        Ok(())
    }

    /// Build a glob pattern that matches a package manifest path.
    fn manifest_pattern(pattern: &str) -> Result<glob::Pattern, WorkspaceError> {
        let manifest_pattern = format!("{}/package.json", pattern.trim_end_matches('/'));
        glob::Pattern::new(&manifest_pattern).map_err(|source| WorkspaceError::InvalidPattern {
            pattern: pattern.to_owned(),
            source,
        })
    }

    /// Extract the non-glob prefix of a workspace pattern.
    fn pattern_base(pattern: &str) -> PathBuf {
        let mut base = PathBuf::new();

        for component in pattern.split('/') {
            if component.is_empty() || Self::has_glob_chars(component) {
                break;
            }
            base.push(component);
        }

        if base.as_os_str().is_empty() {
            PathBuf::from(".")
        } else {
            base
        }
    }

    /// Check whether a pattern component contains glob metacharacters.
    fn has_glob_chars(component: &str) -> bool {
        component.contains(['*', '?', '['])
    }

    /// Check if a path is inside a `node_modules` or dist directory.
    ///
    /// These directories contain external dependencies or build outputs,
    /// not workspace packages.
    fn is_in_excluded_dir(path: &Path) -> bool {
        crate::input::excluded_component(path)
    }

    /// Read package name from a package.json path.
    fn read_package_name(
        pkg_json: &Path,
        budget: &AnalysisBudget,
    ) -> Result<Option<(String, PathBuf)>, WorkspaceError> {
        let contents = crate::input::read_text_budgeted(pkg_json, budget).map_err(|err| {
            WorkspaceError::Read {
                path: pkg_json.to_path_buf(),
                source: err,
            }
        })?;

        let pkg: MinimalPackageJson =
            serde_json::from_str(&contents).map_err(|source| WorkspaceError::Json {
                path: pkg_json.to_path_buf(),
                source,
            })?;

        let Some(name) = pkg.name else {
            return Ok(None);
        };

        let path = pkg_json
            .parent()
            .map_or_else(|| PathBuf::from("."), Path::to_path_buf);

        budget.consume(Resource::GraphBytes, units(name.len()))?;
        budget.consume(
            Resource::GraphBytes,
            units(path.as_os_str().as_encoded_bytes().len()),
        )?;
        Ok(Some((name, path)))
    }

    /// Resolve TypeScript project references to package mappings.
    fn resolve_ts_references(
        root: &Path,
        paths: &[String],
        budget: &AnalysisBudget,
    ) -> Result<HashMap<String, PathBuf>, WorkspaceError> {
        let mut packages = HashMap::new();

        for ref_path in paths {
            budget.consume(Resource::Work, 1)?;
            let reference = root.join(ref_path);
            let metadata =
                std::fs::metadata(&reference).map_err(|source| WorkspaceError::Read {
                    path: reference.clone(),
                    source,
                })?;
            let dir = if metadata.is_dir() {
                reference
            } else if metadata.is_file() {
                reference.parent().unwrap_or(root).to_path_buf()
            } else {
                return Err(WorkspaceError::Read {
                    path: reference,
                    source: std::io::Error::new(
                        std::io::ErrorKind::InvalidInput,
                        "project reference must select a directory or regular config file",
                    ),
                });
            };
            let pkg_json = dir.join("package.json");

            if Self::optional_metadata(&pkg_json)?.is_some()
                && let Some((name, path)) = Self::read_package_name(&pkg_json, budget)?
            {
                Self::insert_package(&mut packages, name, &path, budget)?;
            }
        }

        Ok(packages)
    }

    fn optional_metadata(path: &Path) -> Result<Option<std::fs::Metadata>, WorkspaceError> {
        crate::input::optional_metadata(path).map_err(|source| WorkspaceError::Read {
            path: path.to_path_buf(),
            source,
        })
    }

    fn insert_package(
        packages: &mut HashMap<String, PathBuf>,
        name: String,
        path: &Path,
        budget: &AnalysisBudget,
    ) -> Result<(), WorkspaceError> {
        budget.consume(Resource::Work, 1)?;
        let canonical = std::fs::canonicalize(path).map_err(|source| WorkspaceError::Read {
            path: path.to_path_buf(),
            source,
        })?;
        budget.consume(Resource::GraphBytes, units(canonical.as_os_str().len()))?;
        if let Some(prior) = packages.get(&name)
            && prior != &canonical
        {
            return Err(WorkspaceError::Parse {
                path: path.join("package.json"),
                message: format!(
                    "duplicate workspace package name {name:?} at {} and {}",
                    prior.display(),
                    canonical.display()
                ),
            });
        }
        let _ = packages.insert(name, canonical);
        Ok(())
    }

    /// Reuse the TypeScript loader's origin/override/fallback semantics. A root
    /// config takes precedence over the conventional base-file fallback.
    fn load_aliases(
        root: &Path,
        packages: &HashMap<String, PathBuf>,
        budget: &AnalysisBudget,
    ) -> HashMap<String, String> {
        for selected in [root.join("tsconfig.json"), root.join("tsconfig.base.json")] {
            if budget.check().is_err() {
                break;
            }
            let Ok(config) = crate::TsConfig::load_with_budget(&selected, budget) else {
                continue;
            };
            if !config.paths.is_empty() {
                return Self::map_aliases(&config, packages, budget);
            }
        }
        HashMap::new()
    }

    #[cfg_attr(feature = "profiling", hotpath::measure)]
    fn map_aliases(
        config: &crate::TsConfig,
        packages: &HashMap<String, PathBuf>,
        budget: &AnalysisBudget,
    ) -> HashMap<String, String> {
        let mut package_paths = Vec::new();
        for (name, path) in packages {
            if budget.consume(Resource::Work, 1).is_err() {
                return HashMap::new();
            }
            let Ok(canonical) = std::fs::canonicalize(path) else {
                continue;
            };
            if budget
                .consume(Resource::GraphBytes, units(canonical.as_os_str().len()))
                .is_err()
            {
                return HashMap::new();
            }
            package_paths.push((canonical, name));
        }
        if crate::budget::sorting(package_paths.len(), budget).is_err() {
            return HashMap::new();
        }
        package_paths.sort_by(|left, right| {
            right
                .0
                .as_os_str()
                .len()
                .cmp(&left.0.as_os_str().len())
                .then_with(|| left.1.cmp(right.1))
        });
        if budget
            .consume(Resource::Work, units(config.paths.len()))
            .is_err()
        {
            return HashMap::new();
        }
        let direct = config.can_resolve_exact_aliases_directly();
        // General lookups inspect the same replacement lists. Compute their
        // reservation once; direct lookups admit only the selected mapping.
        let replacement_work = config.paths.iter().try_fold(0_u64, |total, pattern| {
            total.checked_add(units(pattern.replacements.len()))
        });
        let mut aliases = HashMap::new();
        let mut targets = HashMap::new();
        for alias in &config.paths {
            if budget.consume(Resource::Work, 1).is_err() {
                break;
            }
            if alias.has_wildcard {
                continue;
            }
            let Some(target) = Self::resolve_alias_target(
                config,
                alias,
                direct,
                replacement_work,
                &mut targets,
                budget,
            ) else {
                continue;
            };
            if budget
                .consume(Resource::Work, units(package_paths.len()))
                .is_err()
            {
                break;
            }
            let Some((member, name)) = package_paths
                .iter()
                .find(|(path, _)| target.starts_with(path))
            else {
                continue;
            };
            if alias.prefix == **name || Self::owned_by_inner_package(&target, member, budget) {
                continue;
            }
            if budget
                .consume(Resource::GraphBytes, units(alias.prefix.len()))
                .is_err()
            {
                break;
            }
            if budget
                .consume(Resource::GraphBytes, units(name.len()))
                .is_err()
            {
                break;
            }
            let _ = aliases.insert(alias.prefix.clone(), (*name).clone());
        }
        aliases
    }

    /// A named package between the member root and the alias target (for
    /// example a nested non-member package) owns the target, so the alias is
    /// not an edge to the member; it stays a file alias.
    fn owned_by_inner_package(target: &Path, member: &Path, budget: &AnalysisBudget) -> bool {
        #[derive(serde::Deserialize)]
        struct Named {
            name: Option<serde::de::IgnoredAny>,
        }
        target
            .ancestors()
            .skip(1)
            .take_while(|directory| *directory != member && directory.starts_with(member))
            .any(|directory| {
                if budget.consume(Resource::Work, 1).is_err() {
                    return true;
                }
                let manifest = directory.join("package.json");
                crate::input::read_admitted(&manifest, budget)
                    .ok()
                    .and_then(|text| serde_json::from_str::<Named>(&text).ok())
                    .is_some_and(|named| named.name.is_some())
            })
    }

    /// Direct resolution shares substitution/fallback rules with the public
    /// resolver. Unusual caller-owned tables retain general first-match lookup.
    fn resolve_alias_target<'config>(
        config: &crate::TsConfig,
        alias: &'config crate::tsconfig::PathAlias,
        direct: bool,
        replacement_work: Option<u64>,
        targets: &mut HashMap<&'config [PathBuf], PathBuf>,
        budget: &AnalysisBudget,
    ) -> Option<PathBuf> {
        if !direct {
            return Self::resolve_alias_target_uncached(
                config,
                alias,
                direct,
                replacement_work,
                budget,
            );
        }
        // The complete ordered list determines exact-alias fallback. Borrow
        // the already-owned key, and admit hashing work even on a cache hit.
        budget.consume(Resource::Work, 1).ok()?;
        budget
            .consume(Resource::Work, units(alias.replacements.len()))
            .ok()?;
        if let Some(target) = targets.get(alias.replacements.as_slice()) {
            return Some(target.clone());
        }
        let target =
            Self::resolve_alias_target_uncached(config, alias, direct, replacement_work, budget)?;
        budget
            .consume(
                Resource::GraphBytes,
                units(size_of::<(&[PathBuf], PathBuf)>()),
            )
            .ok()?;
        budget
            .consume(Resource::GraphBytes, units(target.as_os_str().len()))
            .ok()?;
        let _ = targets.insert(alias.replacements.as_slice(), target.clone());
        Some(target)
    }

    #[cfg_attr(feature = "profiling", hotpath::measure)]
    fn resolve_alias_target_uncached(
        config: &crate::TsConfig,
        alias: &crate::tsconfig::PathAlias,
        direct: bool,
        replacement_work: Option<u64>,
        budget: &AnalysisBudget,
    ) -> Option<PathBuf> {
        let target = if direct {
            budget.consume(Resource::Work, 1).ok()?;
            budget
                .consume(Resource::Work, units(alias.replacements.len()))
                .ok()?;
            crate::TsConfig::try_resolve_with_alias(&alias.prefix, alias)
        } else {
            budget
                .consume(Resource::Work, units(config.paths.len()))
                .ok()?;
            if let Some(work) = replacement_work {
                budget.consume(Resource::Work, work).ok()?;
            } else {
                // Sequential checked admissions also reject theoretical sum
                // overflow, preserving sticky failure before any file lookup.
                for pattern in &config.paths {
                    budget
                        .consume(Resource::Work, units(pattern.replacements.len()))
                        .ok()?;
                }
            }
            config.resolve_alias(&alias.prefix)
        }?;
        Some(Self::normalize_alias_target(&target))
    }

    /// This is identity normalization, not filesystem confinement. Preserve
    /// declared subpaths when the final source file is not present in a checkout.
    fn normalize_alias_target(path: &Path) -> PathBuf {
        if let Ok(canonical) = std::fs::canonicalize(path) {
            return canonical;
        }
        let mut normalized = PathBuf::new();
        for component in path.components() {
            match component {
                std::path::Component::ParentDir => {
                    let _removed = normalized.pop();
                },
                std::path::Component::CurDir => {},
                other @ (std::path::Component::Prefix(_)
                | std::path::Component::RootDir
                | std::path::Component::Normal(_)) => normalized.push(other.as_os_str()),
            }
        }
        normalized
    }

    /// Check if an import specifier refers to a workspace package.
    ///
    /// Checks both direct package names and tsconfig path aliases.
    #[inline]
    #[must_use]
    pub fn is_workspace_package(&self, specifier: &str) -> bool {
        self.split_specifier(specifier).is_some()
    }

    /// Split a specifier naming a workspace package into the package's real
    /// name and the subpath after the matched name or alias.
    ///
    /// Declared package names win. An exact tsconfig alias matches the whole
    /// specifier or a prefix ending at a `/`, longest first, so alias names
    /// with extra segments (`lib/a`, `@app/core/utils`) are recognized.
    #[must_use]
    pub fn split_specifier<'specifier>(
        &'specifier self,
        specifier: &'specifier str,
    ) -> Option<(&'specifier str, Option<&'specifier str>)> {
        let package_name = Self::extract_package_name(specifier);
        if self.packages.contains_key(package_name) {
            let subpath = Self::extract_subpath(specifier).filter(|rest| !rest.is_empty());
            return Some((package_name, subpath));
        }
        let mut end = specifier.len();
        loop {
            let prefix = specifier.get(..end)?;
            if let Some(real_name) = self.aliases.get(prefix)
                && self.packages.contains_key(real_name)
            {
                let rest = specifier.get(end..).unwrap_or_default();
                let subpath = rest.strip_prefix('/').filter(|rest| !rest.is_empty());
                return Some((real_name.as_str(), subpath));
            }
            end = prefix.rfind('/')?;
        }
    }

    /// Get the path for a workspace package.
    ///
    /// Resolves tsconfig path aliases to their actual package paths.
    #[inline]
    #[must_use]
    pub fn get_package_path(&self, specifier: &str) -> Option<&PathBuf> {
        let (name, _) = self.split_specifier(specifier)?;
        self.packages.get(name)
    }

    /// Resolve an import specifier to its canonical package name.
    ///
    /// If the specifier uses a tsconfig alias, returns the real package name.
    /// Known workspace names take precedence over aliases, matching
    /// [`Self::get_package_path`]. Otherwise returns the extracted name as-is.
    #[inline]
    #[must_use]
    pub fn resolve_package_name<'specifier>(
        &'specifier self,
        specifier: &'specifier str,
    ) -> &'specifier str {
        self.split_specifier(specifier)
            .map_or_else(|| Self::extract_package_name(specifier), |(name, _)| name)
    }

    /// Extract the package name from an import specifier.
    ///
    /// Handles scoped packages: `@org/pkg/subpath` -> `@org/pkg`
    /// And regular packages: `pkg/subpath` -> `pkg`
    #[inline]
    #[must_use]
    pub fn extract_package_name(specifier: &str) -> &str {
        if specifier.starts_with('@') {
            // Scoped package: @org/pkg or @org/pkg/subpath
            Self::extract_scoped_package_name(specifier)
        } else {
            // Regular package: pkg or pkg/subpath
            specifier.split('/').next().unwrap_or(specifier)
        }
    }

    /// Extract scoped package name (@org/pkg).
    fn extract_scoped_package_name(specifier: &str) -> &str {
        let mut parts = specifier.splitn(3, '/');
        let scope = parts.next().unwrap_or("");
        let name = parts.next().unwrap_or("");

        // Find where @scope/name ends
        let scope_len = scope.len();
        let name_len = name.len();

        if name.is_empty() {
            specifier
        } else {
            // Return @scope/name (scope + '/' + name)
            let end = scope_len.saturating_add(1).saturating_add(name_len);
            specifier.get(..end).unwrap_or(specifier)
        }
    }

    /// Extract the subpath from an import specifier, if any.
    ///
    /// `@org/pkg/foo/bar` -> `Some("foo/bar")`
    /// `@org/pkg` -> `None`
    #[inline]
    #[must_use]
    pub fn extract_subpath(specifier: &str) -> Option<&str> {
        let pkg_name = Self::extract_package_name(specifier);
        let rest = specifier.get(pkg_name.len()..)?;

        if rest.is_empty() {
            None
        } else {
            // Skip leading '/'
            rest.strip_prefix('/')
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    use tempfile::tempdir;

    #[test]
    fn search_starts_from_a_directory_or_a_files_parent() {
        let temp = tempdir().expect("directory");
        let file = temp.path().join("index.ts");
        fs::write(&file, "").expect("file");
        let budget = AnalysisBudget::default();
        for start in [temp.path(), file.as_path()] {
            assert_eq!(
                Workspace::search_start(start, &budget).expect("search start"),
                temp.path(),
                "{start:?}"
            );
        }
        let missing = temp.path().join("missing");
        assert!(
            matches!(
                Workspace::search_start(&missing, &budget),
                Err(WorkspaceError::Read { path, .. }) if path == missing
            ),
            "a missing start is a read error"
        );
    }

    #[cfg(unix)]
    #[test]
    fn search_rejects_a_start_that_is_neither_file_nor_directory() {
        let start = Path::new("/dev/null");
        let error = Workspace::search_start(start, &AnalysisBudget::default())
            .expect_err("a device is not a search start");
        assert!(
            error.to_string().contains("/dev/null"),
            "the error names the start: {error}"
        );
    }

    #[test]
    fn test_extract_package_name_simple() {
        assert_eq!(
            Workspace::extract_package_name("lodash"),
            "lodash",
            "simple package name"
        );
        assert_eq!(
            Workspace::extract_package_name("lodash/fp"),
            "lodash",
            "package with subpath"
        );
    }

    #[test]
    fn test_extract_package_name_scoped() {
        assert_eq!(
            Workspace::extract_package_name("@org/pkg"),
            "@org/pkg",
            "scoped package"
        );
        assert_eq!(
            Workspace::extract_package_name("@org/pkg/utils"),
            "@org/pkg",
            "scoped package with subpath"
        );
        assert_eq!(
            Workspace::extract_package_name("@org/pkg/deep/path"),
            "@org/pkg",
            "scoped package with deep subpath"
        );
    }

    #[test]
    fn test_extract_subpath() {
        assert_eq!(
            Workspace::extract_subpath("lodash"),
            None,
            "no subpath for simple package"
        );
        assert_eq!(
            Workspace::extract_subpath("lodash/fp"),
            Some("fp"),
            "subpath for simple package"
        );
        assert_eq!(
            Workspace::extract_subpath("@org/pkg"),
            None,
            "no subpath for scoped package"
        );
        assert_eq!(
            Workspace::extract_subpath("@org/pkg/utils"),
            Some("utils"),
            "subpath for scoped package"
        );
        assert_eq!(
            Workspace::extract_subpath("@org/pkg/deep/path"),
            Some("deep/path"),
            "deep subpath for scoped package"
        );
    }

    #[test]
    fn test_strip_json_comments() {
        let input = r#"{
            // single line comment
            "key": "value", /* inline comment */
            "other": true
        }"#;
        let stripped = crate::syntax::jsonc(input).expect("valid JSONC");
        assert!(
            !stripped.contains("//"),
            "should strip single-line comments"
        );
        assert!(!stripped.contains("/*"), "should strip multi-line comments");
        assert!(stripped.contains("\"key\""), "should preserve strings");
    }

    #[test]
    fn test_is_workspace_package() {
        let mut packages = HashMap::new();
        let _ = packages.insert("@myorg/utils".to_owned(), PathBuf::from("/pkg/utils"));
        let _ = packages.insert("shared".to_owned(), PathBuf::from("/pkg/shared"));

        let workspace = Workspace {
            root: PathBuf::from("/"),
            format: WorkspaceFormat::Npm,
            packages,
            aliases: HashMap::new(),
        };

        assert!(
            workspace.is_workspace_package("@myorg/utils"),
            "should detect workspace package"
        );
        assert!(
            workspace.is_workspace_package("@myorg/utils/helpers"),
            "should detect workspace package with subpath"
        );
        assert!(
            workspace.is_workspace_package("shared"),
            "should detect simple workspace package"
        );
        assert!(
            !workspace.is_workspace_package("react"),
            "should not match external package"
        );
    }

    // =========================================================================
    // Additional workspace tests for comprehensive coverage
    // =========================================================================

    #[test]
    fn test_extract_package_name_edge_cases() {
        // Empty string
        assert_eq!(
            Workspace::extract_package_name(""),
            "",
            "empty string returns empty"
        );

        // Just an @ symbol
        assert_eq!(
            Workspace::extract_package_name("@"),
            "@",
            "just @ returns @"
        );

        // Scope without package name
        assert_eq!(
            Workspace::extract_package_name("@org/"),
            "@org/",
            "scope without name returns full string"
        );

        // Multiple slashes in simple package
        assert_eq!(
            Workspace::extract_package_name("pkg/a/b/c"),
            "pkg",
            "multiple slashes extracts first part"
        );

        // Package with special characters (valid npm names)
        assert_eq!(
            Workspace::extract_package_name("my-pkg-123"),
            "my-pkg-123",
            "package with hyphens and numbers"
        );

        // Scoped package with special characters
        assert_eq!(
            Workspace::extract_package_name("@my-org/my-pkg"),
            "@my-org/my-pkg",
            "scoped package with hyphens"
        );
    }

    #[test]
    fn test_extract_subpath_edge_cases() {
        // Empty string
        assert_eq!(
            Workspace::extract_subpath(""),
            None,
            "empty string has no subpath"
        );

        // Trailing slash (no actual subpath content)
        assert_eq!(
            Workspace::extract_subpath("pkg/"),
            Some(""),
            "trailing slash gives empty subpath"
        );

        // Complex nested subpath
        assert_eq!(
            Workspace::extract_subpath("lodash/fp/core/utils"),
            Some("fp/core/utils"),
            "deeply nested subpath"
        );

        // Scoped package with complex subpath
        assert_eq!(
            Workspace::extract_subpath("@scope/name/dist/esm/index"),
            Some("dist/esm/index"),
            "scoped package with dist path"
        );
    }

    #[test]
    fn test_get_package_path() {
        let mut packages = HashMap::new();
        let _ = packages.insert("@myorg/utils".to_owned(), PathBuf::from("/pkg/utils"));
        let _ = packages.insert("shared".to_owned(), PathBuf::from("/pkg/shared"));

        let workspace = Workspace {
            root: PathBuf::from("/"),
            format: WorkspaceFormat::Npm,
            packages,
            aliases: HashMap::new(),
        };

        assert_eq!(
            workspace.get_package_path("@myorg/utils"),
            Some(&PathBuf::from("/pkg/utils")),
            "should get path for workspace package"
        );

        assert_eq!(
            workspace.get_package_path("@myorg/utils/helpers"),
            Some(&PathBuf::from("/pkg/utils")),
            "should get path for workspace package with subpath"
        );

        assert_eq!(
            workspace.get_package_path("nonexistent"),
            None,
            "should return None for nonexistent package"
        );
    }

    #[test]
    fn test_workspace_format_equality() {
        assert_eq!(WorkspaceFormat::Npm, WorkspaceFormat::Npm, "Npm equals Npm");
        assert_eq!(
            WorkspaceFormat::Pnpm,
            WorkspaceFormat::Pnpm,
            "Pnpm equals Pnpm"
        );
        assert_eq!(
            WorkspaceFormat::TypeScript,
            WorkspaceFormat::TypeScript,
            "TypeScript equals TypeScript"
        );
        assert_ne!(
            WorkspaceFormat::Npm,
            WorkspaceFormat::Pnpm,
            "Npm not equals Pnpm"
        );
    }

    #[test]
    fn test_strip_json_comments_url_in_string() {
        let input_url = r#"{"url": "http://example.com/path"}"#;
        let stripped_url = crate::syntax::jsonc(input_url).expect("valid JSONC");
        assert!(
            stripped_url.contains("http://example.com/path"),
            "should preserve URLs in strings"
        );
    }

    #[test]
    fn test_strip_json_comments_escaped_quotes() {
        let input_escaped = r#"{"text": "say \"hello\""}"#;
        let stripped_escaped = crate::syntax::jsonc(input_escaped).expect("valid JSONC");
        assert!(
            stripped_escaped.contains(r#"\"hello\""#),
            "should preserve escaped quotes"
        );
    }

    #[test]
    fn test_strip_json_comments_multiple_types() {
        let input_multi = r#"{
            // line comment 1
            "a": 1, // trailing comment
            /* block
               comment */
            "b": 2
        }"#;
        let stripped_multi = crate::syntax::jsonc(input_multi).expect("valid JSONC");
        assert!(
            !stripped_multi.contains("line comment"),
            "should strip line comment"
        );
        assert!(
            !stripped_multi.contains("trailing comment"),
            "should strip trailing comment"
        );
        assert!(
            !stripped_multi.contains("block"),
            "should strip multi-line comment"
        );
        assert!(
            stripped_multi.contains("\"a\": 1"),
            "should preserve JSON content"
        );
        assert!(
            stripped_multi.contains("\"b\": 2"),
            "should preserve JSON content"
        );
    }

    #[test]
    fn test_strip_json_comments_preserves_in_string() {
        let input_str = r#"{"comment": "// not a comment /* also not */"}"#;
        let stripped_str = crate::syntax::jsonc(input_str).expect("valid JSONC");
        assert!(
            stripped_str.contains("// not a comment"),
            "should preserve // inside string"
        );
        assert!(
            stripped_str.contains("/* also not */"),
            "should preserve /* */ inside string"
        );
    }

    #[test]
    fn test_strip_json_comments_unterminated() {
        let input_unterm = r#"{"key": "value"} /* unterminated"#;
        assert!(
            crate::syntax::jsonc(input_unterm).is_err(),
            "unterminated comment is malformed metadata"
        );
    }

    #[test]
    fn test_workspace_clone() {
        let mut packages = HashMap::new();
        let _ = packages.insert("@test/pkg".to_owned(), PathBuf::from("/pkg"));

        let workspace = Workspace {
            root: PathBuf::from("/root"),
            format: WorkspaceFormat::Pnpm,
            packages,
            aliases: HashMap::new(),
        };

        let cloned = workspace.clone();
        assert_eq!(cloned.root, workspace.root, "cloned root should match");
        assert_eq!(
            cloned.format, workspace.format,
            "cloned format should match"
        );
        assert_eq!(
            cloned.packages, workspace.packages,
            "cloned packages should match"
        );
    }

    #[test]
    fn test_workspace_multiple_packages() {
        let mut packages = HashMap::new();
        let _ = packages.insert("@scope/pkg-a".to_owned(), PathBuf::from("/packages/a"));
        let _ = packages.insert("@scope/pkg-b".to_owned(), PathBuf::from("/packages/b"));
        let _ = packages.insert("@scope/pkg-c".to_owned(), PathBuf::from("/packages/c"));
        let _ = packages.insert("unscoped".to_owned(), PathBuf::from("/packages/unscoped"));

        let workspace = Workspace {
            root: PathBuf::from("/"),
            format: WorkspaceFormat::Npm,
            packages,
            aliases: HashMap::new(),
        };

        // All should be workspace packages
        assert!(
            workspace.is_workspace_package("@scope/pkg-a"),
            "pkg-a is workspace package"
        );
        assert!(
            workspace.is_workspace_package("@scope/pkg-b"),
            "pkg-b is workspace package"
        );
        assert!(
            workspace.is_workspace_package("@scope/pkg-c"),
            "pkg-c is workspace package"
        );
        assert!(
            workspace.is_workspace_package("unscoped"),
            "unscoped is workspace package"
        );

        // With subpaths
        assert!(
            workspace.is_workspace_package("@scope/pkg-a/src/utils"),
            "pkg-a with subpath"
        );
        assert!(
            workspace.is_workspace_package("unscoped/lib"),
            "unscoped with subpath"
        );

        // Non-existent packages
        assert!(
            !workspace.is_workspace_package("@scope/pkg-d"),
            "pkg-d is not workspace package"
        );
        assert!(
            !workspace.is_workspace_package("@other/pkg"),
            "different scope is not workspace package"
        );
    }

    #[test]
    fn test_workspace_empty_packages() {
        let workspace = Workspace {
            root: PathBuf::from("/"),
            format: WorkspaceFormat::Npm,
            packages: HashMap::new(),
            aliases: HashMap::new(),
        };

        assert!(
            !workspace.is_workspace_package("any-package"),
            "empty workspace has no packages"
        );
        assert!(
            workspace.get_package_path("any-package").is_none(),
            "empty workspace returns no path"
        );
    }

    // =========================================================================
    // pnpm workspace tests
    // =========================================================================

    #[test]
    fn test_pnpm_workspace_discovery() {
        let fixture_path =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/pnpm-workspace");

        let workspace = Workspace::discover(&fixture_path)
            .expect("workspace discovery should succeed")
            .expect("fixture should be a workspace");

        assert_eq!(
            workspace.format,
            WorkspaceFormat::Pnpm,
            "should detect pnpm format"
        );
        assert_eq!(workspace.packages.len(), 2, "should find 2 packages");
    }

    #[test]
    fn test_pnpm_workspace_package_names() {
        let fixture_path =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/pnpm-workspace");

        let workspace = Workspace::discover(&fixture_path)
            .expect("workspace discovery should succeed")
            .expect("fixture should be a workspace");

        assert!(
            workspace.packages.contains_key("@pnpm-test/a"),
            "should find @pnpm-test/a"
        );
        assert!(
            workspace.packages.contains_key("@pnpm-test/b"),
            "should find @pnpm-test/b"
        );
    }

    #[test]
    fn test_pnpm_workspace_complex_discovery() {
        let fixture_path =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/pnpm-workspace-complex");

        let workspace = Workspace::discover(&fixture_path)
            .expect("workspace discovery should succeed")
            .expect("fixture should be a workspace");

        assert_eq!(
            workspace.format,
            WorkspaceFormat::Pnpm,
            "should detect pnpm format"
        );

        // Should find core and utils, but NOT ignored (due to negative glob !packages/ignored)
        assert!(
            workspace.packages.contains_key("@pnpm-complex/core"),
            "should find @pnpm-complex/core"
        );
        assert!(
            workspace.packages.contains_key("@pnpm-complex/utils"),
            "should find @pnpm-complex/utils"
        );
    }

    #[test]
    fn test_pnpm_workspace_root_has_correct_path() {
        let fixture_path =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/pnpm-workspace");

        let workspace = Workspace::discover(&fixture_path)
            .expect("workspace discovery should succeed")
            .expect("fixture should be a workspace");

        assert!(
            workspace.root.ends_with("pnpm-workspace"),
            "root should be the fixture directory"
        );
    }

    // =========================================================================
    // npm workspace tests
    // =========================================================================

    #[test]
    fn test_npm_workspace_discovery() {
        let fixture_path =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/inter-package-simple");

        let workspace = Workspace::discover(&fixture_path)
            .expect("workspace discovery should succeed")
            .expect("fixture should be a workspace");

        assert_eq!(
            workspace.format,
            WorkspaceFormat::Npm,
            "should detect npm format"
        );
    }

    #[test]
    fn test_npm_workspace_package_paths() {
        let fixture_path =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/inter-package-simple");

        let workspace = Workspace::discover(&fixture_path)
            .expect("workspace discovery should succeed")
            .expect("fixture should be a workspace");

        // Verify paths are resolved correctly
        let pkg_a_path = workspace
            .packages
            .get("@simple/a")
            .expect("should find @simple/a");
        assert!(
            pkg_a_path.ends_with("packages/pkg-a"),
            "pkg-a path should end with packages/pkg-a"
        );

        let pkg_b_path = workspace
            .packages
            .get("@simple/b")
            .expect("should find @simple/b");
        assert!(
            pkg_b_path.ends_with("packages/pkg-b"),
            "pkg-b path should end with packages/pkg-b"
        );
    }

    // =========================================================================
    // Workspace error handling tests
    // =========================================================================

    #[test]
    fn test_workspace_error_display() {
        use crate::WorkspaceError;

        let read_err = WorkspaceError::Read {
            path: PathBuf::from("/test/path"),
            source: std::io::Error::new(std::io::ErrorKind::NotFound, "test error"),
        };
        let read_msg = format!("{read_err}");
        assert!(
            read_msg.contains("/test/path"),
            "Read error should contain path"
        );
        assert!(
            read_msg.contains("test error"),
            "Read error should contain source message"
        );

        let parse_err = WorkspaceError::Parse {
            path: PathBuf::from("/test/config.json"),
            message: "invalid JSON".to_owned(),
        };
        let parse_msg = format!("{parse_err}");
        assert!(
            parse_msg.contains("/test/config.json"),
            "Parse error should contain path"
        );
        assert!(
            parse_msg.contains("invalid JSON"),
            "Parse error should contain message"
        );

        let glob_err = WorkspaceError::InvalidPattern {
            pattern: "**[invalid".to_owned(),
            source: glob::Pattern::new("**[invalid").unwrap_err(),
        };
        let glob_msg = format!("{glob_err}");
        assert!(
            glob_msg.contains("**[invalid"),
            "Glob error should contain pattern"
        );
    }

    // =========================================================================
    // Non-workspace directory tests
    // =========================================================================

    #[test]
    fn test_non_workspace_directory() {
        let fixture_path =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/simple-cycle");

        let workspace = Workspace::discover(&fixture_path).expect("discovery should not error");

        assert!(
            workspace.is_none(),
            "simple-cycle fixture is not a workspace"
        );
    }

    // =========================================================================
    // tsconfig path alias tests
    // =========================================================================

    #[test]
    fn test_tsconfig_aliases_discovery() {
        let fixture_path =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/tsconfig-aliases");

        let workspace = Workspace::discover(&fixture_path)
            .expect("workspace discovery should succeed")
            .expect("fixture should be a workspace");

        assert_eq!(
            workspace.format,
            WorkspaceFormat::Pnpm,
            "should detect pnpm format"
        );
        assert_eq!(workspace.packages.len(), 2, "should find 2 packages");
    }

    #[test]
    fn test_tsconfig_aliases_loaded() {
        let fixture_path =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/tsconfig-aliases");

        let workspace = Workspace::discover(&fixture_path)
            .expect("workspace discovery should succeed")
            .expect("fixture should be a workspace");

        // Check that aliases are loaded (wildcard aliases are skipped)
        assert_eq!(
            workspace.aliases.len(),
            2,
            "should have 2 non-wildcard aliases"
        );

        // Verify specific aliases
        assert_eq!(
            workspace.aliases.get("@alias-test/package-a"),
            Some(&"@alias-test/pkg-a".to_owned()),
            "package-a alias should map to pkg-a"
        );
        assert_eq!(
            workspace.aliases.get("@alias-test/package-b"),
            Some(&"@alias-test/pkg-b".to_owned()),
            "package-b alias should map to pkg-b"
        );
    }

    #[test]
    fn test_tsconfig_aliases_is_workspace_package() {
        let fixture_path =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/tsconfig-aliases");

        let workspace = Workspace::discover(&fixture_path)
            .expect("workspace discovery should succeed")
            .expect("fixture should be a workspace");

        // Direct package names work
        assert!(
            workspace.is_workspace_package("@alias-test/pkg-a"),
            "direct package name should work"
        );
        assert!(
            workspace.is_workspace_package("@alias-test/pkg-b"),
            "direct package name should work"
        );

        // Aliases also work
        assert!(
            workspace.is_workspace_package("@alias-test/package-a"),
            "alias should be recognized as workspace package"
        );
        assert!(
            workspace.is_workspace_package("@alias-test/package-b"),
            "alias should be recognized as workspace package"
        );

        // With subpaths
        assert!(
            workspace.is_workspace_package("@alias-test/package-a/utils"),
            "alias with subpath should be recognized"
        );
    }

    #[test]
    fn test_tsconfig_aliases_resolve_package_name() {
        let fixture_path =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/tsconfig-aliases");

        let workspace = Workspace::discover(&fixture_path)
            .expect("workspace discovery should succeed")
            .expect("fixture should be a workspace");

        // Direct names resolve to themselves
        assert_eq!(
            workspace.resolve_package_name("@alias-test/pkg-a"),
            "@alias-test/pkg-a",
            "direct name resolves to itself"
        );

        // Aliases resolve to real package names
        assert_eq!(
            workspace.resolve_package_name("@alias-test/package-a"),
            "@alias-test/pkg-a",
            "alias resolves to real package name"
        );

        // With subpaths
        assert_eq!(
            workspace.resolve_package_name("@alias-test/package-a/utils"),
            "@alias-test/pkg-a",
            "alias with subpath resolves to real package name"
        );
    }

    #[test]
    fn test_tsconfig_aliases_get_package_path() {
        let fixture_path =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/tsconfig-aliases");

        let workspace = Workspace::discover(&fixture_path)
            .expect("workspace discovery should succeed")
            .expect("fixture should be a workspace");

        // Direct package name
        let direct_path = workspace.get_package_path("@alias-test/pkg-a");
        assert!(
            direct_path.is_some(),
            "direct package name should have path"
        );
        assert!(
            direct_path
                .expect("checked above")
                .ends_with("packages/pkg-a"),
            "path should end with packages/pkg-a"
        );

        // Alias should resolve to same path
        let alias_path = workspace.get_package_path("@alias-test/package-a");
        assert!(alias_path.is_some(), "alias should resolve to path");
        assert_eq!(
            direct_path, alias_path,
            "alias should resolve to same path as direct name"
        );
    }

    #[test]
    fn test_recursive_workspace_patterns_skip_excluded_dirs() {
        let temp = tempdir().expect("tempdir should be created");
        let root = temp.path();

        let real_pkg = root.join("apps/real-app");
        let ignored_pkg = root.join("apps/real-app/node_modules/fake-pkg");
        let dist_pkg = root.join("apps/real-app/dist/generated-pkg");

        fs::create_dir_all(&real_pkg).expect("real package dir should be created");
        fs::create_dir_all(&ignored_pkg).expect("node_modules package dir should be created");
        fs::create_dir_all(&dist_pkg).expect("dist package dir should be created");

        fs::write(
            real_pkg.join("package.json"),
            r#"{ "name": "@demo/real-app" }"#,
        )
        .expect("real package manifest should be written");
        fs::write(
            ignored_pkg.join("package.json"),
            r#"{ "name": "@demo/fake-pkg" }"#,
        )
        .expect("node_modules package manifest should be written");
        fs::write(
            dist_pkg.join("package.json"),
            r#"{ "name": "@demo/generated-pkg" }"#,
        )
        .expect("dist package manifest should be written");

        let packages = Workspace::expand_patterns(
            root,
            &[String::from("apps/**/*")],
            &AnalysisBudget::default(),
        )
        .expect("workspace patterns should expand");

        assert!(
            packages.contains_key("@demo/real-app"),
            "should include the real recursive workspace package"
        );
        assert!(
            !packages.contains_key("@demo/fake-pkg"),
            "should skip node_modules packages"
        );
        assert!(
            !packages.contains_key("@demo/generated-pkg"),
            "should skip dist packages"
        );
    }
}
