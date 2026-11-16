# Upgrading to 0.5

0.5 changes only the Rust library; the command, its exit statuses and its JSON
output are unchanged. The changes bring the library in line with the
[Google Rust style guide](https://google.github.io/styleguide/rustguide.html)'s
code organization and API design rules for library code.

## One crate per part of the analysis

The `jscycles` crate is now only the command-line binary (`cargo install
jscycles`); its `cli` feature is gone. The library is split into small crates.
To embed a scan, depend on `jscycles-scan`, which re-exports the types its API
uses (`Config`, `ConfigError`, the cycle and result types, `ColorMode`,
`CycleFilter` and the error types):

```toml
[dependencies]
jscycles-scan = "0.5"
```

| 0.4 path | 0.5 crate |
| --- | --- |
| `jscycles::Scan`, `ScanReport`, `TsConfigSource`, `JscyclesError`, `jscycles::scan` | `jscycles-scan` |
| `jscycles::budget` | `jscycles-budget` |
| `jscycles::config`, `Config`, `ConfigError`, `DEFAULT_EXTENSIONS` | `jscycles-config` |
| `jscycles::discovery`, `PackageDiscovery`, `Package`, `Discovered`, `DiscoveryError` | `jscycles-discovery` |
| `jscycles::workspace`, `Workspace`, `WorkspaceFormat`, `WorkspaceError`, `WorkspaceYamlError` | `jscycles-workspace` |
| `jscycles::tsconfig`, `TsConfig`, `TsConfigCache`, `PathAlias`, `TsConfigError` | `jscycles-tsconfig` |
| `Import`, `ImportTarget`, `ExtractError` (were in `jscycles::imports` and the root) | `jscycles-import` |
| `ImportExtractor`, `ExtractionMode` (were in `jscycles::imports`) | `jscycles-extract` |
| `jscycles::graph`, `DependencyGraph`, `PackageDependencyGraph`, `EdgeEvidence` | `jscycles-graph` |
| `jscycles::cycles`, `Cycle`, `PackageCycle`, `PackageCycleEdge`, `PackageCycleWithFiles` | `jscycles-cycles` |
| `jscycles::output`, the formatters, `Results`, `UnifiedResults`, `PackageResult`, `CycleFilter`, `ColorMode` | `jscycles-output` |

The other crates (`jscycles-analysis`, `-input`, `-syntax`,
`-module-syntax`, `-module-path` and `-terminal`) were private modules. They
are published because the crates above depend on them; their APIs are
documented but exist to serve those crates.

`jscycles-graph`, `jscycles-cycles` and `jscycles-import` do not depend on the
native parser, so a caller that brings its own import graph no longer
compiles tree-sitter.

New in the crates' APIs: `UnifiedResults::has_cycles_for(filter)` (which
`ScanReport::has_cycles` uses), `ImportTarget::retained_bytes`,
`PathAlias::resolve`, and in `jscycles-analysis` the `Evidence` choice that
replaces a `bool` parameter. `jscycles-module-syntax`'s `visit` takes a
`TypeOnlyImports` choice instead of a `bool`.

## API changes

### Renamed and replaced

