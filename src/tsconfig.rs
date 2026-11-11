//! TypeScript configuration parsing for path alias resolution.
//!
//! Handles loading `tsconfig.json` and resolving path aliases like `@/utils`.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};

use serde::Deserialize;

use crate::TsConfigError;
use crate::budget::{AnalysisBudget, Resource, units};

/// Internal struct for parsing tsconfig.json.
#[derive(Debug, Deserialize)]
struct TsConfigJson {
    /// Parent config(s) to extend. TypeScript 5.0+ accepts an array, applied
    /// in order with later entries overriding earlier ones.
    extends: Option<Extends>,

    /// Compiler options section.
    #[serde(rename = "compilerOptions")]
    compiler_options: Option<CompilerOptions>,
}

/// `extends` as a single path or (TypeScript 5.0+) an ordered list of paths.
#[derive(Debug, Deserialize)]
#[serde(
    untagged,
    expecting = "`extends` must be a path string or an array of path strings"
)]
enum Extends {
    One(String),
    Many(Vec<String>),
}

impl Extends {
    fn entries(&self) -> &[String] {
        match self {
            Self::One(path) => std::slice::from_ref(path),
            Self::Many(paths) => paths,
        }
    }
}

/// Internal struct for compiler options.
#[derive(Debug, Default, Deserialize, Clone)]
struct CompilerOptions {
    #[serde(skip)]
    paths_base: Option<PathBuf>,
    /// Base URL for module resolution.
    #[serde(rename = "baseUrl")]
    base_url: Option<PathBuf>,

    /// Path alias mappings.
    paths: Option<HashMap<String, Vec<String>>>,
}

impl CompilerOptions {
    fn selected_copy(&self, fields: InheritedFields) -> Self {
        Self {
            paths_base: None,
            base_url: if fields.base_url {
                self.base_url.clone()
            } else {
                None
            },
            paths: if fields.paths {
                self.paths.clone()
            } else {
                None
            },
        }
    }

    /// Merge with parent options. Self (child) takes precedence.
    fn merge_with_parent(self, parent_opts: Self) -> Self {
        Self {
            paths_base: if self.paths.is_some() {
                self.paths_base
            } else {
                parent_opts.paths_base
            },
            base_url: self.base_url.or(parent_opts.base_url),
            paths: self.paths.or(parent_opts.paths),
        }
    }
}

/// A compiled path alias pattern.
#[derive(Debug, Clone)]
pub struct PathAlias {
    /// The pattern prefix (e.g., "@/" from "@/*").
    pub prefix: String,

    /// Whether this pattern has a wildcard.
    pub has_wildcard: bool,

    /// Replacement paths (resolved relative to `base_url`).
    pub replacements: Vec<PathBuf>,
}

impl PathAlias {
    fn priority(&self) -> (bool, std::cmp::Reverse<usize>, &str) {
        (
            self.has_wildcard,
            std::cmp::Reverse(self.prefix.len()),
            &self.prefix,
        )
    }
}

/// Parsed TypeScript configuration for path resolution.
#[derive(Debug, Clone)]
pub struct TsConfig {
    /// Base URL for path resolution.
    pub base_url: PathBuf,

    /// Compiled path alias patterns.
    pub paths: Vec<PathAlias>,
}

/// Shared cache for tsconfig discovery and loading.
#[derive(Debug, Clone, Default)]
pub struct TsConfigCache {
    /// Memoized mapping from searched directories to discovered tsconfig paths.
    dir_cache: Arc<Mutex<HashMap<PathBuf, CachedDirResult>>>,
    /// Memoized parsed tsconfig contents keyed by file path.
    config_cache: Arc<Mutex<HashMap<PathBuf, Arc<TsConfig>>>>,
    documents: Arc<DocumentCache>,
    /// Automatically selected configs that failed to load, with the reason.
    rejected: Arc<Mutex<std::collections::BTreeMap<PathBuf, String>>>,
    budget: AnalysisBudget,
}

type DocumentEntry = Arc<OnceLock<Option<Arc<TsConfigJson>>>>;

/// Share parsed bytes/values, never merged options tied to a particular child.
/// Initializers parse one document only, so cyclic extends cannot wait on itself.
#[derive(Debug, Default)]
struct DocumentCache {
    entries: Mutex<HashMap<PathBuf, DocumentEntry>>,
}

impl DocumentCache {
    fn load(
        &self,
        path: &Path,
        budget: &AnalysisBudget,
    ) -> Result<Arc<TsConfigJson>, TsConfigError> {
        budget.check()?;
        budget.consume(Resource::Work, 1)?;
        let key = std::fs::canonicalize(path).map_err(TsConfigError::Read)?;
        let entry = {
            let mut entries = self
                .entries
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if let Some(entry) = entries.get(&key) {
                Arc::clone(entry)
            } else {
                budget.consume(Resource::GraphBytes, units(key.as_os_str().len()))?;
                budget.consume(
                    Resource::GraphBytes,
                    units(size_of::<OnceLock<Option<Arc<TsConfigJson>>>>()),
                )?;
                let entry = Arc::new(OnceLock::new());
                let _ = entries.insert(key, Arc::clone(&entry));
                entry
            }
        };
        // Release the map lock before I/O. OnceLock coordinates only this file;
        // unrelated documents can load concurrently and failed init keeps cause.
        let mut failure = None;
        let document = entry.get_or_init(|| {
            match read_document(path, budget).and_then(|document| {
                admit_document(&document, budget)?;
                Ok(Arc::new(document))
            }) {
                Ok(document) => Some(document),
                Err(error) => {
                    failure = Some(error);
                    None
                },
            }
        });
        if let Some(document) = document {
            budget.check()?;
            Ok(Arc::clone(document))
        } else if let Some(error) = failure {
            Err(error)
        } else {
            // Failed parses are not fabricated/shared errors. Retry through the
            // typed reader for this caller, unless a quota is already exhausted.
            budget.check()?;
            read_document(path, budget).map(Arc::new)
        }
    }
}

#[derive(Clone, Copy)]
struct InheritedFields {
    base_url: bool,
    paths: bool,
}

impl InheritedFields {
    const ALL: Self = Self {
        base_url: true,
        paths: true,
    };

    fn parent(self, child: Option<&CompilerOptions>) -> Self {
        Self {
            base_url: self.base_url && child.is_none_or(|options| options.base_url.is_none()),
            paths: self.paths && child.is_none_or(|options| options.paths.is_none()),
        }
    }
}

fn absolute_path(path: &Path) -> std::io::Result<PathBuf> {
    std::path::absolute(if path.as_os_str().is_empty() {
        Path::new(".")
    } else {
        path
    })
}

/// Parent components must follow the selected filesystem directory. Walking
/// syntactic parents of `caller/../pkg` can otherwise descend back into caller.
/// Leave ordinary spellings alone, including their existing symlink context.
fn resolved_search_path(path: PathBuf) -> std::io::Result<PathBuf> {
    if path
        .components()
        .any(|component| component == std::path::Component::ParentDir)
    {
        std::fs::canonicalize(path)
    } else {
        Ok(path)
    }
}

#[cfg_attr(feature = "profiling", hotpath::measure)]
fn read_document(path: &Path, budget: &AnalysisBudget) -> Result<TsConfigJson, TsConfigError> {
    let contents = crate::input::read_text_budgeted(path, budget).map_err(TsConfigError::Read)?;
    let normalized = crate::syntax::jsonc(&contents).map_err(|message| {
        TsConfigError::Read(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            message,
        ))
    })?;
    serde_json::from_str(&normalized).map_err(TsConfigError::Parse)
}

