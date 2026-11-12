//! Import extraction from JavaScript/TypeScript files using tree-sitter cursors.
//!
//! Parses source files and extracts import statements, resolving them to file paths.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use rayon::prelude::*;

use crate::ExtractError;
use crate::budget::{AnalysisBudget, Resource, units};
use crate::tsconfig::{TsConfig, names_directory, normalize_module_path};
use crate::workspace::Workspace;

/// Resolution kinds occupy separate namespaces: a real `__index__` filename
/// must never alias a memoized directory lookup.
#[derive(Default)]
struct ResolutionEntries {
    files: HashMap<PathBuf, Option<PathBuf>>,
    directories: HashMap<PathBuf, Option<PathBuf>>,
    aliases: HashMap<String, Option<PathBuf>>,
    walked_indices: HashSet<PathBuf>,
    /// Canonical root of the package being extracted.
    root: Option<PathBuf>,
}

type ResolutionCache = Arc<Mutex<ResolutionEntries>>;

/// Controls which imports are extracted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ExtractionMode {
    /// Extract and resolve all imports.
    #[default]
    All,
    /// Only keep imports that target workspace packages.
    WorkspaceOnly,
}

/// An extracted import statement.
#[derive(Debug, Clone)]
pub struct Import {
    /// File containing the import.
    pub source: PathBuf,

    /// What's being imported.
    pub target: ImportTarget,

    /// Decoded JavaScript module specifier value, without surrounding quotes.
    pub specifier: String,
}

/// Classification of an import target.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImportTarget {
    /// Successfully resolved to a file path.
    Resolved(PathBuf),

    /// External module (`node_modules`, not a workspace package).
    External(String),

    /// Workspace package import.
    WorkspacePackage {
        /// The package name (e.g., "@myorg/utils").
        package_name: String,
        /// Optional subpath (e.g., "helpers" from "@myorg/utils/helpers").
        subpath: Option<String>,
    },

    /// Could not resolve the import.
    Unresolved(String),
}

/// Import extraction engine.
pub struct ImportExtractor {
    /// File extensions to analyze.
    extensions: Vec<String>,

    /// TypeScript configuration for path alias resolution.
    tsconfig: Option<Arc<TsConfig>>,

    /// Workspace configuration for workspace package detection.
    workspace: Option<Arc<Workspace>>,

    /// Controls whether to resolve all imports or only workspace imports.
    mode: ExtractionMode,

    /// Glob patterns for files to ignore.
    ignore_patterns: Vec<crate::input::PathPattern>,
    invalid_ignore: Option<String>,

    /// Canonical roots of other packages nested inside this one; the nearest
    /// package owns their files.
    nested_packages: HashSet<PathBuf>,

    /// Canonical roots of every package and excluded boundary in the scan;
    /// a file link belongs to the owner of its target.
    owners: Arc<[PathBuf]>,

    /// Leave out imports erased from JavaScript output.
    skip_type_imports: bool,

    /// Cache for extension resolution to avoid redundant filesystem stat calls.
    resolution_cache: ResolutionCache,
    budget: AnalysisBudget,
}

impl ImportExtractor {
    /// Create a new import extractor.
    #[inline]
    pub fn new(extensions: Vec<String>, tsconfig: Option<TsConfig>) -> Self {
        Self {
            extensions,
            tsconfig: tsconfig.map(Arc::new),
            workspace: None,
            mode: ExtractionMode::All,
            ignore_patterns: Vec::new(),
            invalid_ignore: None,
            nested_packages: HashSet::new(),
            owners: Arc::from(Vec::new()),
            skip_type_imports: false,
            resolution_cache: Arc::new(Mutex::new(ResolutionEntries::default())),
            budget: AnalysisBudget::default(),
        }
    }

    /// Set workspace configuration for workspace package detection.
    #[inline]
    #[must_use]
    pub fn with_workspace(mut self, workspace: Option<Workspace>) -> Self {
        self.workspace = workspace.map(Arc::new);
        self
    }

    /// Share scan-wide quotas across extraction workers.
    #[must_use]
    pub fn with_budget(mut self, budget: AnalysisBudget) -> Self {
        self.budget = budget;
        self
    }

    /// Reuse an immutable workspace snapshot across packages.
    #[must_use]
    pub fn with_shared_workspace(mut self, workspace: Option<Arc<Workspace>>) -> Self {
        self.workspace = workspace;
        self
    }

    /// Reuse an immutable tsconfig snapshot across packages.
    #[must_use]
    pub fn with_shared_tsconfig(mut self, tsconfig: Option<Arc<TsConfig>>) -> Self {
        self.tsconfig = tsconfig;
        self
    }

    /// Set extraction mode.
    #[inline]
    #[must_use]
    pub const fn with_mode(mut self, mode: ExtractionMode) -> Self {
        self.mode = mode;
        self
    }

    /// Set ignore patterns for files to skip. Invalid patterns are retained as
    /// an error and rejected by extract before filesystem traversal.
    #[inline]
    #[must_use]
    pub fn with_ignore_patterns(mut self, patterns: Vec<String>) -> Self {
        self.ignore_patterns.clear();
        self.invalid_ignore = None;
        for pattern in patterns {
            if self.budget.consume(Resource::Work, 1).is_err() {
                break;
            }
            if self
                .budget
                .consume(Resource::GraphBytes, units(pattern.len()))
                .is_err()
            {
                break;
            }
            match crate::input::PathPattern::new(&pattern) {
                Ok(compiled) => self.ignore_patterns.push(compiled),
                Err(error) => {
                    self.invalid_ignore = Some(format!("invalid ignore glob {pattern:?}: {error}"));
                    break;
                },
            }
        }
        self
    }