| 0.4 | 0.5 |
| --- | --- |
| `CycleDetector::detect(graph)` | `jscycles_cycles::file_cycles(graph)` |
| `CycleDetector::try_detect(graph, budget)` | `try_file_cycles(graph, budget)` |
| `CycleDetector::try_detect_imports(imports, budget)` | `try_file_cycles_from_imports(imports, budget)` |
| `PackageCycleDetector::detect(graph)` | `package_cycles(graph)` |
| `PackageCycleDetector::detect_with_files(graph)` | `package_cycles_with_files(graph)` |
| `PackageCycleDetector::try_detect(graph, budget)` | `try_package_cycles(graph, budget)` |
| `PackageCycleDetector::try_detect_imports(groups, budget)` | `try_package_cycles_from_imports(groups, budget)` |
| `PackageCycleDetector::try_detect_imports_with_files(groups, budget)` | `try_package_cycles_with_files_from_imports(groups, budget)` |
| `ScanReport::to_human(bool)` | `ScanReport::to_human(ColorMode::Plain \| ColorMode::Ansi)` |
| `HumanFormatter::new(bool)` | `HumanFormatter::new(ColorMode)`; its `use_color` and `verbose` fields are private |
| `HumanFormatter::with_verbose(v)` | `HumanFormatter::verbose(v)` |
| `Workspace::get_package_path` | `Workspace::package_path` |
| `Config::load_or_default` (removed) | `Config::try_load_or_default`, which reports unreadable or invalid files instead of silently using defaults |
| `AnalysisBudget::new(limits)` | `AnalysisBudget::try_new(limits)` |
| `PackageDiscovery::new(..)`, `new_with_budget(..)` | `PackageDiscovery::try_new(..)`, `try_new_with_budget(..)` |
| `TsConfigCache::new().with_budget(budget)` | `TsConfigCache::new_with_budget(budget)` |
| `TsConfigCache::rejected()` → `Vec<(PathBuf, String)>` | `TsConfigCache::ignored()` → `Vec<IgnoredTsConfig>` (`path`, `reason`) |
| `ScanReport::ignored_tsconfigs()` → `&[(PathBuf, String)]` | `&[IgnoredTsConfig]` |
| `EdgeEvidence::files()`, `PackageCycleEdge::files` as `(PathBuf, Option<String>)` | `EvidenceFile` (`source`, `subpath`) |
| `Workspace::split_specifier` → `Option<(&str, Option<&str>)>` | `Option<PackageSpecifier>` (`name`, `subpath`) |
| `PackageCycleEdge::with_evidence` | `PackageCycleEdge::new_with_evidence` |
| `PackageCycleWithFiles::from_cycle` | `PackageCycleWithFiles::new_from_cycle` |

The cycle functions are in `jscycles_cycles`. The detector structs held no
state, so the functions take the same arguments. Names follow the Google
style guide: fallible constructors are `try_new`, further constructors
`new_with_..`/`new_from_..`, and `with_..` is kept for closure accessors.

### The import extractor borrows

`ImportExtractor<'shared>` borrows the data every package's extractor shares,
instead of holding `Arc`s, and its builder methods drop the `with_` prefix:

| 0.4 | 0.5 |
| --- | --- |
| `ImportExtractor::new(extensions, Some(tsconfig))` | `ImportExtractor::new(extensions, Some(&tsconfig))` |
| `.with_workspace(Some(workspace))`, `.with_shared_workspace(Some(arc))` | `.workspace(Some(&workspace))` |
| `.with_shared_tsconfig(arc)` | pass `arc.as_deref()` to `new` |
| `.with_owners(arc)`, `.with_package_names(arc)` | `.owners(&roots)`, `.package_names(&names)` |
| `.with_budget`, `.with_mode`, `.with_ignore_patterns`, `.with_nested_packages`, `.with_skip_type_imports` | `.budget`, `.mode`, `.ignore_patterns`, `.nested_packages`, `.skip_type_imports` |

### Changed signatures

- `TsConfig::discover` returns `Result<Option<TsConfig>, TsConfigError>`.
  `Ok(None)` still means no `tsconfig.json` was found; a malformed or
  unreadable nearest config, which used to be `None`, is now an error. For the
  scan's best-effort behavior, which skips unusable files and lists them, use
  `TsConfigCache::discover` and `TsConfigCache::ignored`.
- `Config::extensions_for_package` returns `&[String]` and
  `Config::ignore_for_package` returns `Option<&[String]>` instead of cloned
  vectors. Call `.to_vec()` where an owned copy is needed.
- Functions that only iterate their input take iterators:
  `DependencyGraph::from_imports`, `cycles::try_file_cycles_from_imports` and
  `Indexed::imports` take any `IntoIterator` of imports (owned or borrowed);
  `PackageDependencyGraph::from_imports` takes `(name, imports)` pairs, so a
  `&HashMap<String, Vec<Import>>` still works; `PackageDiscovery`'s
  `discover_explicit*` and `discover_workspace_packages` take any
  `IntoIterator` of paths.