fn admit_document(
    document: &TsConfigJson,
    budget: &AnalysisBudget,
) -> Result<(), crate::budget::LimitError> {
    budget.consume(Resource::GraphBytes, units(size_of::<TsConfigJson>()))?;
    for entry in document.extends.iter().flat_map(Extends::entries) {
        budget.consume(Resource::Work, 1)?;
        budget.consume(Resource::GraphBytes, units(entry.len()))?;
    }
    if let Some(options) = &document.compiler_options {
        budget.consume(
            Resource::GraphBytes,
            units(
                options
                    .base_url
                    .as_ref()
                    .map_or(0, |path| path.as_os_str().len()),
            ),
        )?;
        if let Some(paths) = &options.paths {
            budget.consume(Resource::Work, units(paths.len()))?;
            for (pattern, replacements) in paths {
                budget.consume(
                    Resource::GraphBytes,
                    units(size_of::<(String, Vec<String>)>()),
                )?;
                budget.consume(Resource::GraphBytes, units(pattern.len()))?;
                for replacement in replacements {
                    budget.consume(Resource::Work, 1)?;
                    budget.consume(Resource::GraphBytes, units(size_of::<String>()))?;
                    budget.consume(Resource::GraphBytes, units(replacement.len()))?;
                }
            }
        }
    }
    Ok(())
}

fn select_child_options(
    document: Arc<TsConfigJson>,
    fields: InheritedFields,
    origin: &Path,
) -> CompilerOptions {
    let mut options = match Arc::try_unwrap(document) {
        Ok(document) => {
            let mut options = document.compiler_options.unwrap_or_default();
            if !fields.paths {
                options.paths = None;
            }
            if !fields.base_url {
                options.base_url = None;
            }
            options
        },
        Err(document) => document
            .compiler_options
            .as_ref()
            .map_or_else(CompilerOptions::default, |options| {
                options.selected_copy(fields)
            }),
    };
    options.paths_base = fields.paths.then(|| origin.to_path_buf());
    options.base_url = options.base_url.map(|base| origin.join(base));
    options
}

/// Cached result for a tsconfig discovery lookup.
#[derive(Debug, Clone)]
enum CachedDirResult {
    /// A tsconfig was found at this path.
    Found(PathBuf),
    /// No tsconfig exists for this directory or its parents.
    Missing,
}

/// Maximum depth for tsconfig extends chain to prevent infinite loops.
const MAX_EXTENDS_DEPTH: usize = 10;

/// Whether a module path names a directory explicitly: it ends in a separator
/// or its final segment is `.`/`..`. Node and TypeScript resolve such a
/// specifier only as a directory (index file), never as `<name>.ts`. Lexical
/// normalization loses this fact, so callers must test the raw spelling.
pub(crate) fn names_directory(path: &Path) -> bool {
    spelled_directory(path.as_os_str().as_encoded_bytes(), cfg!(windows), true)
}

/// Whether a `paths` substitution names a directory. TypeScript appends
/// extensions to a substituted `.`/`..` (so `"@root": ["."]` can select a
/// sibling `<dir>.ts`); only a trailing separator forces directory lookup.
pub(crate) fn ends_with_separator(path: &Path) -> bool {
    spelled_directory(path.as_os_str().as_encoded_bytes(), cfg!(windows), false)
}

/// Host-independent core of [`names_directory`] so both separator policies are
/// tested on every platform. A backslash separates components only on Windows.
fn spelled_directory(bytes: &[u8], windows: bool, dot_segments: bool) -> bool {
    let last = bytes
        .rsplit(|byte| *byte == b'/' || (windows && *byte == b'\\'))
        .next()
        .unwrap_or(bytes);
    last.is_empty() || (dot_segments && (last == b"." || last == b".."))
}

#[cfg(test)]
mod spelled_directory_tests {
    use super::spelled_directory;

