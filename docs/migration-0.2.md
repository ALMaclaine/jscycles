# Migration notes for the next 0.2 release

This is unreleased guidance, not a version bump or publication authorization.
The public surface is the documented `jscycles` modules/reexports and CLI/JSON
contract. Patch releases must not conceal source or behavioral incompatibilities.
Owner: alm; Rust-OE API-010, CI-009 and STD-015/016/068.

- `ConfigError::Parse` now carries `serde_saphyr::Error`; update callers that name
  the old serde_yaml payload. Config adds public `limits`; use `Config::default()`
  plus field updates rather than exhaustive literals. `Config::parse` and
  `validate` reject invalid quota/glob/extension settings, including unused overrides.
- Public error enums add quota/validation cases. Update exhaustive matches;
  operational errors remain Results, and the CLI keeps status 0/1/2.
  `DiscoveryError::DuplicatePackage` identifies both canonical directories.
  `TsConfigError::AtPath` wraps the actual selected/inherited input and preserves
  its typed cause; walk `std::error::Error::source` when inspecting leaf errors.
  Workspace JSON/YAML failures also retain their parser causes through
  `WorkspaceError::Json` and `WorkspaceError::Yaml`; the latter boxes a
  `WorkspaceYamlError` path/cause context to keep ordinary Result size small.
- Extracted source/resolved file paths are canonical absolute paths. Rebase a
  path against the package root for display, and keep the native PathBuf for
  identity. The CLI performs its own display rebasing.
- The parser is now WillBooster's tree-sitter runtime and JS/TS grammar forks,
  pinned exactly; the upstream 0.23 grammars rejected valid tagged templates with
  `\u` escapes and `typeof import()` type arguments. The MSRV is 1.93.0 (was
  1.92.0) because the runtime requires it. Syntax errors now name the first
  error's line and column.
- A package no longer analyzes files inside other scanned packages nested in it.
  A workspace root that is itself a named package previously repeated every
  member file cycle under its own name; it now reports only its own files.
  Members removed by `--only`/`--exclude` keep their directories, and
  `scan.exclude` directories are skipped by enclosing packages too.
  `ImportExtractor::with_nested_packages` exposes the skip to library callers;
  `PackageDiscovery::discover_with_boundaries` and
  `discover_explicit_with_boundaries` return the directories to pass it.
