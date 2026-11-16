# Upgrading to 0.5

0.5 changes only the Rust library API; the command, its exit statuses and its
JSON output are unchanged. The changes bring the public API in line with the
[Google Rust style guide](https://google.github.io/styleguide/rustguide.html)'s
API design rules for library code. `Scan` callers that only read results need
no changes except `to_human`.

## Renamed and replaced

| 0.4 | 0.5 |
| --- | --- |
| `CycleDetector::detect(graph)` | `cycles::file_cycles(graph)` |
| `CycleDetector::try_detect(graph, budget)` | `cycles::try_file_cycles(graph, budget)` |
| `CycleDetector::try_detect_imports(imports, budget)` | `cycles::try_file_cycles_from_imports(imports, budget)` |
| `PackageCycleDetector::detect(graph)` | `cycles::package_cycles(graph)` |
| `PackageCycleDetector::detect_with_files(graph)` | `cycles::package_cycles_with_files(graph)` |
| `PackageCycleDetector::try_detect(graph, budget)` | `cycles::try_package_cycles(graph, budget)` |
| `PackageCycleDetector::try_detect_imports(groups, budget)` | `cycles::try_package_cycles_from_imports(groups, budget)` |
| `PackageCycleDetector::try_detect_imports_with_files(groups, budget)` | `cycles::try_package_cycles_with_files_from_imports(groups, budget)` |
| `ScanReport::to_human(bool)` | `ScanReport::to_human(ColorMode::Plain \| ColorMode::Ansi)` |
| `HumanFormatter::new(bool)` | `HumanFormatter::new(ColorMode)`; its `use_color` and `verbose` fields are private, set verbosity with `with_verbose` |
| `Workspace::get_package_path` | `Workspace::package_path` |
| `Config::load_or_default` (removed) | `Config::try_load_or_default`, which reports unreadable or invalid files instead of silently using defaults |

The detector structs held no state, so the functions take the same arguments.

## Changed signatures

- `TsConfig::discover` returns `Result<Option<TsConfig>, TsConfigError>`.
  `Ok(None)` still means no `tsconfig.json` was found; a malformed or
  unreadable nearest config, which used to be `None`, is now an error. For the
  scan's best-effort behavior, which skips unusable files and lists them, use
  `TsConfigCache::discover` and `TsConfigCache::rejected`.
- `Config::extensions_for_package` returns `&[String]` and
  `Config::ignore_for_package` returns `Option<&[String]>` instead of cloned
  vectors. Call `.to_vec()` where an owned copy is needed.
- `OutputFormatter::format` and `UnifiedOutputFormatter::format_unified` are
  generic over the writer (`W: Write + ?Sized`) instead of taking
  `&mut dyn Write`. Existing calls compile unchanged, including with a
  `&mut dyn Write`, but the traits can no longer be used as trait objects.
- These builder methods and accessors are no longer `const fn`:
  `Scan::skip_type_imports`, `filter` and `verbose`; `ScanReport::results` and
  `filter`; `ImportExtractor::with_mode` and `with_skip_type_imports`;
  `HumanFormatter::new` and `with_verbose`; `UnifiedResults::has_package_cycles`;
  `DependencyGraph::nodes`, `PackageDependencyGraph::nodes` and
  `EdgeEvidence::file_count`; `BoundedReport::new`. Public `const fn` is a
  permanent promise, and these may need non-const work later.

## Non-exhaustive types

These enums are `#[non_exhaustive]`; add a wildcard arm to matches outside the
crate: `Resource`, `ExtractionMode`, `ImportTarget`, `CycleFilter`,
`WorkspaceFormat` and the new `ColorMode`.

These structs are `#[non_exhaustive]`. Their fields stay public for reading and
updating, but struct literals outside the crate no longer compile; use the
constructors:

| Type | Constructor |
| --- | --- |
| `Import` | `Import::new(source, target, specifier)` |
| `Cycle`, `PackageCycle` | `Cycle::new(path)`, `PackageCycle::new(packages)` |
| `PackageCycleEdge` | `PackageCycleEdge::new` or `with_evidence` |
| `PackageCycleWithFiles` | `PackageCycleWithFiles::new(packages, edges)` or `from_cycle` |
| `PackageResult` | `PackageResult::new(name, path, cycles)` |
| `Results`, `UnifiedResults` | `new()`, then set or push to the fields |
| `Package` | `Package::new(name, path, config)` |
| `Workspace` | `Workspace::new(root, format, packages)`, then set `aliases` if needed |
| `TsConfig` | `TsConfig::new(base_url, paths)` |
| `PathAlias` | `PathAlias::exact(name, replacements)` or `PathAlias::wildcard(prefix, replacements)` |

`Config`, `ScanConfig` and `AnalysisLimits` stay exhaustive so that
`Config { limits, ..Config::default() }` keeps working; fields added later take
their defaults in that form.

## Additions

- `ColorMode` (re-exported at the crate root).
- `PartialEq`/`Eq` for `Import`, `PackageCycleEdge`, `PackageCycleWithFiles`,
  `PackageResult`, `Results`, `UnifiedResults`, `TsConfig` and `PathAlias`;
  `Hash` for `Cycle`, `PackageCycle`, `ImportTarget`, `Resource`,
  `ExtractionMode`, `CycleFilter` and `WorkspaceFormat`; `Clone` for
  `HumanFormatter`. All are structural.
- `Debug` for `ImportExtractor` (settings only) and `BoundedReport`.
- `#[must_use]` on `TsConfigCache::discover_shared` and both graphs'
  `edge_count`.
- Runnable documentation examples for the cycle functions, `Config`,
  `HumanFormatter`, `Workspace` and `TsConfig`.