    #[test]
    fn separators_and_dot_segments_name_directories_per_platform() {
        for windows in [false, true] {
            for spelled in ["./b/", ".", "..", "a/.", "a/..", ".//"] {
                assert!(
                    spelled_directory(spelled.as_bytes(), windows, true),
                    "{spelled}"
                );
            }
            for spelled in ["./b", "b", "a/.b", "a/..b", "./b.ts"] {
                assert!(
                    !spelled_directory(spelled.as_bytes(), windows, true),
                    "{spelled}"
                );
            }
            // `paths` substitutions: only a trailing separator.
            assert!(spelled_directory(b"src/lib/", windows, false));
            for spelled in [".", "..", "src/.", "src/lib"] {
                assert!(
                    !spelled_directory(spelled.as_bytes(), windows, false),
                    "{spelled}"
                );
            }
        }
        // A trailing backslash is a separator only on Windows; elsewhere it is
        // an ordinary file-name byte.
        assert!(spelled_directory(br".\b\", true, true));
        assert!(!spelled_directory(br".\b\", false, true));
        assert!(spelled_directory(br"a\..", true, true));
        assert!(!spelled_directory(br"a\..", false, true));
    }
}

/// JS/TS module resolution collapses dot segments before filesystem lookup.
/// Keep native origin bytes and root/prefix semantics; this lexical operation
/// neither follows symlinks nor provides filesystem containment.
pub(crate) fn normalize_module_path(path: &Path) -> PathBuf {
    use std::path::Component;
    let mut components: Vec<Component<'_>> = Vec::new();
    for component in path.components() {
        match component {
            Component::CurDir => {},
            Component::ParentDir if matches!(components.last(), Some(Component::Normal(_))) => {
                let _ = components.pop();
            },
            Component::ParentDir if matches!(components.last(), Some(Component::RootDir)) => {},
            comp @ (Component::Prefix(_)
            | Component::RootDir
            | Component::ParentDir
            | Component::Normal(_)) => components.push(comp),
        }
    }
    components.iter().collect()
}

impl TsConfig {
    /// Loaded tables have unique exact entries before wildcard entries. Check
    /// that sufficient invariant once before bypassing a name search. Public
    /// vectors with different ordering, duplicates or empty replacements retain
    /// the existing first-matching-pattern behavior through the general lookup.
    pub(crate) fn can_resolve_exact_aliases_directly(&self) -> bool {
        self.paths
            .iter()
            .all(|alias| !alias.replacements.is_empty())
            && self
                .paths
                .iter()
                .is_sorted_by(|left, right| left.priority() < right.priority())
    }

    /// Load and parse a tsconfig.json file.
    ///
    /// Handles `extends` field by recursively loading parent configs and merging.
    ///
    /// # Errors
    ///
    /// Returns an error if the file cannot be read or parsed.
    #[inline]
    pub fn load(path: &Path) -> Result<Self, TsConfigError> {
        Self::load_with_budget(path, &AnalysisBudget::default())
    }

    /// Load a configuration under the caller's shared scan quotas.
    /// # Errors
    /// Returns input, parse, extends, or quota errors.
    #[cfg_attr(feature = "profiling", hotpath::measure)]
    pub fn load_with_budget(
        selected_path: &Path,
        budget: &AnalysisBudget,
    ) -> Result<Self, TsConfigError> {
        Self::load_with_documents(selected_path, budget, None)
    }

    #[cfg_attr(feature = "profiling", hotpath::measure)]
    fn load_with_documents(
        selected_path: &Path,
        budget: &AnalysisBudget,
        documents: Option<&DocumentCache>,
    ) -> Result<Self, TsConfigError> {
        let canonical = std::fs::canonicalize(selected_path)
            .map_err(|source| TsConfigError::Read(source).at_path(selected_path))?;
        let path = canonical.as_path();
        let tsconfig_dir = path
            .parent()
            .map_or_else(|| PathBuf::from("."), Path::to_path_buf);

        // Load and merge all compiler options from extends chain
        let compiler_options =
            Self::load_merged_options(path, 0, budget, documents, InheritedFields::ALL)?;

        let base_url = compiler_options.base_url.unwrap_or_else(|| {
            compiler_options
                .paths_base
                .unwrap_or_else(|| tsconfig_dir.clone())
        });

        if let Some(mappings) = &compiler_options.paths {
            crate::budget::sorting(mappings.len(), budget)?;
            budget.consume(Resource::Work, units(mappings.len()))?;
            for (pattern, replacements) in mappings {
                if (pattern.contains('*')
                    && (!pattern.ends_with('*') || pattern.matches('*').count() != 1))
                    || replacements.is_empty()
                {
                    return Err(TsConfigError::InvalidPath("paths requires nonempty replacements and exact or single trailing-wildcard patterns".to_owned()));
                }
                budget.consume(Resource::GraphBytes, units(pattern.len()))?;
                for replacement in replacements {
                    let stars = replacement.matches('*').count();
                    if stars > 1
                        || (pattern.ends_with('*') && stars != 1)
                        || (!pattern.ends_with('*') && stars != 0)
                    {
                        return Err(TsConfigError::InvalidPath("supported paths substitutions must pair exact patterns with exact targets or trailing-wildcard patterns with single-wildcard targets".to_owned()));
                    }
                    budget.consume(Resource::Work, 1)?;
                    budget.consume(Resource::GraphBytes, units(base_url.as_os_str().len()))?;
                    budget.consume(Resource::GraphBytes, units(replacement.len()))?;
                }
            }
        }
        let paths = compiler_options
            .paths
            .map(|mappings| Self::compile_path_mappings(mappings, &base_url))
            .unwrap_or_default();

        Ok(Self { base_url, paths })
    }

    /// Load and merge compiler options from tsconfig and its extends chain.
    fn load_merged_options(
        path: &Path,
        depth: usize,
        budget: &AnalysisBudget,
        documents: Option<&DocumentCache>,
        fields: InheritedFields,
    ) -> Result<CompilerOptions, TsConfigError> {
        Self::load_options_at(path, depth, budget, documents, fields)
            .map_err(|source| source.at_path(path))
    }

    fn load_options_at(
        path: &Path,
        depth: usize,
        budget: &AnalysisBudget,
        documents: Option<&DocumentCache>,
        fields: InheritedFields,
    ) -> Result<CompilerOptions, TsConfigError> {
        if depth >= MAX_EXTENDS_DEPTH {
            return Err(TsConfigError::ExtendsDepth);
        }

        let json = match documents {
            Some(cache) => cache.load(path, budget)?,
            None => Arc::new(read_document(path, budget)?),
        };

        let tsconfig_dir = path
            .parent()
            .map_or_else(|| PathBuf::from("."), Path::to_path_buf);

        // Load parent options if extends is specified
        let parent_fields = fields.parent(json.compiler_options.as_ref());
        let parent_options = Self::load_parent_options(
            json.extends.as_ref(),
            &tsconfig_dir,
            depth,
            budget,
            documents,
            parent_fields,
        )?;

        // Merge child options with parent (child takes precedence)
        let child_options = select_child_options(json, fields, &tsconfig_dir);
        Ok(child_options.merge_with_parent(parent_options))
    }

    /// Load parent tsconfig options if extends is specified. Array entries are
    /// applied in order: each later parent overrides the ones before it, and
    /// the extending config (merged by the caller) overrides them all.
    fn load_parent_options(
        extends: Option<&Extends>,
        tsconfig_dir: &Path,
        depth: usize,
        budget: &AnalysisBudget,
        documents: Option<&DocumentCache>,
        fields: InheritedFields,
    ) -> Result<CompilerOptions, TsConfigError> {
        let mut merged = CompilerOptions::default();
        let Some(extends) = extends else {
            return Ok(merged);
        };

        let next_depth = depth.checked_add(1).ok_or(TsConfigError::ExtendsDepth)?;
        for extends_path in extends.entries() {
            // Each entry is a separate load; fan-out across levels is charged
            // to the shared work quota rather than bounded by depth alone.
            budget.consume(Resource::Work, 1)?;
            let parent_path = Self::resolve_extends_path(extends_path, tsconfig_dir, budget)?;
            let parent =
                Self::load_merged_options(&parent_path, next_depth, budget, documents, fields)?;
            merged = parent.merge_with_parent(merged);
        }
        Ok(merged)
    }

    /// Resolve the extends path to an absolute path.
    ///
    /// Relative and absolute paths name a file (with `.json` appended when
    /// missing). A package specifier is looked up in the nearest enclosing
    /// `node_modules` that resolves it, the way TypeScript's config lookup
    /// does: a package's `exports` (string, subpath map, condition map, or
    /// array; conditions `node`, `require`, `types`, `default`; one `*`
    /// pattern) decide alone when present; otherwise a subpath names a file or
    /// directory, and a bare package uses its `tsconfig` field or
    /// `tsconfig.json`.
    fn resolve_extends_path(
        extends: &str,
        tsconfig_dir: &Path,
        budget: &AnalysisBudget,
    ) -> Result<PathBuf, TsConfigError> {
        if extends.starts_with('.') || Path::new(extends).is_absolute() {
            let resolved = tsconfig_dir.join(extends);
            return Self::resolve_tsconfig_file(&resolved);
        }
        let (package, subpath) = split_package_specifier(extends);
        let mut current = Some(tsconfig_dir);
        while let Some(dir) = current {
            budget.consume(Resource::Work, 1)?;
            let package_dir = dir.join("node_modules").join(package);
            if package_dir.is_dir()
                && let Some(resolved) = Self::resolve_in_package(&package_dir, subpath, budget)?
            {
                return Ok(resolved);
            }
            current = dir.parent();
        }

        Err(TsConfigError::ExtendsNotFound(extends.to_owned()))
    }

    /// Resolve `subpath` (empty for the bare package) inside an installed package.
    fn resolve_in_package(
        package_dir: &Path,
        subpath: &str,
        budget: &AnalysisBudget,
    ) -> Result<Option<PathBuf>, TsConfigError> {
        let manifest = read_manifest(&package_dir.join("package.json"), budget)?;
        if let Some(exports) = manifest.as_ref().and_then(|value| value.exports.as_ref()) {
            let key = if subpath.is_empty() {
                ".".to_owned()
            } else {
                format!("./{subpath}")
            };
            return Ok(resolve_exports(exports, &key, package_dir, budget)?);
        }
        if subpath.is_empty() {
            if let Some(Ordered::String(field)) =
                manifest.as_ref().and_then(|value| value.tsconfig.as_ref())
            {
                return Ok(Self::resolve_tsconfig_file(&package_dir.join(field)).ok());
            }
            let default = package_dir.join("tsconfig.json");
            return Ok(default.is_file().then_some(default));
        }
        let target = package_dir.join(subpath);
        if let Ok(file) = Self::resolve_config_file(&target) {
            return Ok(Some(file));
        }
        if target.is_dir() {
            return Self::resolve_in_package(&target, "", budget);
        }
        Ok(None)
    }

    /// A config file named exactly, or with `.json` appended.
    fn resolve_config_file(path: &Path) -> Result<PathBuf, TsConfigError> {
        if path.is_file() {
            return Ok(path.to_path_buf());
        }
        let mut json_name = path.as_os_str().to_os_string();
        json_name.push(".json");
        let with_json = PathBuf::from(json_name);
        if with_json.is_file() {
            return Ok(with_json);
        }
        Err(TsConfigError::ExtendsNotFound(
            path.to_string_lossy().into_owned(),
        ))
    }

    /// Resolve a path to an actual tsconfig file (handles missing .json extension).
    fn resolve_tsconfig_file(path: &Path) -> Result<PathBuf, TsConfigError> {
        // Try exact path first
        if path.is_file() {
            return Ok(path.to_path_buf());
        }

        // Try with .json extension
        let mut json_name = path.as_os_str().to_os_string();
        json_name.push(".json");
        let with_json = PathBuf::from(json_name);
        if with_json.is_file() {
            return Ok(with_json);
        }

        // Try as directory with tsconfig.json inside
        let as_dir = path.join("tsconfig.json");
        if as_dir.is_file() {
            return Ok(as_dir);
        }

        Err(TsConfigError::ExtendsNotFound(
            path.to_string_lossy().into_owned(),
        ))
    }

    /// Compile path mappings from tsconfig into `PathAlias` structs.
    fn compile_path_mappings(
        mappings: HashMap<String, Vec<String>>,
        base_url: &Path,
    ) -> Vec<PathAlias> {
        // Sort keys for deterministic iteration
        let mut entries: Vec<_> = mappings.into_iter().collect();
        entries.sort_by(|a, b| {
            let priority =
                |pattern: &str| (pattern.ends_with('*'), std::cmp::Reverse(pattern.len()));
            priority(&a.0)
                .cmp(&priority(&b.0))
                .then_with(|| a.0.cmp(&b.0))
        });

        entries
            .into_iter()
            .map(|(pattern, replacements)| Self::compile_alias(&pattern, replacements, base_url))
            .collect()
    }

    /// Compile a single alias pattern and its replacements.
    fn compile_alias(pattern: &str, replacements: Vec<String>, base_url: &Path) -> PathAlias {
        let (prefix, has_wildcard) = Self::parse_pattern(pattern);
        let resolved_replacements = Self::resolve_replacements(replacements, base_url);

        PathAlias {
            prefix,
            has_wildcard,
            replacements: resolved_replacements,
        }
    }

    /// Parse a pattern into prefix and wildcard flag.
    fn parse_pattern(pattern: &str) -> (String, bool) {
        if pattern.ends_with('*') {
            (
                pattern.strip_suffix('*').unwrap_or(pattern).to_owned(),
                true,
            )
        } else {
            (pattern.to_owned(), false)
        }
    }

    /// Resolve replacement paths relative to `base_url`.
    fn resolve_replacements(replacements: Vec<String>, base_url: &Path) -> Vec<PathBuf> {
        replacements.into_iter().map(|r| base_url.join(r)).collect()
    }

    /// Discover tsconfig.json by searching from the given directory upward.
    ///
    /// Returns `None` if no tsconfig.json is found, or if the nearest one (or an
    /// inherited config) cannot be read or parsed: automatic discovery is
    /// best-effort. Use [`TsConfig::load`] for strict errors.
    #[inline]
    pub fn discover(start: &Path) -> Option<Self> {
        let start = absolute_path(start).ok()?;
        let mut current = if start.is_file() {
            start.parent().map(Path::to_path_buf)
        } else {
            Some(start)
        };

        while let Some(dir) = current {
            let dir = resolved_search_path(dir).ok()?;
            let tsconfig_path = dir.join("tsconfig.json");
            if tsconfig_path.exists() {
                return Self::load(&tsconfig_path).ok();
            }
            current = dir.parent().map(Path::to_path_buf);
        }

        None
    }

    /// Resolve a path alias to a file or its first declared target candidate.
    ///
    /// Returns `None` if the specifier doesn't match any configured alias.
    /// When no replacement exists, retains the first candidate for declarative
    /// workspace membership; callers requiring a file must check its existence.
    #[inline]
    #[must_use]
    pub fn resolve_alias(&self, specifier: &str) -> Option<PathBuf> {
        for alias in &self.paths {
            if let Some(resolved) = Self::try_resolve_with_alias(specifier, alias) {
                return Some(resolved);
            }
        }
        None
    }

    /// Try to resolve a specifier with a specific alias.
    pub(crate) fn try_resolve_with_alias(specifier: &str, alias: &PathAlias) -> Option<PathBuf> {
        let suffix = if alias.has_wildcard {
            specifier.strip_prefix(&alias.prefix)?
        } else if specifier == alias.prefix {
            ""
        } else {
            return None;
        };
        let mut first = None;
        for replacement in &alias.replacements {
            let resolved = if alias.has_wildcard
                && let Some(substituted) = Self::substitute_wildcard(replacement, suffix)
            {
                substituted
            } else if suffix.is_empty() {
                replacement.clone()
            } else {
                replacement.join(suffix)
            };
            // A trailing separator may come from the specifier or from the
            // configured replacement (`"@x/*": ["src/*/"]`); either one makes
            // the target a directory, as in TypeScript.
            let directory = ends_with_separator(&resolved) || ends_with_separator(replacement);
            let resolved = normalize_module_path(&resolved);
            let file = if directory {
                None
            } else {
                Self::try_resolve_with_extensions(&resolved)
            };
            if let Some(file) = file.or_else(|| Self::try_resolve_as_directory(&resolved)) {
                return Some(file);
            }
            if first.is_none() {
                first = Some(resolved);
            }
        }
        first
    }

    /// The configured replacement has one marker; its origin may contain literal
    /// asterisks or non-UTF-8 components. Substitute the last marked component
    /// without converting the native origin path to a Unicode string.
    fn substitute_wildcard(replacement: &Path, suffix: &str) -> Option<PathBuf> {
        for marked in replacement.ancestors() {
            let Some((prefix, tail)) = marked
                .file_name()
                .and_then(|name| name.to_str())
                .and_then(|name| name.rsplit_once('*'))
            else {
                continue;
            };
            // Push each `/`-separated suffix segment as its own component.
            // Pasting `a/b` into one component is wrong on Windows verbatim
            // (`\\?\`) bases, where `/` is not a separator.
            let mut segments = suffix.split('/');
            let mut name = std::ffi::OsString::from(prefix);
            name.push(segments.next().unwrap_or_default());
            let mut resolved = marked.parent()?.to_path_buf();
            for segment in segments {
                resolved.push(&name);
                name = std::ffi::OsString::from(segment);
            }
            name.push(tail);
            resolved.push(name);
            let remaining = replacement.strip_prefix(marked).ok()?;
            if !remaining.as_os_str().is_empty() {
                resolved.push(remaining);
            }
            return Some(resolved);
        }
        None
    }

    /// Shared file-candidate policy for aliases and relative source imports.
    /// Exact files win. Substitute known source extensions; otherwise append
    /// extensions to the complete stem, preserving dotted names and native bytes.
    pub(crate) fn try_resolve_with_extensions(base: &Path) -> Option<PathBuf> {
        let extension = base.extension().and_then(|extension| extension.to_str());
        let candidates: &[&str] = match extension {
            // TypeScript's order: sources, then declarations, then JavaScript.
            Some("mjs" | "mts") => &["", ".mts", ".d.mts", ".mjs"],
            Some("cjs" | "cts") => &["", ".cts", ".d.cts", ".cjs"],
            Some("jsx") => &["", ".tsx", ".ts", ".d.ts", ".js"],
            _ => &["", ".ts", ".tsx", ".d.ts", ".js", ".jsx"],
        };
        for ext in candidates {
            let candidate = if ext.is_empty() {
                base.to_path_buf()
            } else {
                // Known source extensions retain substitution semantics; an
                // arbitrary dotted suffix belongs to the complete module stem.
                if matches!(
                    extension,
                    Some("ts" | "tsx" | "js" | "jsx" | "mts" | "mjs" | "cts" | "cjs")
                ) {
                    base.with_extension(ext.trim_start_matches('.'))
                } else {
                    let mut name = base.as_os_str().to_os_string();
                    name.push(ext);
                    PathBuf::from(name)
                }
            };
            if candidate.is_file() {
                return Some(candidate);
            }
        }
        None
    }

    /// Try to resolve a path as a directory with an index file.
    fn try_resolve_as_directory(path: &Path) -> Option<PathBuf> {
        if !path.is_dir() {
            return None;
        }
        for index_name in &[
            "index.ts",
            "index.tsx",
            "index.d.ts",
            "index.js",
            "index.jsx",
        ] {
            let index_path = path.join(index_name);
            if index_path.is_file() {
                return Some(index_path);
            }
        }
        None
    }
}

impl TsConfigCache {
    /// Create a new empty cache.
    #[inline]
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Share a caller-owned budget across metadata discovery and loading.
    /// Starts empty caches so all retained values are admitted to that budget.
    #[must_use]
    pub fn with_budget(self, budget: AnalysisBudget) -> Self {
        Self {
            budget,
            ..Self::default()
        }
    }

    /// Selected tsconfig files that failed to load and were ignored, sorted by
    /// path, with each error message. Their path aliases were not applied, so
    /// callers should surface these rather than present a silent clean result.
    #[must_use]
    pub fn rejected(&self) -> Vec<(PathBuf, String)> {
        self.rejected
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .iter()
            .map(|(path, error)| (path.clone(), error.clone()))
            .collect()
    }

    /// Discover a tsconfig for a path, caching both lookup results and parsed configs.
    /// Returns an owned clone of the cached aliases; [`Self::discover_shared`]
    /// shares the snapshot without copying its mappings.
    /// A failed copy admission returns None and leaves the shared budget exhausted.
    #[inline]
    #[must_use]
    pub fn discover(&self, start: &Path) -> Option<TsConfig> {
        let snapshot = self.discover_shared(start)?;
        self.clone_snapshot(&snapshot)
    }

    /// Discover an immutable shared snapshot. Optional metadata remains best-effort;
    /// callers must check the shared budget before accepting a scan result.
    #[cfg_attr(feature = "profiling", hotpath::measure)]
    pub fn discover_shared(&self, start: &Path) -> Option<Arc<TsConfig>> {
        self.budget.check().ok()?;
        let start = absolute_path(start).ok()?;
        let mut current = if start.is_file() {
            start.parent().map(Path::to_path_buf)
        } else {
            Some(start)
        };
        let mut visited = Vec::new();

        while let Some(dir) = current {
            self.budget.consume(Resource::Work, 1).ok()?;
            self.budget
                .consume(Resource::GraphBytes, units(dir.as_os_str().len()))
                .ok()?;
            if let Some(cached) = self.cached_dir_result(&dir) {
                self.cache_dir_results(&visited, cached.path())?;
                let path = cached.path()?;
                return self.load_cached(path);
            }

            let search_dir = resolved_search_path(dir.clone()).ok()?;
            if search_dir != dir {
                visited.push(dir);
            }

            let tsconfig_path = search_dir.join("tsconfig.json");
            if tsconfig_path.exists() {
                visited.push(search_dir);
                self.cache_dir_results(&visited, Some(&tsconfig_path))?;
                return self.load_cached(&tsconfig_path);
            }

            current = search_dir.parent().map(Path::to_path_buf);
            visited.push(search_dir);
        }

        self.cache_dir_results(&visited, None)?;
        None
    }

    /// Load a specific tsconfig path using the parsed-config cache.
    /// Returns an owned copy of the cached configuration and alias mappings.
    /// A failed copy admission returns None and leaves the shared budget exhausted.
    #[inline]
    #[must_use]
    pub fn load(&self, path: &Path) -> Option<TsConfig> {
        let snapshot = self.load_cached(path)?;
        self.clone_snapshot(&snapshot)
    }

    /// Admit all known owned payloads before cloning; shared snapshots do not
    /// incur these copy reservations. Capacity/allocator overhead is not exact RSS.
    fn clone_snapshot(&self, snapshot: &TsConfig) -> Option<TsConfig> {
        self.budget
            .consume(Resource::GraphBytes, units(size_of::<TsConfig>()))
            .ok()?;
        self.budget
            .consume(
                Resource::GraphBytes,
                units(snapshot.base_url.as_os_str().len()),
            )
            .ok()?;
        for alias in &snapshot.paths {
            self.budget.consume(Resource::Work, 1).ok()?;
            self.budget
                .consume(Resource::GraphBytes, units(size_of::<PathAlias>()))
                .ok()?;
            self.budget
                .consume(Resource::GraphBytes, units(alias.prefix.len()))
                .ok()?;
            for replacement in &alias.replacements {
                self.budget.consume(Resource::Work, 1).ok()?;
                self.budget
                    .consume(Resource::GraphBytes, units(size_of::<PathBuf>()))
                    .ok()?;
                self.budget
                    .consume(Resource::GraphBytes, units(replacement.as_os_str().len()))
                    .ok()?;
            }
        }
        Some(snapshot.clone())
    }

    /// Get a cached directory lookup result, if any.
    fn cached_dir_result(&self, dir: &Path) -> Option<CachedDirResult> {
        self.dir_cache
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get(dir)
            .cloned()
    }

    /// Cache discovery results for a list of visited directories.
    fn cache_dir_results(&self, dirs: &[PathBuf], result: Option<&Path>) -> Option<()> {
        let mut cache = self
            .dir_cache
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        for dir in dirs {
            let result_bytes = result.map_or(0, |path| path.as_os_str().len());
            self.budget.consume(Resource::Work, 1).ok()?;
            self.budget
                .consume(Resource::GraphBytes, units(dir.as_os_str().len()))
                .ok()?;
            self.budget
                .consume(Resource::GraphBytes, units(result_bytes))
                .ok()?;
            let value = result.map_or(CachedDirResult::Missing, |path| {
                CachedDirResult::Found(path.to_path_buf())
            });
            let _ = cache.insert(dir.clone(), value);
        }
        drop(cache);
        Some(())
    }

    /// Load a parsed tsconfig from cache or disk.
    fn load_cached(&self, selected_path: &Path) -> Option<Arc<TsConfig>> {
        self.budget.check().ok()?;
        let absolute = absolute_path(selected_path).ok()?;
        let path = absolute.as_path();
        let cached_config = self
            .config_cache
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get(path)
            .cloned();
        if let Some(config) = cached_config {
            return Some(config);
        }

        self.budget
            .consume(Resource::GraphBytes, units(path.as_os_str().len()))
            .ok()?;
        let config = match TsConfig::load_with_documents(path, &self.budget, Some(&self.documents))
        {
            Ok(config) => Arc::new(config),
            Err(error) => {
                // Exhaustion fails the whole operation; only report ordinary
                // bad metadata, which optional discovery tolerates.
                if self.budget.check().is_ok() {
                    let _ = self
                        .rejected
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner)
                        .insert(path.to_path_buf(), error.to_string());
                }
                return None;
            },
        };
        let mut cache = self
            .config_cache
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(existing) = cache.get(path) {
            return Some(Arc::clone(existing));
        }
        let _ = cache.insert(path.to_path_buf(), Arc::clone(&config));
        drop(cache);
        Some(config)
    }
}

impl CachedDirResult {
    /// Get the discovered path, if one exists.
    fn path(&self) -> Option<&Path> {
        match self {
            Self::Found(path) => Some(path.as_path()),
            Self::Missing => None,
        }
    }
}

#[cfg(test)]
mod selection_tests {
    use super::*;

    #[test]
    fn owned_and_shared_document_selection_preserves_values_and_snapshot_ownership() {
        let text = r#"{"compilerOptions":{"baseUrl":"base","paths":{"@value":["target"]}}}"#;
        let original: TsConfigJson = serde_json::from_str(text).expect("document");
        let original = Arc::new(original);
        for fields in [
            InheritedFields::ALL,
            InheritedFields {
                base_url: false,
                paths: true,
            },
            InheritedFields {
                base_url: true,
                paths: false,
            },
            InheritedFields {
                base_url: false,
                paths: false,
            },
        ] {
            let origin = Path::new("config-origin");
            let shared = select_child_options(Arc::clone(&original), fields, origin);
            let owned = select_child_options(
                Arc::new(serde_json::from_str(text).expect("document")),
                fields,
                origin,
            );
            assert_eq!(
                shared.base_url, owned.base_url,
                "ownership does not alter baseUrl"
            );
            assert_eq!(shared.paths, owned.paths, "ownership does not alter paths");
            assert_eq!(
                shared.base_url,
                fields.base_url.then(|| origin.join("base")),
                "apply this load's origin"
            );
            assert_eq!(
                shared.paths.is_some(),
                fields.paths,
                "copy only a still-needed option"
            );
            let mut changed = shared;
            if let Some(paths) = &mut changed.paths {
                paths.clear();
            }
            assert_eq!(
                original
                    .compiler_options
                    .as_ref()
                    .expect("options")
                    .paths
                    .as_ref()
                    .expect("original paths")
                    .len(),
                1,
                "a child's owned options cannot mutate the raw snapshot"
            );
        }
    }

    #[test]
    fn child_overrides_select_parent_fields_independently_including_empty_paths() {
        for (text, base_url, paths) in [
            ("{}", true, true),
            (r#"{"compilerOptions":{"baseUrl":"."}}"#, false, true),
            (r#"{"compilerOptions":{"paths":{}}}"#, true, false),
            (
                r#"{"compilerOptions":{"baseUrl":".","paths":{}}}"#,
                false,
                false,
            ),
        ] {
            let document: TsConfigJson = serde_json::from_str(text).expect("document");
            let parent = InheritedFields::ALL.parent(document.compiler_options.as_ref());
            assert_eq!(
                (parent.base_url, parent.paths),
                (base_url, paths),
                "child options replace these complete parent fields"
            );
        }
    }
}

/// `@scope/name/rest` -> (`@scope/name`, `rest`); `name/rest` -> (`name`, `rest`).
fn split_package_specifier(specifier: &str) -> (&str, &str) {
    let mut boundaries = specifier.match_indices('/').map(|(index, _)| index);
    let first = boundaries.next();
    let end = if specifier.starts_with('@') {
        boundaries.next()
    } else {
        first
    };
    end.and_then(|index| {
        Some((
            specifier.get(..index)?,
            specifier.get(index.checked_add(1)?..)?,
        ))
    })
    .unwrap_or((specifier, ""))
}

/// A JSON value whose objects keep their key order. `exports` condition maps
/// are tried in the order they are written, and `serde_json`'s own map sorts.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Ordered {
    String(String),
    Array(Vec<Self>),
    Object(Vec<(String, Self)>),
    /// Numbers, booleans and null: never a target.
    Other,
}

impl<'de> Deserialize<'de> for Ordered {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct OrderedVisitor;

        impl<'de> serde::de::Visitor<'de> for OrderedVisitor {
            type Value = Ordered;

            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("a JSON value")
            }

            fn visit_str<E: serde::de::Error>(self, value: &str) -> Result<Ordered, E> {
                Ok(Ordered::String(value.to_owned()))
            }

            fn visit_string<E: serde::de::Error>(self, value: String) -> Result<Ordered, E> {
                Ok(Ordered::String(value))
            }

            fn visit_bool<E: serde::de::Error>(self, _value: bool) -> Result<Ordered, E> {
                Ok(Ordered::Other)
            }

            fn visit_i64<E: serde::de::Error>(self, _value: i64) -> Result<Ordered, E> {
                Ok(Ordered::Other)
            }

            fn visit_u64<E: serde::de::Error>(self, _value: u64) -> Result<Ordered, E> {
                Ok(Ordered::Other)
            }

            fn visit_f64<E: serde::de::Error>(self, _value: f64) -> Result<Ordered, E> {
                Ok(Ordered::Other)
            }

            fn visit_unit<E: serde::de::Error>(self) -> Result<Ordered, E> {
                Ok(Ordered::Other)
            }

            fn visit_seq<A: serde::de::SeqAccess<'de>>(
                self,
                mut seq: A,
            ) -> Result<Ordered, A::Error> {
                let mut items = Vec::new();
                while let Some(item) = seq.next_element()? {
                    items.push(item);
                }
                Ok(Ordered::Array(items))
            }

            fn visit_map<A: serde::de::MapAccess<'de>>(
                self,
                mut map: A,
            ) -> Result<Ordered, A::Error> {
                let mut entries: Vec<(String, Ordered)> = Vec::new();
                while let Some((key, value)) = map.next_entry::<String, Ordered>()? {
                    // JSON.parse keeps the last duplicate at the first's position.
                    if let Some(entry) = entries.iter_mut().find(|(name, _)| *name == key) {
                        entry.1 = value;
                    } else {
                        entries.push((key, value));
                    }
                }
                Ok(Ordered::Object(entries))
            }
        }

        deserializer.deserialize_any(OrderedVisitor)
    }
}