- Include/exclude/ignore globs ignore a trailing `/`, and absolute globs match
  lexically normalized paths. On Windows `.\` and a trailing `\` are ignored
  too, and absolute globs match canonical (`\\?\`) package paths regardless
  of drive-letter case. An automatically found tsconfig that fails to load
  now prints a stderr warning; `TsConfigCache::rejected` lists such files.
- Resolution probes declaration files (`.d.ts`, `.d.mts`, `.d.cts`, `index.d.ts`)
  in TypeScript's order, and `.jsx` specifiers prefer `.tsx`.
  `Workspace::split_specifier` returns a specifier's package and subpath; exact
  workspace aliases may contain extra `/` segments. Self-imports by package
  name resolve to files. Duplicate edge evidence and empty subpaths are dropped.
- `ImportExtractor::with_owners` shares every package/boundary root so file
  links are analyzed only by the package owning their target; links within a
  package are analyzed again (a regression in the previous unreleased change).
- A package `extends` (`"extends": "@scope/base"`) resolves through the
  package's `exports` (conditions `node`, `require`, `types`, `default`), its
  `tsconfig` field, or `tsconfig.json`, as TypeScript does. It previously
  ignored `exports` and the `tsconfig` field, so such a config was rejected
  and its aliases dropped with a warning.
- A UTF-8 byte-order mark at the start of `package.json`, a tsconfig, the YAML
  configuration, a source file or `--stdin` is ignored; a BOM in `package.json`
  previously failed the scan.
- An exact workspace alias whose target does not resolve yet (an unbuilt
  `dist` entry, say) is attributed to the member containing its first
  replacement path. This was already the behavior; it is now documented.
- `DiscoveryError::DuplicatePackage` suggests `scan.exclude` for copies such as
  nested worktrees.
- Invalid source syntax, invalid ignore patterns and non-UTF-8 source/package
  paths now fail instead of producing a plausible partial graph. Literal JS
  escapes are decoded; computed dynamic/require expressions are omitted.
  Bare require identifiers also decode their Unicode escapes. TypeScript
  assertions/instantiations around a bare callee or literal argument preserve
  its import edge; values mentioned only in types remain outside the graph.
  `Import::specifier` contains the decoded value. Valid leading-zero braced
  escapes and paired braced surrogate units retain their actual Unicode value;
  unpaired surrogates, NUL module values and legacy numeric escapes fail.
  Missing relative/alias file targets now remain Unresolved rather than
  inventing successfully resolved graph vertices.
- TypeScript config accepts comments/trailing commas. Relative configuration
  paths retain their declaring origin. Child `paths` replaces the parent's option,
  as in [TypeScript's documented inheritance](https://www.typescriptlang.org/tsconfig/extends.html)
  and [its versioned parser implementation](https://github.com/microsoft/TypeScript/blob/v5.9.3/src/compiler/commandLineParser.ts#L3372-L3395).
  Explicit tsconfig selection and no-tsconfig apply to workspace aliases too.
  Exact patterns and trailing-wildcard patterns with one substitution marker are supported; explicitly selected
  unsupported substitution shapes fail instead of being silently misresolved.
  Compiled PathAlias replacements retain their substitution marker rather than
  exposing only its stripped prefix.
  Old fixtures that depended on merging unrelated parent/child aliases now
  correctly stop treating those parent aliases as active.
  Missing `.json` resolution appends to the complete extends stem; native config
  origins remain PathBuf and literal checkout asterisks remain literal.
  File-alias completion also preserves dotted stems. Relative `.js`/`.jsx`
  specifiers can now select their TypeScript source, using the same exact-file
  precedence as aliases. Explicit `.mjs`/`.cjs` substitutions select their matching
  `.mts`/`.cts` family when those sources are included. These repairs can reveal
  previously hidden cycles or remove cycles created by an unrelated shorter stem.
  Alias substitutions now normalize dot segments before file lookup, as relative
  modules already did. This can reveal or remove alias cycles around missing
  directories and directory symlinks; existing targets still use canonical identity.
- Workspace package paths are canonical identities. Duplicate names in distinct
  package directories fail; package analysis across different workspaces must use
  separate invocations. A dangling default-config symlink and malformed CLI
  extension suffix now fail rather than silently selecting defaults/no files.
  Fallible discovery also rejects duplicate names in inner scans without a
  workspace and selects overlapping operands once. Workspace single-star globs
  match one path component; `**` selects recursive paths. Broken workspace
  metadata links, invalid existing glob prefixes and missing TypeScript reference
  roots now fail. Project references may select a directory or a named config file.
  A declared workspace package name also takes precedence over a conflicting
  tsconfig alias, matching path lookup. This can reveal hidden package cycles or
  remove invented ones. Eligible loaded alias tables use direct mapping lookup;
  manually constructed tables preserve their existing first-match semantics.
  Equal ordered replacement lists in eligible exact tables now share normalized
  target lookups during one setup. Target-cache records/paths consume the retained
  payload quota; distinct lists can therefore use more admitted payload. Reloads,
  replacement order, generated targets and nested membership retain their
  existing behavior. Each setup drops its temporary cache before returning.
- CLI scans use shared finite quotas. See [limits](analysis-limits.md) to raise
  a quota deliberately. Prefer the new fallible detector/import APIs; legacy
  infallible detector signatures remain unlimited but use iterative traversal.
  Owned `TsConfigCache::discover/load` copies now consume shared payload/work
  admission; their legacy Option return becomes None on exhaustion and callers
  must inspect the shared budget. `discover_shared` retains Arc sharing.
  Metadata/override sorting also consumes logical work before allocation/sorting.
  Full extraction now admits positive first-index hints from the current walk.
  Their path/record storage consumes the retained payload quota, including hints
  not ultimately referenced. Workspace-only mode skips that storage. File/index
  precedence, ignored/later candidates, host case behavior and file-link origins
  retain filesystem fallback; hints reset before the next extraction. Source
  canonicalization and regular-file reading are unchanged.
  Raw tsconfig documents are now shared once across concurrent children of one
  cache. Unused inherited fields avoid redundant copies; parent type/depth checks
  and declaration-relative origins still apply. Retained raw data consumes the
  payload quota even when a child overrides it. Relative load/discovery keys now
  include the caller's working directory, and relative package operands discover
  configurations above it. Parent-containing operands use their actual selected
  filesystem context rather than an unrelated sibling/caller configuration.
  These fixes can reveal previously hidden alias cycles. Successful cached
  snapshots remain reusable after source removal; create a new cache to reload.
  Calling `TsConfigCache::with_budget` on a used cache now starts empty caches;
  values charged to an earlier budget cannot bypass the replacement budget.
  Ordinary clones retain their shared snapshots and original budget.
- Directory exclusions and globs now apply to paths relative to the walk root.
  The built-in vendor/build names, `scan.exclude` and package `ignore` patterns
  no longer match ancestors of the scanned checkout. Previously a checkout under
  a `build/`, `dist/`, `node_modules/`, `.git/` or `.next/` ancestor (common CI
  workspace layouts) was skipped entirely and reported clean; it is now scanned,
  which can reveal cycles. Package-relative `ignore` patterns such as `src/x.ts`
  now match; `**/`-prefixed patterns behave as before. A leading `./` is
  ignored and absolute patterns still match absolute paths. Relative
  `scan.exclude` patterns are relative to the walked root (the cwd for an
  automatic scan, the operand for an explicit one). The walk root itself is
  never pruned, so an explicitly selected operand is always scanned and an
  empty pattern cannot remove the whole scan.
- An existing resolved import target whose canonical path cannot be established
  now fails with status 2 instead of keeping a non-canonical spelling that could
  silently drop a cycle edge.
- TypeScript `extends` accepts the TypeScript 5.0 array form. Entries apply in
  order, later ones overriding earlier ones; each counts toward the depth-ten
  limit along its chain and consumes shared work quota. Previously an array
  failed to parse (strict `--tsconfig` error; automatic discovery dropped aliases).
- Public error enums are `#[non_exhaustive]`; add a wildcard arm to matches.
- Import specifiers that name a directory (relative `./dir/`, `.`, `..`, and
  aliases whose specifier or configured target ends in `/`; as in TypeScript,
  a substituted alias `.` still tries `<dir>.ts` first) resolve only to the directory's index file, as Node and
  TypeScript do. A sibling `dir.ts` previously matched and could invent a cycle.