    /// Leave the given canonical directories to the packages rooted there, so
    /// each file belongs to its nearest package.
    #[must_use]
    pub fn with_nested_packages(mut self, roots: impl IntoIterator<Item = PathBuf>) -> Self {
        self.nested_packages = roots.into_iter().collect();
        self
    }

    /// Share the canonical roots of every package and excluded boundary in
    /// the scan, so a file link whose target another package owns (sibling or
    /// nested) is left to that package.
    #[must_use]
    pub fn with_owners(mut self, owners: Arc<[PathBuf]>) -> Self {
        self.owners = owners;
        self
    }

    /// Leave out imports TypeScript erases from JavaScript output: `import
    /// type`, `export type`, type-position `import()`, and every import of a
    /// declaration file (`.d.ts`, `.d.mts`, `.d.cts`). Declaration files are
    /// still parsed, so syntax errors in them still fail.
    #[must_use]
    pub const fn with_skip_type_imports(mut self, skip: bool) -> Self {
        self.skip_type_imports = skip;
        self
    }

    /// Extract all imports from files in a package directory.
    ///
    /// Uses parallel file I/O for improved performance on large packages.
    /// Sources and resolved local targets use canonical absolute paths so aliases
    /// and relative imports identify the same graph nodes.
    ///
    /// # Errors
    ///
    /// Returns an error if file reading or parsing fails.
    #[inline]
    #[cfg_attr(feature = "profiling", hotpath::measure)]
    pub fn extract(&self, package_path: &Path) -> Result<Vec<Import>, ExtractError> {
        self.budget.check()?;
        if let Some(message) = &self.invalid_ignore {
            return Err(ExtractError::ParseError {
                path: package_path.to_path_buf(),
                message: message.clone(),
            });
        }
        let root = std::fs::canonicalize(package_path).map_err(|source| ExtractError::IoError {
            path: package_path.to_path_buf(),
            source,
        })?;
        // Collect all file paths first
        let mut files = Vec::new();
        let mut index_hints = HashSet::new();
        for entry in crate::input::walk(&root, &self.budget, |relative, full| {
            self.should_skip(relative, full)
        }) {
            let entry = entry.map_err(|source| ExtractError::IoError {
                path: root.clone(),
                source,
            })?;
            self.budget
                .consume(Resource::Work, units(self.extensions.len()))?;
            if !entry.file_type().is_dir()
                && entry
                    .path()
                    .extension()
                    .and_then(|ext| ext.to_str())
                    .is_some_and(|ext| self.extensions.iter().any(|expected| expected == ext))
            {
                if entry.path_is_symlink() && self.link_owned_elsewhere(&root, entry.path())? {
                    continue;
                }
                self.budget.consume(Resource::Files, 1)?;
                self.budget
                    .consume(Resource::GraphBytes, units(entry.path().as_os_str().len()))?;
                if self.mode == ExtractionMode::All
                    && entry.file_type().is_file()
                    && entry.file_name() == std::ffi::OsStr::new("index.ts")
                {
                    self.retain_index_hint(entry.path(), &mut index_hints)?;
                }
                files.push(entry.into_path());
            }
        }

        crate::budget::sorting(files.len(), &self.budget)?;
        files.sort();
        // A new extraction observes new files; filesystem resolution caches are
        // valid only within this extraction's stable-checkout assumption.
        let mut cache = self
            .resolution_cache
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        cache.files.clear();
        cache.directories.clear();
        cache.aliases.clear();
        cache.walked_indices = index_hints;
        cache.root = Some(root);
        drop(cache);

        // Process files in parallel
        let results: Vec<Result<Vec<Import>, ExtractError>> = files
            .par_iter()
            .map(|path| self.extract_from_file(path))
            .collect();

        // Aggregate results, propagating first error
        let mut imports = Vec::new();
        for result in results {
            imports.extend(result?);
        }

        self.budget.check()?;
        Ok(imports)
    }

    /// Roots are canonical and child directory links are not followed, so a
    /// regular first-index entry supplies a positive canonical target in the
    /// stable checkout. Missing/ignored/later candidates imply no absence and
    /// retain normal filesystem lookup. No map lock spans this traversal.
    #[cfg_attr(feature = "profiling", hotpath::measure)]
    fn retain_index_hint(
        &self,
        path: &Path,
        hints: &mut HashSet<PathBuf>,
    ) -> Result<(), crate::budget::LimitError> {
        self.budget.consume(Resource::Work, 1)?;
        self.budget
            .consume(Resource::GraphBytes, units(size_of::<PathBuf>()))?;
        self.budget
            .consume(Resource::GraphBytes, units(path.as_os_str().len()))?;
        let _ = hints.insert(path.to_path_buf());
        Ok(())
    }

    /// A file link belongs to the nearest owner of its canonical target (a
    /// package or excluded boundary), not to the package containing the link.
    /// Targets outside every owner stay with this package.
    fn link_owned_elsewhere(&self, root: &Path, link: &Path) -> Result<bool, ExtractError> {
        self.budget.consume(
            Resource::Work,
            units(self.nested_packages.len().saturating_add(self.owners.len())),
        )?;
        let target = canonical_target(link.to_path_buf())?;
        let nearest = self
            .owners
            .iter()
            .chain(&self.nested_packages)
            .map(PathBuf::as_path)
            .chain(std::iter::once(root))
            .filter(|owner| target.starts_with(owner))
            .max_by_key(|owner| owner.components().count());
        Ok(nearest.is_some_and(|owner| owner != root))
    }