#[cfg(test)]
mod ordered_tests {
    use super::Ordered;
    use serde::Deserialize;
    use serde::de::value::{BytesDeserializer, Error};

    #[test]
    fn keeps_written_key_order_and_names_what_it_expects() {
        let value: Ordered =
            serde_json::from_str(r#"{"b": 1, "a": ["x"], "b": "y"}"#).expect("valid JSON");
        assert_eq!(
            value,
            Ordered::Object(vec![
                ("b".to_owned(), Ordered::String("y".to_owned())),
                (
                    "a".to_owned(),
                    Ordered::Array(vec![Ordered::String("x".to_owned())])
                ),
            ])
        );
        // JSON never yields bytes; another deserializer reaches `expecting`.
        let error = Ordered::deserialize(BytesDeserializer::<Error>::new(b"x"))
            .expect_err("bytes are not a JSON value");
        assert!(error.to_string().contains("a JSON value"), "{error}");
    }
}

/// The `package.json` fields config lookup reads.
#[derive(Deserialize)]
struct InstalledManifest {
    exports: Option<Ordered>,
    tsconfig: Option<Ordered>,
}

/// An installed package's manifest, if it has one. A manifest that exists but
/// cannot be read or parsed rejects the config rather than being skipped.
fn read_manifest(
    path: &Path,
    budget: &AnalysisBudget,
) -> Result<Option<InstalledManifest>, TsConfigError> {
    if !path.is_file() {
        return Ok(None);
    }
    let text = crate::input::read_text_budgeted(path, budget).map_err(TsConfigError::Read)?;
    serde_json::from_str(&text)
        .map(Some)
        .map_err(|error| TsConfigError::Parse(error).at_path(path))
}

/// Conditions TypeScript's config lookup accepts, in addition to `default`.
const EXPORT_CONDITIONS: [&str; 4] = ["node", "require", "types", "default"];

/// Resolve `key` (`.` or `./sub`) through a package's `exports`.
fn resolve_exports(
    exports: &Ordered,
    key: &str,
    package_dir: &Path,
    budget: &AnalysisBudget,
) -> Result<Option<PathBuf>, crate::budget::LimitError> {
    let subpaths = match exports {
        Ordered::Object(entries) if entries.iter().all(|(name, _)| name.starts_with('.')) => {
            Some(entries)
        },
        Ordered::Object(_) | Ordered::String(_) | Ordered::Array(_) | Ordered::Other => None,
    };
    let Some(entries) = subpaths else {
        // A string, array or condition map is the `.` entry.
        return if key == "." {
            resolve_export_target(exports, None, package_dir, budget)
        } else {
            Ok(None)
        };
    };
    if let Some((_, target)) = entries.iter().find(|(name, _)| name == key) {
        return resolve_export_target(target, None, package_dir, budget);
    }
    // The longest matching `*` pattern wins.
    let best = entries
        .iter()
        .filter_map(|(pattern, target)| {
            let (prefix, suffix) = pattern.split_once('*')?;
            let middle = key.strip_prefix(prefix)?.strip_suffix(suffix)?;
            Some((prefix.len(), middle, target))
        })
        .max_by_key(|(length, _, _)| *length);
    match best {
        Some((_, middle, target)) => {
            resolve_export_target(target, Some(middle), package_dir, budget)
        },
        None => Ok(None),
    }
}

fn resolve_export_target(
    target: &Ordered,
    star: Option<&str>,
    package_dir: &Path,
    budget: &AnalysisBudget,
) -> Result<Option<PathBuf>, crate::budget::LimitError> {
    budget.consume(Resource::Work, 1)?;
    match target {
        Ordered::String(text) => {
            let Some(relative) = text.strip_prefix("./") else {
                return Ok(None);
            };
            let relative = star.map_or_else(
                || relative.to_owned(),
                |middle| relative.replace('*', middle),
            );
            let file = package_dir.join(relative);
            // Config lookup accepts only an existing `.json` target.
            Ok(
                (file.extension().is_some_and(|ext| ext == "json") && file.is_file())
                    .then_some(file),
            )
        },
        Ordered::Array(items) => {
            for item in items {
                if let Some(found) = resolve_export_target(item, star, package_dir, budget)? {
                    return Ok(Some(found));
                }
            }
            Ok(None)
        },
        Ordered::Object(conditions) => {
            // In written order, as TypeScript and Node try them.
            for (condition, value) in conditions {
                if EXPORT_CONDITIONS.contains(&condition.as_str())
                    && let Some(found) = resolve_export_target(value, star, package_dir, budget)?
                {
                    return Ok(Some(found));
                }
            }
            Ok(None)
        },
        Ordered::Other => Ok(None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    use tempfile::tempdir;

    #[test]
    fn test_parse_tsconfig() {
        let json = r#"{
            "compilerOptions": {
                "baseUrl": ".",
                "paths": {
                    "@/*": ["src/*"],
                    "~/utils": ["src/utils"]
                }
            }
        }"#;

        let parsed: TsConfigJson =
            serde_json::from_str(json).expect("test json should parse successfully");
        assert!(parsed.compiler_options.is_some());

        let opts = parsed
            .compiler_options
            .expect("compiler options should exist in test");
        assert_eq!(opts.base_url, Some(PathBuf::from(".")));
        assert!(opts.paths.is_some());
    }

    #[test]
    fn test_wildcard_detection() {
        let pattern = "@/*";
        let has_wildcard = pattern.ends_with('*');
        assert!(has_wildcard);

        let prefix = pattern.strip_suffix('*').unwrap_or(pattern);
        assert_eq!(prefix, "@/");
    }

    // =========================================================================
    // Additional tsconfig tests for comprehensive coverage
    // =========================================================================

    #[test]
    fn test_parse_pattern_with_wildcard_at() {
        let (prefix, has_wildcard) = TsConfig::parse_pattern("@/*");
        assert_eq!(prefix, "@/", "prefix should be @/");
        assert!(has_wildcard, "should detect wildcard");
    }

    #[test]
    fn test_parse_pattern_with_wildcard_components() {
        let (prefix, has_wildcard) = TsConfig::parse_pattern("@components/*");
        assert_eq!(prefix, "@components/", "prefix should be @components/");
        assert!(has_wildcard, "should detect wildcard");
    }

    #[test]
    fn test_parse_pattern_with_wildcard_tilde() {
        let (prefix, has_wildcard) = TsConfig::parse_pattern("~/*");
        assert_eq!(prefix, "~/", "prefix should be ~/");
        assert!(has_wildcard, "should detect wildcard");
    }

    #[test]
    fn test_parse_pattern_without_wildcard_utils() {
        let (prefix, has_wildcard) = TsConfig::parse_pattern("@utils");
        assert_eq!(prefix, "@utils", "prefix should be @utils");
        assert!(!has_wildcard, "should not detect wildcard");
    }

    #[test]
    fn test_parse_pattern_without_wildcard_path() {
        let (prefix, has_wildcard) = TsConfig::parse_pattern("exact/path");
        assert_eq!(prefix, "exact/path", "prefix should be exact/path");
        assert!(!has_wildcard, "should not detect wildcard");
    }

    #[test]
    fn test_resolve_replacements() {
        let base_url = PathBuf::from("/project");
        let replacements = vec!["src/*".to_owned(), "lib/*".to_owned()];
        let resolved = TsConfig::resolve_replacements(replacements, &base_url);

        assert_eq!(resolved.len(), 2, "should have 2 resolved paths");
        assert_eq!(resolved.first(), Some(&PathBuf::from("/project/src/*")));
        assert_eq!(resolved.get(1), Some(&PathBuf::from("/project/lib/*")));
    }

    #[test]
    fn test_resolve_replacements_no_wildcard() {
        let base_url = PathBuf::from("/project");
        let replacements = vec!["src/utils".to_owned()];
        let resolved = TsConfig::resolve_replacements(replacements, &base_url);

        assert_eq!(resolved.len(), 1, "should have 1 resolved path");
        assert_eq!(resolved.first(), Some(&PathBuf::from("/project/src/utils")));
    }

    #[test]
    fn test_try_resolve_with_alias_exact_match() {
        let alias = PathAlias {
            prefix: "@utils".to_owned(),
            has_wildcard: false,
            replacements: vec![PathBuf::from("/project/src/utils")],
        };

        let result_exact = TsConfig::try_resolve_with_alias("@utils", &alias);
        assert_eq!(
            result_exact,
            Some(PathBuf::from("/project/src/utils")),
            "exact match should resolve"
        );

        let result_partial = TsConfig::try_resolve_with_alias("@utils/foo", &alias);
        assert_eq!(
            result_partial, None,
            "partial match should not resolve exact alias"
        );
    }

    #[test]
    fn test_compile_alias() {
        let base_url = PathBuf::from("/project");
        let alias = TsConfig::compile_alias("@/*", vec!["src/*".to_owned()], &base_url);

        assert_eq!(alias.prefix, "@/", "prefix should be @/");
        assert!(alias.has_wildcard, "should have wildcard");
        assert_eq!(alias.replacements.len(), 1, "should have 1 replacement");
    }

    #[test]
    fn test_compile_path_mappings_ordering() {
        let mut mappings = HashMap::new();
        let _ = mappings.insert("z/*".to_owned(), vec!["z/*".to_owned()]);
        let _ = mappings.insert("a/*".to_owned(), vec!["a/*".to_owned()]);
        let _ = mappings.insert("m/*".to_owned(), vec!["m/*".to_owned()]);

        let base_url = PathBuf::from("/");
        let aliases = TsConfig::compile_path_mappings(mappings, &base_url);

        // Should be sorted alphabetically
        assert_eq!(aliases.len(), 3, "should have 3 aliases");
        assert_eq!(aliases.first().map(|a| a.prefix.as_str()), Some("a/"));
        assert_eq!(aliases.get(1).map(|a| a.prefix.as_str()), Some("m/"));
        assert_eq!(aliases.get(2).map(|a| a.prefix.as_str()), Some("z/"));
    }

    #[test]
    fn test_tsconfig_clone() {
        let config = TsConfig {
            base_url: PathBuf::from("/project"),
            paths: vec![PathAlias {
                prefix: "@/".to_owned(),
                has_wildcard: true,
                replacements: vec![PathBuf::from("/project/src/")],
            }],
        };

        let cloned = config.clone();
        assert_eq!(cloned.base_url, config.base_url, "base_url should match");
        assert_eq!(cloned.paths.len(), config.paths.len(), "paths should match");
    }

    #[test]
    fn test_path_alias_clone() {
        let alias = PathAlias {
            prefix: "@utils/".to_owned(),
            has_wildcard: true,
            replacements: vec![PathBuf::from("/a/"), PathBuf::from("/b/")],
        };

        let cloned = alias.clone();
        assert_eq!(cloned.prefix, alias.prefix, "prefix should match");
        assert_eq!(
            cloned.has_wildcard, alias.has_wildcard,
            "wildcard should match"
        );
        assert_eq!(
            cloned.replacements.len(),
            alias.replacements.len(),
            "replacements should match"
        );
    }

    #[test]
    fn test_resolve_alias_no_match() {
        let config = TsConfig {
            base_url: PathBuf::from("/project"),
            paths: vec![PathAlias {
                prefix: "@/".to_owned(),
                has_wildcard: true,
                replacements: vec![PathBuf::from("/project/src/")],
            }],
        };

        let result_local = config.resolve_alias("./local");
        assert_eq!(result_local, None, "relative import should not match alias");

        let result_react = config.resolve_alias("react");
        assert_eq!(result_react, None, "external import should not match alias");

        let result_tilde = config.resolve_alias("~/utils");
        assert_eq!(result_tilde, None, "different prefix should not match");
    }

    #[test]
    fn test_resolve_alias_with_multiple_patterns() {
        let config = TsConfig {
            base_url: PathBuf::from("/project"),
            paths: vec![
                PathAlias {
                    prefix: "@/".to_owned(),
                    has_wildcard: true,
                    replacements: vec![PathBuf::from("/project/src/")],
                },
                PathAlias {
                    prefix: "@components/".to_owned(),
                    has_wildcard: true,
                    replacements: vec![PathBuf::from("/project/components/")],
                },
            ],
        };

        let result_utils = config.resolve_alias("@/utils");
        assert!(result_utils.is_some(), "should match @/ alias");
        assert!(
            result_utils
                .as_ref()
                .is_some_and(|p| p.starts_with("/project/src")),
            "should resolve to src"
        );

        let result_component = config.resolve_alias("@components/Button");
        assert!(
            result_component.is_some(),
            "should match @components/ alias"
        );
        assert!(
            result_component
                .as_ref()
                .is_some_and(|p| p.starts_with("/project/components")),
            "should resolve to components"
        );
    }

    #[test]
    fn test_compiler_options_merge() {
        let parent = CompilerOptions {
            paths_base: None,
            base_url: Some(PathBuf::from("parent_base")),
            paths: Some({
                let mut m = HashMap::new();
                let _ = m.insert("@parent/*".to_owned(), vec!["parent/*".to_owned()]);
                m
            }),
        };

        let child = CompilerOptions {
            paths_base: None,
            base_url: Some(PathBuf::from("child_base")),
            paths: Some({
                let mut m = HashMap::new();
                let _ = m.insert("@child/*".to_owned(), vec!["child/*".to_owned()]);
                m
            }),
        };

        let merged = child.merge_with_parent(parent);

        assert_eq!(
            merged.base_url,
            Some(PathBuf::from("child_base")),
            "child base_url takes precedence"
        );
        let paths = merged.paths.expect("merged should have paths");
        assert!(
            !paths.contains_key("@parent/*"),
            "child paths replace the inherited option"
        );
        assert!(paths.contains_key("@child/*"), "should have child paths");
    }

    #[test]
    fn test_compiler_options_merge_child_only() {
        let parent = CompilerOptions::default();

        let child = CompilerOptions {
            paths_base: None,
            base_url: Some(PathBuf::from("child_base")),
            paths: Some({
                let mut m = HashMap::new();
                let _ = m.insert("@/*".to_owned(), vec!["src/*".to_owned()]);
                m
            }),
        };

        let merged = child.merge_with_parent(parent);

        assert_eq!(
            merged.base_url,
            Some(PathBuf::from("child_base")),
            "should have child base_url"
        );
        assert!(merged.paths.is_some(), "should have child paths");
    }

    #[test]
    fn test_compiler_options_merge_parent_only() {
        let parent = CompilerOptions {
            paths_base: None,
            base_url: Some(PathBuf::from("parent_base")),
            paths: Some({
                let mut m = HashMap::new();
                let _ = m.insert("@/*".to_owned(), vec!["lib/*".to_owned()]);
                m
            }),
        };

        let child = CompilerOptions::default();
        let merged = child.merge_with_parent(parent);

        assert_eq!(
            merged.base_url,
            Some(PathBuf::from("parent_base")),
            "should inherit parent base_url"
        );
        assert!(merged.paths.is_some(), "should inherit parent paths");
    }

    #[test]
    fn test_compiler_options_default() {
        let opts = CompilerOptions::default();
        assert!(opts.base_url.is_none(), "default has no base_url");
        assert!(opts.paths.is_none(), "default has no paths");
    }

    #[test]
    fn test_tsconfig_error_extends_depth() {
        let err = TsConfigError::ExtendsDepth;
        let msg = format!("{err}");
        assert!(
            msg.contains("too deep"),
            "error message should mention depth"
        );
    }

    #[test]
    fn test_tsconfig_error_extends_not_found() {
        let err = TsConfigError::ExtendsNotFound("./missing.json".to_owned());
        let msg = format!("{err}");
        assert!(
            msg.contains("missing.json"),
            "error message should contain path"
        );
    }

    // =========================================================================
    // Extends chain tests
    // =========================================================================

    #[test]
    fn test_extends_relative_path_resolution() {
        // Test that relative extends paths work
        let fixture_path =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/tsconfig-extends");

        let config_opt = TsConfig::discover(&fixture_path);
        assert!(
            config_opt.is_some(),
            "should discover tsconfig with extends"
        );

        let config = config_opt.expect("config should exist");

        // The child's paths option replaces inherited mappings.
        assert!(
            !config.paths.is_empty(),
            "should have paths from extends chain"
        );
    }

    #[test]
    fn test_extends_child_paths_replace_parent_aliases() {
        let fixture_path =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/tsconfig-extends");

        let config = TsConfig::discover(&fixture_path).expect("should discover config");

        // Only the child's mappings survive the option replacement.
        let alias_prefixes: Vec<&str> = config.paths.iter().map(|a| a.prefix.as_str()).collect();

        assert!(
            alias_prefixes.contains(&"@app/"),
            "should have @app/* from child config"
        );
        assert!(
            !alias_prefixes.contains(&"@shared/"),
            "child paths replace parent aliases"
        );
        assert!(
            !alias_prefixes.contains(&"@base/"),
            "child paths replace grandparent aliases"
        );
    }

    #[test]
    fn test_extends_from_node_modules() {
        let fixture_path =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/tsconfig-extends-pkg");

        let config_opt = TsConfig::discover(&fixture_path);
        assert!(
            config_opt.is_some(),
            "should discover tsconfig extending from node_modules"
        );

        let config = config_opt.expect("config should exist");

        // The local config's paths replace the company's inherited paths.
        let alias_prefixes: Vec<&str> = config.paths.iter().map(|a| a.prefix.as_str()).collect();

        assert!(
            alias_prefixes.contains(&"@local/"),
            "should have @local/* from child config"
        );
        assert!(
            !alias_prefixes.contains(&"@company/"),
            "child paths replace the inherited paths option"
        );
    }

    #[test]
    fn test_extends_child_overrides_parent_same_alias() {
        // When child and parent define the same alias, child should win
        let child_paths =
            Some(std::iter::once(("@alias/*".to_owned(), vec!["child/*".to_owned()])).collect());
        let parent_paths =
            Some(std::iter::once(("@alias/*".to_owned(), vec!["parent/*".to_owned()])).collect());

        let child_opts = CompilerOptions {
            paths_base: None,
            base_url: Some(PathBuf::from(".")),
            paths: child_paths,
        };
        let parent_opts = CompilerOptions {
            paths_base: None,
            base_url: Some(PathBuf::from(".")),
            paths: parent_paths,
        };

        let merged = child_opts.merge_with_parent(parent_opts);
        let paths = merged.paths.expect("should have paths");

        let alias_target = paths.get("@alias/*").expect("should have @alias/*");
        assert_eq!(
            alias_target.first().map(String::as_str),
            Some("child/*"),
            "child should override parent for same alias"
        );
    }

    #[test]
    fn test_extends_base_url_inherited() {
        // When child doesn't specify baseUrl, should inherit from parent
        let parent_opts = CompilerOptions {
            paths_base: None,
            base_url: Some(PathBuf::from("src")),
            paths: None,
        };
        let child_opts = CompilerOptions {
            paths_base: None,
            base_url: None,
            paths: Some(std::iter::once(("@/*".to_owned(), vec!["*".to_owned()])).collect()),
        };

        let merged = child_opts.merge_with_parent(parent_opts);

        assert_eq!(
            merged.base_url.as_deref(),
            Some(Path::new("src")),
            "child should inherit baseUrl from parent"
        );
    }

    #[test]
    fn test_extends_base_url_child_overrides() {
        // When child specifies baseUrl, should override parent
        let parent_opts = CompilerOptions {
            paths_base: None,
            base_url: Some(PathBuf::from("parent-src")),
            paths: None,
        };
        let child_opts = CompilerOptions {
            paths_base: None,
            base_url: Some(PathBuf::from("child-src")),
            paths: None,
        };

        let merged = child_opts.merge_with_parent(parent_opts);

        assert_eq!(
            merged.base_url.as_deref(),
            Some(Path::new("child-src")),
            "child should override parent baseUrl"
        );
    }

    #[test]
    fn test_extends_paths_replace_the_inherited_option() {
        // A child paths option replaces the entire inherited option.
        let parent_paths =
            Some(std::iter::once(("@parent/*".to_owned(), vec!["parent/*".to_owned()])).collect());
        let child_paths =
            Some(std::iter::once(("@child/*".to_owned(), vec!["child/*".to_owned()])).collect());

        let parent_opts = CompilerOptions {
            paths_base: None,
            base_url: Some(PathBuf::from(".")),
            paths: parent_paths,
        };
        let child_opts = CompilerOptions {
            paths_base: None,
            base_url: Some(PathBuf::from(".")),
            paths: child_paths,
        };

        let merged = child_opts.merge_with_parent(parent_opts);
        let paths = merged.paths.expect("should have paths");

        assert!(!paths.contains_key("@parent/*"), "parent alias is replaced");
        assert!(paths.contains_key("@child/*"), "should retain child alias");
    }

    #[test]
    fn test_tsconfig_cache_discovers_shared_parent_config() {
        let temp = tempdir().expect("tempdir should be created");
        let root = temp.path();
        fs::create_dir_all(root.join("packages/a/src")).expect("package a dirs should exist");
        fs::create_dir_all(root.join("packages/b/src")).expect("package b dirs should exist");
        fs::write(
            root.join("tsconfig.json"),
            r#"{
  "compilerOptions": {
    "baseUrl": ".",
    "paths": {
      "@/*": ["src/*"]
    }
  }
}"#,
        )
        .expect("tsconfig should be written");

        let cache = TsConfigCache::new();
        let first = cache.discover(&root.join("packages/a"));
        let second = cache.discover(&root.join("packages/b"));

        assert!(first.is_some(), "first lookup should find tsconfig");
        assert!(
            second.is_some(),
            "second lookup should reuse cached discovery"
        );
        assert_eq!(
            first.expect("checked above").base_url,
            second.expect("checked above").base_url,
            "cached lookups should return equivalent configs"
        );
    }
}