- `OutputFormatter::format` and `UnifiedOutputFormatter::format_unified` are
  generic over the writer (`W: Write + ?Sized`) instead of taking
  `&mut dyn Write`. Existing calls compile unchanged, including with a
  `&mut dyn Write`, but the traits can no longer be used as trait objects.
- These builder methods and accessors are no longer `const fn`:
  `Scan::skip_type_imports`, `filter` and `verbose`; `ScanReport::results` and
  `filter`; `ImportExtractor::mode`, `skip_type_imports`, `workspace`,
  `owners` and `package_names`; `HumanFormatter::new` and `verbose`;
  `UnifiedResults::has_package_cycles`; `DependencyGraph::nodes`,
  `PackageDependencyGraph::nodes` and `EdgeEvidence::file_count`;
  `BoundedReport::new`. Public `const fn` is a permanent promise, and these
  may need non-const work later.
- A poisoned lock in the tsconfig cache or the extractor now propagates the
  other thread's panic instead of reading the possibly half-updated state.

### Non-exhaustive types

These enums are `#[non_exhaustive]`; add a wildcard arm to matches outside the
crate: `Resource`, `ExtractionMode`, `ImportTarget`, `CycleFilter`,
`WorkspaceFormat` and the new `ColorMode`, `Evidence`, `Language` and
`TypeOnlyImports`.

These structs are `#[non_exhaustive]`. Their fields stay public for reading and
updating, but struct literals outside the crate no longer compile; use the
constructors:

| Type | Constructor |
| --- | --- |
| `Import` | `Import::new(source, target, specifier)` |
| `Cycle`, `PackageCycle` | `Cycle::new(path)`, `PackageCycle::new(packages)` |
| `PackageCycleEdge` | `PackageCycleEdge::new` or `new_with_evidence` |
| `PackageCycleWithFiles` | `PackageCycleWithFiles::new(packages, edges)` or `new_from_cycle` |
| `EvidenceFile` | `EvidenceFile::new(source, subpath)` |
| `PackageResult` | `PackageResult::new(name, path, cycles)` |
| `Results`, `UnifiedResults` | `new()`, then set or push to the fields |
| `Package` | `Package::new(name, path, config)` |
| `Workspace` | `Workspace::new(root, format, packages)`, then set `aliases` if needed |
| `TsConfig` | `TsConfig::new(base_url, paths)` |
| `PathAlias` | `PathAlias::new_exact(name, replacements)` or `PathAlias::new_wildcard(prefix, replacements)` |

`Config`, `ScanConfig` and `AnalysisLimits` stay exhaustive so that
`Config { limits, ..Config::default() }` keeps working; fields added later take
their defaults in that form.

### Additions

- `ColorMode`, `EvidenceFile`, `IgnoredTsConfig` and `PackageSpecifier`.
- `impl FromStr for Config` (the same as `Config::parse`) and
  `impl FromIterator<Import> for DependencyGraph`, for owned or borrowed
  imports.
- Structural `PartialEq`/`Eq` for the result, configuration, discovery,
  workspace, tsconfig and graph types, and `Hash` for the small value types
  and enums, so they can be compared and used as keys.
- `Clone` for the graphs, `PackageDiscovery`, `HumanFormatter`,
  `JsonFormatter` (also `Copy`) and `BoundedReport`; `Default` is derived for
  `Results`.
- `Debug` for `ImportExtractor` (settings only) and `BoundedReport`.
- `#[must_use]` on `TsConfigCache::discover_shared` and both graphs'
  `edge_count`.
- Runnable documentation examples for `AnalysisBudget`, `PackageDiscovery`,
  `ImportExtractor`, the cycle functions, `Config`, `HumanFormatter`,
  `Workspace` and `TsConfig`.