    /// Check if a path should be skipped.
    #[cfg_attr(feature = "profiling", hotpath::measure)]
    fn should_skip(&self, relative: &Path, full: &Path) -> bool {
        if self
            .budget
            .consume(Resource::Work, units(self.ignore_patterns.len()))
            .is_err()
        {
            return true;
        }
        crate::input::excluded_component(relative)
            || self.nested_packages.contains(full)
            || self
                .ignore_patterns
                .iter()
                .any(|pattern| pattern.matches(relative, full))
    }

    /// Extract imports from a single file.
    #[cfg_attr(feature = "profiling", hotpath::measure)]
    fn extract_from_file(&self, original_path: &Path) -> Result<Vec<Import>, ExtractError> {
        let canonical =
            std::fs::canonicalize(original_path).map_err(|source| ExtractError::IoError {
                path: original_path.to_path_buf(),
                source,
            })?;
        let file_path = canonical.as_path();
        if file_path.to_str().is_none() {
            return Err(ExtractError::ParseError {
                path: canonical,
                message: "source paths must be representable as UTF-8".to_owned(),
            });
        }
        let source = crate::input::read_admitted(file_path, &self.budget).map_err(|source| {
            ExtractError::IoError {
                path: file_path.to_path_buf(),
                source,
            }
        })?;

        self.extract_source(&source, file_path)
    }

    fn extract_source(&self, source: &str, file_path: &Path) -> Result<Vec<Import>, ExtractError> {
        let mut imports = Vec::new();
        let declaration = self.skip_type_imports && is_declaration_file(file_path);
        crate::module_syntax::visit(
            source,
            file_path,
            crate::module_syntax::Language::for_path(file_path),
            &self.budget,
            self.skip_type_imports,
            |literal| {
                if declaration {
                    return Ok(());
                }
                if let Some(import) = self.create_import_from_specifier_text(literal, file_path)? {
                    imports.push(import);
                }
                Ok(())
            },
        )?;
        self.budget.check()?;
        Ok(imports)
    }

    /// Decode before resolving identity. Escaped source text is not the module value.
    fn create_import_from_specifier_text(
        &self,
        text: &str,
        file_path: &Path,
    ) -> Result<Option<Import>, ExtractError> {
        self.budget.consume(Resource::Work, units(text.len()))?;
        // A UTF-16 decode buffer and UTF-8 result are bounded before allocation.
        if text.contains('\\') {
            for _ in 0..3 {
                self.budget
                    .consume(Resource::GraphBytes, units(text.len()))?;
            }
        }
        let decoded =
            crate::syntax::module_literal(text).map_err(|message| ExtractError::ParseError {
                path: file_path.to_path_buf(),
                message: message.to_owned(),
            })?;
        let specifier = decoded.as_ref();
        if specifier.contains('\0') {
            return Err(ExtractError::ParseError {
                path: file_path.to_path_buf(),
                message: "module specifier must not contain NUL".to_owned(),
            });
        }
        if specifier.is_empty() {
            return Ok(None);
        }
        if self.mode == ExtractionMode::WorkspaceOnly {
            // A file in another member reached by a relative path or file
            // alias is a package edge too, so resolve fully and keep only
            // package targets.
            let target = self.resolve_import(file_path, specifier)?;
            if !matches!(target, ImportTarget::WorkspacePackage { .. }) {
                return Ok(None);
            }
            self.reserve_import(specifier, file_path)?;
            self.admit_target(&target)?;
            return Ok(Some(Import {
                source: file_path.to_path_buf(),
                target,
                specifier: specifier.to_owned(),
            }));
        }
        self.reserve_import(specifier, file_path)?;
        let target = self.resolve_import(file_path, specifier)?;
        self.admit_target(&target)?;
        Ok(Some(Import {
            source: file_path.to_path_buf(),
            target,
            specifier: specifier.to_owned(),
        }))
    }

    /// Admit a record and its source/specifier before allocating retained strings.
    fn reserve_import(
        &self,
        specifier: &str,
        file_path: &Path,
    ) -> Result<(), crate::budget::LimitError> {
        self.budget.consume(Resource::Imports, 1)?;
        self.budget.consume(Resource::Work, 1)?;
        // One admission covers all retained pieces before any are allocated.
        // This avoids three contended updates to the same scan-wide counter.
        let pieces = [
            units(size_of::<Import>()),
            units(file_path.as_os_str().len()),
            units(specifier.len()),
        ];
        if let Some(total) = pieces.into_iter().try_fold(0_u64, u64::checked_add) {
            self.budget.consume(Resource::GraphBytes, total)?;
        } else {
            // Preserve checked, sticky rejection even for a theoretical sum
            // exceeding u64::MAX; sequential admissions cannot all succeed.
            for bytes in pieces {
                self.budget.consume(Resource::GraphBytes, bytes)?;
            }
        }
        Ok(())
    }