- Workspace globs and negations, and `scan.include` roots, ignore a leading
  `./`; such patterns previously matched nothing and reported clean.
- A tsconfig project with `references` is now provisional during upward
  workspace discovery: the first pnpm/npm workspace above it wins only if one
  of its declared member patterns selects a directory containing the project
  (a member without `name` counts). A member package's own tsconfig references
  previously captured the workspace root, rejecting valid multi-operand scans.
  Nothing above that first workspace is read, and its unreadable metadata keeps
  the references project, as before.
  `Workspace::discover_root` (new) identifies a root without expanding members;
  the CLI uses it so many operands no longer repeat full workspace discovery.
- An explicitly selected `package.json` without `name` fails with
  `DiscoveryError::MissingPackageName` instead of reporting "No packages found."
  with status 0. A bare `package.json` operand (no directory) now selects the
  current directory. Explicit YAML `null` for a package `extensions`/`ignore`
  override is rejected rather than treated as omitted.
- Source/configuration errors previously ignored now exit 2; do not interpret
  any failed report as clean. `--stdin` conflicts with positional operands.
  `--outer` is a source-graph filter; unused manifest declarations are not edges.

The YAML parser now requires serde-saphyr 2.1.1 after 2.1.0 was yanked. Blank and
comment-only configuration streams select default settings; explicit nulls and
invalid mapping/sequence shapes still fail. Source-file alias memoization is
bounded and cleared between extractions, including cached missing targets.
Alias-table matching work is charged on actual uncached alias lookups. Relative,
workspace and cached imports no longer reserve an unused table on every reference;
scans may now fit a finite work limit that previously rejected them unnecessarily.

No semver-checker result is claimed. It would need an explicit published/git
baseline, selected targets/features, tool pilot and behavioral review. These
known intentional changes already require the 0.2 classification; a tool cannot
approve changed path/config/graph meanings. Release preparation must compare the
chosen baseline and these notes before tagging.

Rollback: reinstall the previous binary/version from its verified release assets
(or build its source with its lockfile). There is no project database to migrate.
Discard reports from failed/unsupported scans; preserve the chosen YAML and CLI
arguments to reproduce differences. Do not rewrite a released tag or substitute
new bytes under an old version to perform a rollback.