    /// Reserve a resolved target before retaining the record. Resolution itself
    /// uses bounded input strings; cache entries have separate reservations.
    fn admit_target(&self, target: &ImportTarget) -> Result<(), crate::budget::LimitError> {
        let bytes = match target {
            ImportTarget::Resolved(path) => path.as_os_str().len(),
            ImportTarget::External(text) | ImportTarget::Unresolved(text) => text.len(),
            ImportTarget::WorkspacePackage {
                package_name,
                subpath,
            } => {
                self.budget.consume(
                    Resource::GraphBytes,
                    units(subpath.as_ref().map_or(0, String::len)),
                )?;
                package_name.len()
            },
        };
        self.budget.consume(Resource::GraphBytes, units(bytes))
    }

    /// Resolve an import specifier to a file path.
    #[cfg_attr(feature = "profiling", hotpath::measure)]
    fn resolve_import(&self, from: &Path, specifier: &str) -> Result<ImportTarget, ExtractError> {
        self.budget.check()?;
        let target = if specifier.starts_with('.') || specifier.starts_with('/') {
            // Relative cache entries already contain canonical identities. A
            // popular target should not repeat realpath for every incoming edge.
            self.resolve_relative_import(from, specifier)?
        } else {
            self.resolve_external_or_alias(from, specifier)?
        };
        self.attribute_to_member(from, target)
    }

    /// A file resolved into another workspace member, by a relative path or a
    /// file alias, is a dependency on that member, as if imported by name.
    /// The nearest package root containing the file owns it: a nested
    /// non-member package or an excluded directory keeps it a file import.
    fn attribute_to_member(
        &self,
        from: &Path,
        target: ImportTarget,
    ) -> Result<ImportTarget, ExtractError> {
        let (Some(workspace), ImportTarget::Resolved(path)) = (&self.workspace, &target) else {
            return Ok(target);
        };
        if self.owned_by_extracting_package(path)? {
            return Ok(target);
        }
        let units_needed = units(self.owners.len().saturating_add(workspace.packages.len()));
        self.budget.consume(Resource::Work, units_needed)?;
        let depth = |root: &Path| root.components().count();
        let Some((name, member)) = workspace
            .packages
            .iter()
            .filter(|(_, directory)| path.starts_with(directory))
            .max_by_key(|(_, directory)| depth(directory))
        else {
            return Ok(target);
        };
        let nearer_owner = self
            .owners
            .iter()
            .any(|root| path.starts_with(root) && depth(root) > depth(member));
        if nearer_owner || self.is_own_package(workspace, name, from) {
            return Ok(target);
        }
        Ok(ImportTarget::WorkspacePackage {
            package_name: name.clone(),
            subpath: None,
        })
    }

    /// The package being extracted owns `path`: it lies under the package root
    /// and under none of the packages nested in it.
    fn owned_by_extracting_package(&self, path: &Path) -> Result<bool, ExtractError> {
        let root = self
            .resolution_cache
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .root
            .clone();
        let Some(root) = root else {
            return Ok(false);
        };
        if !path.starts_with(&root) {
            return Ok(false);
        }
        self.budget
            .consume(Resource::Work, units(self.nested_packages.len()))?;
        Ok(!self
            .nested_packages
            .iter()
            .any(|nested| path.starts_with(nested)))
    }

    /// Resolve an external module, tsconfig alias, or workspace package.
    fn resolve_external_or_alias(
        &self,
        from: &Path,
        specifier: &str,
    ) -> Result<ImportTarget, ExtractError> {
        // Check if it's a workspace package FIRST (before tsconfig aliases)
        // This is important because tsconfig aliases might resolve to a path within
        // a workspace package, but we want to track package-level dependencies.
        if let Some(workspace) = &self.workspace
            && let Some((package_name, subpath)) = workspace.split_specifier(specifier)
            && !self.is_own_package(workspace, package_name, from)
        {
            return Ok(ImportTarget::WorkspacePackage {
                package_name: package_name.to_owned(),
                subpath: subpath.map(str::to_owned),
            });
        }

        // Check if it's a tsconfig path alias (for non-workspace imports)
        if let Some(tsconfig) = &self.tsconfig
            && let Some(target) = self.resolve_alias_cached(tsconfig, specifier)?
        {
            return Ok(target);
        }

        Ok(ImportTarget::External(specifier.to_owned()))
    }

    /// A package importing its own name has no package edge to record; its
    /// file aliases still supply the file edge (otherwise it is dropped as a
    /// package self-edge and lost from both graphs).
    ///
    /// The source's own location decides, not only the extraction root: a file
    /// reached through a link from another package still imports itself when
    /// it lies in the named member and no nested member is nearer.
    fn is_own_package(&self, workspace: &Workspace, package_name: &str, from: &Path) -> bool {
        let Some(path) = workspace.packages.get(package_name) else {
            return false;
        };
        let extracting = self
            .resolution_cache
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .root
            .as_ref()
            .is_some_and(|root| root == path);
        extracting
            || (from.starts_with(path)
                && !workspace.packages.values().any(|other| {
                    other != path && other.starts_with(path) && from.starts_with(other)
                }))
    }

    /// Only matched file aliases are memoized. A cache miss is distinct from a
    /// matched alias with no file, and workspace classification happens first.
    /// No cache lock spans file lookup or canonicalization.
    fn resolve_alias_cached(
        &self,
        config: &TsConfig,
        specifier: &str,
    ) -> Result<Option<ImportTarget>, ExtractError> {
        let cached = self
            .resolution_cache
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .aliases
            .get(specifier)
            .cloned();
        let resolved = if let Some(cached) = cached {
            cached
        } else {
            // Admit alias-table traversal only when it will actually run.
            // Relative/workspace imports and warm entries never scan this table.
            self.budget
                .consume(Resource::Work, units(config.paths.len()))?;
            for alias in &config.paths {
                self.budget
                    .consume(Resource::Work, units(alias.replacements.len()))?;
            }
            let Some(candidate) = config.resolve_alias(specifier) else {
                return Ok(None);
            };
            let resolved = if candidate.is_file() {
                Some(canonical_target(candidate)?)
            } else {
                None
            };
            let mut cache = self
                .resolution_cache
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            // Another worker may have filled the entry during filesystem I/O.
            // Admit only the payload that this cache will actually retain.
            if let Some(existing) = cache.aliases.get(specifier) {
                existing.clone()
            } else {
                self.budget
                    .consume(Resource::GraphBytes, units(specifier.len()))?;
                self.budget.consume(
                    Resource::GraphBytes,
                    units(resolved.as_ref().map_or(0, |path| path.as_os_str().len())),
                )?;
                drop(cache.aliases.insert(specifier.to_owned(), resolved.clone()));
                resolved
            }
        };
        Ok(Some(resolved.map_or_else(
            || ImportTarget::Unresolved(specifier.to_owned()),
            ImportTarget::Resolved,
        )))
    }

    /// Resolve a relative import specifier.
    fn resolve_relative_import(
        &self,
        from: &Path,
        specifier: &str,
    ) -> Result<ImportTarget, ExtractError> {
        let base_dir = from.parent().unwrap_or_else(|| Path::new("."));
        let resolved_base = normalize_module_path(&base_dir.join(specifier));

        // `./dir/`, `.` and `..` name a directory; a sibling `dir.ts` must not
        // match them (Node/TypeScript resolve only the directory's index).
        if !names_directory(Path::new(specifier))
            && let Some(resolved) = self.try_resolve_with_ext_cached(&resolved_base)?
        {
            return Ok(ImportTarget::Resolved(resolved));
        }

        // Try as directory with index file (using cache)
        if let Some(index) = self.try_resolve_index_cached(&resolved_base)? {
            return Ok(ImportTarget::Resolved(index));
        }

        Ok(ImportTarget::Unresolved(specifier.to_owned()))
    }

    /// Try to resolve with various extensions, using cache to avoid redundant fs calls.
    fn try_resolve_with_ext_cached(&self, base: &Path) -> Result<Option<PathBuf>, ExtractError> {
        // Check cache first
        {
            let cache = self
                .resolution_cache
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if let Some(cached) = cache.files.get(base) {
                return Ok(cached.clone());
            }
        }

        // Perform actual filesystem resolution
        let result = Self::try_resolve_with_ext(base)
            .map(canonical_target)
            .transpose()?;

        // Reserve the cloned cache key/value before retaining it.
        self.budget
            .consume(Resource::GraphBytes, units(base.as_os_str().len()))?;
        self.budget.consume(
            Resource::GraphBytes,
            units(result.as_ref().map_or(0, |path| path.as_os_str().len())),
        )?;
        drop(
            self.resolution_cache
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .files
                .insert(base.to_path_buf(), result.clone()),
        );

        Ok(result)
    }

    /// Try to resolve with various extensions (uncached).
    fn try_resolve_with_ext(base: &Path) -> Option<PathBuf> {
        TsConfig::try_resolve_with_extensions(base)
    }

    /// Try to resolve as directory with index file, using cache.
    fn try_resolve_index_cached(&self, path: &Path) -> Result<Option<PathBuf>, ExtractError> {
        // Check cache first
        {
            let cache = self
                .resolution_cache
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if let Some(cached) = cache.directories.get(path) {
                return Ok(cached.clone());
            }
            self.budget.consume(Resource::Work, 1)?;
            let candidate = path.join("index.ts");
            if cache.walked_indices.contains(&candidate) {
                return Ok(Some(candidate));
            }
        }

        // Perform actual filesystem resolution
        let result = Self::try_resolve_index(path)
            .map(canonical_target)
            .transpose()?;

        // Reserve the cloned cache key/value before retaining it.
        self.budget
            .consume(Resource::GraphBytes, units(path.as_os_str().len()))?;
        self.budget.consume(
            Resource::GraphBytes,
            units(result.as_ref().map_or(0, |value| value.as_os_str().len())),
        )?;
        drop(
            self.resolution_cache
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .directories
                .insert(path.to_path_buf(), result.clone()),
        );

        Ok(result)
    }

    /// Try to resolve as directory with index file (uncached).
    #[cfg_attr(feature = "profiling", hotpath::measure)]
    fn try_resolve_index(path: &Path) -> Option<PathBuf> {
        if !path.is_dir() {
            return None;
        }
        for index in &[
            "index.ts",
            "index.tsx",
            "index.d.ts",
            "index.js",
            "index.jsx",
        ] {
            let candidate = path.join(index);
            if candidate.is_file() {
                return Some(candidate);
            }
        }
        None
    }
}

/// A probed target exists, so a failure to establish its canonical identity is
/// an input error. Falling back to the probed spelling could disagree with the
/// canonical source vertex and silently drop a cycle edge.
fn canonical_target(path: PathBuf) -> Result<PathBuf, ExtractError> {
    std::fs::canonicalize(&path).map_err(|source| ExtractError::IoError { path, source })
}

#[cfg(test)]
mod lexical_path_tests {
    use crate::tsconfig::normalize_module_path;
    use std::path::PathBuf;

    #[test]
    fn lexical_parents_preserve_relative_escape_and_absolute_roots() {
        for (input, expected) in [
            ("../x", "../x"),
            ("../../x", "../../x"),
            ("a/../../x", "../x"),
            ("/../x", "/x"),
            ("/a/../../x", "/x"),
            ("a/./b/../c", "a/c"),
        ] {
            assert_eq!(
                normalize_module_path(&PathBuf::from(input)),
                PathBuf::from(expected),
                "lexical module path {input}"
            );
        }
    }
}

/// `.d.ts`, `.d.mts` and `.d.cts` files hold only types.
fn is_declaration_file(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| {
            [".d.ts", ".d.mts", ".d.cts"]
                .iter()
                .any(|suffix| name.ends_with(suffix))
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_external_detection() {
        let extractor = ImportExtractor::new(vec!["ts".to_owned()], None);
        let target = extractor
            .resolve_import(Path::new("/project/src/index.ts"), "react")
            .expect("resolution within budget");
        assert!(
            matches!(target, ImportTarget::External(s) if s == "react"),
            "expected External for 'react'"
        );
    }

    #[test]
    fn test_scoped_package_detection() {
        let extractor = ImportExtractor::new(vec!["ts".to_owned()], None);
        let target = extractor
            .resolve_import(Path::new("/project/src/index.ts"), "@myorg/shared-utils")
            .expect("resolution within budget");
        assert!(
            matches!(target, ImportTarget::External(s) if s == "@myorg/shared-utils"),
            "expected External for scoped package"
        );
    }

    #[test]
    fn test_relative_import() {
        let extractor = ImportExtractor::new(vec!["ts".to_owned()], None);
        let target = extractor
            .resolve_import(Path::new("/project/src/index.ts"), "./utils")
            .expect("resolution within budget");
        // Relative specifiers are local targets, including when unresolved.
        assert!(
            !matches!(target, ImportTarget::External(_)),
            "relative import should not be External"
        );
    }

    #[test]
    fn test_reexport_extraction() {
        let code = r"export { helper } from './helper';
export * from './utils';";

        let extractor = ImportExtractor::new(vec!["ts".to_owned()], None);

        let imports = extractor
            .extract_source(code, Path::new("/test/file.ts"))
            .expect("valid extraction");

        let specifiers: Vec<_> = imports.iter().map(|i| i.specifier.as_str()).collect();
        assert!(
            specifiers.contains(&"./helper"),
            "should extract './helper' from named re-export, got: {specifiers:?}"
        );
        assert!(
            specifiers.contains(&"./utils"),
            "should extract './utils' from star re-export, got: {specifiers:?}"
        );
    }

    // =========================================================================
    // Additional import extraction tests for comprehensive coverage
    // =========================================================================

    /// Creates a test workspace with common packages.
    fn create_test_workspace() -> Workspace {
        use crate::workspace::WorkspaceFormat;
        use std::collections::HashMap;

        let mut packages = HashMap::new();
        let _ = packages.insert("@myorg/utils".to_owned(), PathBuf::from("/pkg/utils"));
        let _ = packages.insert("@myorg/shared".to_owned(), PathBuf::from("/pkg/shared"));
        let _ = packages.insert("common".to_owned(), PathBuf::from("/pkg/common"));

        Workspace {
            root: PathBuf::from("/"),
            format: WorkspaceFormat::Npm,
            packages,
            aliases: HashMap::new(),
        }
    }

    #[test]
    fn test_workspace_package_scoped_no_subpath() {
        let workspace = create_test_workspace();
        let extractor =
            ImportExtractor::new(vec!["ts".to_owned()], None).with_workspace(Some(workspace));

        let target = extractor
            .resolve_import(Path::new("/app/index.ts"), "@myorg/utils")
            .expect("resolution within budget");
        assert!(
            matches!(
                &target,
                ImportTarget::WorkspacePackage {
                    package_name,
                    subpath: None
                } if package_name == "@myorg/utils"
            ),
            "should detect @myorg/utils as workspace package, got: {target:?}"
        );
    }

    #[test]
    fn test_workspace_package_scoped_with_subpath() {
        let workspace = create_test_workspace();
        let extractor =
            ImportExtractor::new(vec!["ts".to_owned()], None).with_workspace(Some(workspace));

        let target = extractor
            .resolve_import(Path::new("/app/index.ts"), "@myorg/shared/helpers")
            .expect("resolution within budget");
        assert!(
            matches!(
                &target,
                ImportTarget::WorkspacePackage {
                    package_name,
                    subpath: Some(sub)
                } if package_name == "@myorg/shared" && sub == "helpers"
            ),
            "should detect @myorg/shared/helpers with subpath, got: {target:?}"
        );
    }

    #[test]
    fn test_workspace_package_unscoped() {
        let workspace = create_test_workspace();
        let extractor =
            ImportExtractor::new(vec!["ts".to_owned()], None).with_workspace(Some(workspace));

        let target = extractor
            .resolve_import(Path::new("/app/index.ts"), "common")
            .expect("resolution within budget");
        assert!(
            matches!(
                &target,
                ImportTarget::WorkspacePackage {
                    package_name,
                    subpath: None
                } if package_name == "common"
            ),
            "should detect common as workspace package, got: {target:?}"
        );
    }

    #[test]
    fn test_workspace_external_package() {
        let workspace = create_test_workspace();
        let extractor =
            ImportExtractor::new(vec!["ts".to_owned()], None).with_workspace(Some(workspace));

        let target = extractor
            .resolve_import(Path::new("/app/index.ts"), "react")
            .expect("resolution within budget");
        assert!(
            matches!(&target, ImportTarget::External(name) if name == "react"),
            "should detect react as external, got: {target:?}"
        );
    }

    #[test]
    fn test_workspace_package_deep_subpath() {
        use crate::workspace::WorkspaceFormat;
        use std::collections::HashMap;

        let mut packages = HashMap::new();
        let _ = packages.insert("@myorg/utils".to_owned(), PathBuf::from("/pkg/utils"));

        let ws = Workspace {
            root: PathBuf::from("/"),
            format: WorkspaceFormat::Npm,
            packages,
            aliases: HashMap::new(),
        };

        let extractor = ImportExtractor::new(vec!["ts".to_owned()], None).with_workspace(Some(ws));

        let target = extractor
            .resolve_import(Path::new("/app/index.ts"), "@myorg/utils/deep/nested/path")
            .expect("resolution within budget");
        assert!(
            matches!(
                &target,
                ImportTarget::WorkspacePackage {
                    package_name,
                    subpath: Some(sub)
                } if package_name == "@myorg/utils" && sub == "deep/nested/path"
            ),
            "should extract deep nested subpath, got: {target:?}"
        );
    }

    #[test]
    fn test_workspace_only_mode_skips_non_workspace_imports() {
        let workspace = create_test_workspace();
        let extractor = ImportExtractor::new(vec!["ts".to_owned()], None)
            .with_workspace(Some(workspace))
            .with_mode(ExtractionMode::WorkspaceOnly);

        let workspace_import = extractor
            .create_import_from_specifier_text(
                "\"@myorg/shared/helpers\"",
                Path::new("/app/index.ts"),
            )
            .expect("valid extraction");
        assert!(
            matches!(
                workspace_import.as_ref().map(|import| &import.target),
                Some(ImportTarget::WorkspacePackage {
                    package_name,
                    subpath: Some(subpath),
                }) if package_name == "@myorg/shared" && subpath == "helpers"
            ),
            "workspace-only mode should keep workspace imports, got: {workspace_import:?}"
        );

        let relative_import = extractor
            .create_import_from_specifier_text("\"./local-helper\"", Path::new("/app/index.ts"))
            .expect("valid extraction");
        assert!(
            relative_import.is_none(),
            "workspace-only mode should skip relative imports"
        );

        let external_import = extractor
            .create_import_from_specifier_text("\"react\"", Path::new("/app/index.ts"))
            .expect("valid extraction");
        assert!(
            external_import.is_none(),
            "workspace-only mode should skip external imports"
        );
    }

    #[test]
    fn test_import_target_equality() {
        let resolved1 = ImportTarget::Resolved(PathBuf::from("a.ts"));
        let resolved2 = ImportTarget::Resolved(PathBuf::from("a.ts"));
        let resolved3 = ImportTarget::Resolved(PathBuf::from("b.ts"));

        assert_eq!(resolved1, resolved2, "same resolved path should be equal");
        assert_ne!(
            resolved1, resolved3,
            "different resolved paths should not be equal"
        );

        let external1 = ImportTarget::External("react".to_owned());
        let external2 = ImportTarget::External("react".to_owned());
        let external3 = ImportTarget::External("vue".to_owned());

        assert_eq!(external1, external2, "same external should be equal");
        assert_ne!(
            external1, external3,
            "different externals should not be equal"
        );

        let ws1 = ImportTarget::WorkspacePackage {
            package_name: "@pkg/a".to_owned(),
            subpath: Some("utils".to_owned()),
        };
        let ws2 = ImportTarget::WorkspacePackage {
            package_name: "@pkg/a".to_owned(),
            subpath: Some("utils".to_owned()),
        };
        let ws3 = ImportTarget::WorkspacePackage {
            package_name: "@pkg/a".to_owned(),
            subpath: None,
        };

        assert_eq!(ws1, ws2, "same workspace package should be equal");
        assert_ne!(ws1, ws3, "different subpaths should not be equal");
    }

    #[test]
    fn test_import_clone() {
        let import = Import {
            source: PathBuf::from("index.ts"),
            target: ImportTarget::WorkspacePackage {
                package_name: "@myorg/utils".to_owned(),
                subpath: Some("helpers".to_owned()),
            },
            specifier: "@myorg/utils/helpers".to_owned(),
        };

        let cloned = import.clone();
        assert_eq!(cloned.source, import.source, "cloned source should match");
        assert_eq!(cloned.target, import.target, "cloned target should match");
        assert_eq!(
            cloned.specifier, import.specifier,
            "cloned specifier should match"
        );
    }

    #[test]
    fn test_no_workspace_external_detection() {
        // Without workspace, all non-relative imports should be external
        let extractor = ImportExtractor::new(vec!["ts".to_owned()], None);

        let target = extractor
            .resolve_import(Path::new("/app/index.ts"), "@myorg/utils")
            .expect("resolution within budget");
        assert!(
            matches!(target, ImportTarget::External(name) if name == "@myorg/utils"),
            "without workspace, should detect as external"
        );
    }

    #[test]
    fn test_with_ignore_patterns() {
        // This test ensures the builder pattern works without panicking
        let extractor = ImportExtractor::new(vec!["ts".to_owned()], None)
            .with_ignore_patterns(vec!["*.test.ts".to_owned(), "**/__tests__/**".to_owned()]);

        // Verify the extractor was created (ignore patterns are private, so we just verify construction)
        assert!(
            !extractor.extensions.is_empty(),
            "extractor should have extensions set"
        );
    }

    #[test]
    fn test_static_import_extraction() {
        let code = r"import { foo } from './foo';
import bar from './bar';
import * as utils from './utils';
import type { Type } from './types';";

        let extractor = ImportExtractor::new(vec!["ts".to_owned()], None);

        let imports = extractor
            .extract_source(code, Path::new("/test/file.ts"))
            .expect("valid extraction");

        let specifiers: Vec<_> = imports.iter().map(|i| i.specifier.as_str()).collect();
        assert!(
            specifiers.contains(&"./foo"),
            "should extract './foo', got: {specifiers:?}"
        );
        assert!(
            specifiers.contains(&"./bar"),
            "should extract './bar', got: {specifiers:?}"
        );
        assert!(
            specifiers.contains(&"./utils"),
            "should extract './utils', got: {specifiers:?}"
        );
    }

    #[test]
    fn test_dynamic_import_extraction() {
        let code = r"const mod = import('./dynamic');
const lazy = import('./lazy-module');
async function load() { return import('./async'); }";

        let extractor = ImportExtractor::new(vec!["ts".to_owned()], None);

        let imports = extractor
            .extract_source(code, Path::new("/test/file.ts"))
            .expect("valid extraction");

        let specifiers: Vec<_> = imports.iter().map(|i| i.specifier.as_str()).collect();
        assert!(
            specifiers.contains(&"./dynamic"),
            "should extract dynamic import './dynamic', got: {specifiers:?}"
        );
        assert!(
            specifiers.contains(&"./lazy-module"),
            "should extract dynamic import './lazy-module', got: {specifiers:?}"
        );
    }

    #[test]
    fn test_require_extraction() {
        let code = r"const a = require('./module-a');
const { b } = require('./module-b');
require('./side-effect');";

        let extractor = ImportExtractor::new(vec!["js".to_owned()], None);

        let imports = extractor
            .extract_source(code, Path::new("/test/file.js"))
            .expect("valid extraction");

        let specifiers: Vec<_> = imports.iter().map(|i| i.specifier.as_str()).collect();
        assert!(
            specifiers.contains(&"./module-a"),
            "should extract require './module-a', got: {specifiers:?}"
        );
        assert!(
            specifiers.contains(&"./module-b"),
            "should extract require './module-b', got: {specifiers:?}"
        );
    }

    #[test]
    fn test_mixed_import_styles() {
        let code = r"import { a } from './static';
export { b } from './reexport';
const c = import('./dynamic');
const d = require('./cjs');";

        let extractor = ImportExtractor::new(vec!["ts".to_owned()], None);

        let imports = extractor
            .extract_source(code, Path::new("/test/file.ts"))
            .expect("valid extraction");
        let specifiers: std::collections::BTreeSet<_> = imports
            .iter()
            .map(|import| import.specifier.as_str())
            .collect();
        assert_eq!(
            specifiers,
            ["./static", "./reexport", "./dynamic", "./cjs"]
                .into_iter()
                .collect(),
            "all four forms contribute their own edge"
        );
    }

    #[test]
    fn test_absolute_path_import() {
        let extractor = ImportExtractor::new(vec!["ts".to_owned()], None);

        // Absolute paths starting with / are relative to some base
        let target = extractor
            .resolve_import(Path::new("/project/src/index.ts"), "/utils")
            .expect("resolution within budget");
        assert!(
            !matches!(target, ImportTarget::External(_)),
            "absolute path starting with / should not be external"
        );
    }

    #[test]
    fn test_parent_directory_import() {
        let extractor = ImportExtractor::new(vec!["ts".to_owned()], None);

        let target = extractor
            .resolve_import(Path::new("/project/src/deep/index.ts"), "../utils")
            .expect("resolution within budget");
        assert!(
            !matches!(target, ImportTarget::External(_)),
            "parent directory import should not be external"
        );
    }

    #[test]
    fn alias_cache_payload_exhaustion_is_typed_for_existing_and_missing_targets() {
        use crate::budget::AnalysisLimits;

        for exists in [false, true] {
            let directory = tempfile::tempdir().expect("temporary directory");
            let root = directory.path();
            std::fs::write(
                root.join("tsconfig.json"),
                r#"{"compilerOptions":{"paths":{"@local/target":["target.ts"]}}}"#,
            )
            .expect("aliases");
            if exists {
                std::fs::write(root.join("target.ts"), "export const leaf = 1;").expect("target");
            }
            let config = TsConfig::load(&root.join("tsconfig.json")).expect("configuration");
            let budget = AnalysisBudget::new(AnalysisLimits {
                max_graph_bytes: 1,
                ..AnalysisLimits::default()
            })
            .expect("positive limits");
            let extractor = ImportExtractor::new(vec!["ts".to_owned()], Some(config))
                .with_budget(budget.clone());
            let ExtractError::Limit(error) = extractor
                .resolve_import(&root.join("entry.ts"), "@local/target")
                .expect_err("reserve alias payload before retaining the memo")
            else {
                panic!("payload admission must be a quota error");
            };
            assert_eq!(error.resource(), Resource::GraphBytes, "payload admission");
            assert_eq!(budget.check(), Err(error), "failure is sticky");
        }
    }
}
