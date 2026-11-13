# Product and interface contract

Owner/change authority: repository maintainer alm. Rust-OE revision:
`19e4133c0461129b0f5fdb286abd7b87511242e2`, especially STD-015–018,
STD-047/048/056–059/061–063/071 and API/INPUT/CLI. These requirements describe
implemented local CLI/library behavior, not a claim of complete JavaScript runtime
or TypeScript compiler conformance. Review this contract when those boundaries change.

| Requirement | Selected behavior | Acceptance evidence |
| --- | --- | --- |
| R01 Selection | Discover named package.json packages. Automatic scans apply include roots; explicit directories recursively discover packages and manifest operands select one package. A directory without a named manifest contributes no package; an explicitly selected manifest without a name is an error. Workspace and include globs ignore a leading `./`. A TypeScript references project is the workspace unless the first pnpm/npm workspace above it declares a member pattern selecting a directory that contains the project; higher or unrelated workspaces are not read, and unreadable metadata there (other than an exhausted quota) leaves the references project in effect. Include overrides and CLI name selectors are globs. Each source file belongs to its nearest package: a package skips nested directories of other discovered packages (including those removed by name filters) and directories pruned by `scan.exclude`. Package analysis accepts one workspace per invocation; different workspace roots and duplicate package names are errors. | `tests/cli.rs`, discovery integration tests |
| R02 Source graph | Extract static imports (including type-only imports unless `skip_type_imports`/`--skip-type-imports` selects runtime imports only: then `import type`/`export type` statements, type-position `import()` and declaration-file imports are left out), re-exports, quoted-literal dynamic imports and require calls from JS/TS/JSX/TSX. Resolve relative paths and supported tsconfig aliases; classify known workspace names and exact workspace aliases before file aliases. A declared workspace package name wins over a conflicting alias; name and path lookup agree, including subpaths. Exact workspace aliases match the whole specifier or its longest `/`-bounded prefix, so alias names may have extra segments. A file importing the package that contains it (judged by the file's own location, nearest member first) resolves through file aliases rather than recording a package edge. An exact alias whose target lies in a named package nested inside a member, nearer than any member, is a file alias, not an edge to the member. File candidates follow TypeScript's order: `.ts`, `.tsx`, `.d.ts`, then JavaScript (`.jsx` prefers `.tsx`; `.mjs`/`.cjs` use their module families), and directory indexes likewise. Any import that resolves to a file owned by another scanned package or workspace member, through a relative path or a wildcard or exact file alias, is an edge to that package (no subpath), with or without a workspace; the nearest package root or excluded boundary containing the file owns it, so a file in a nested package belongs to it and a file in an excluded directory creates no edge. Bare names become package edges only for workspace members. `--outer` resolves the same way and keeps only package edges. Ignore computed expressions. This is static analysis, not code execution or type checking. | import fixtures and `tests/adherence.rs` |
| R03 Cycle semantics | One deterministic representative per cyclic file SCC, preferring a multi-file witness in a multi-file SCC and reporting singleton self-loops. Enumerate elementary cycles among distinct packages (scanned packages and the workspace members they depend on); no workspace is required. Package edges come from source, not dependency declarations. Outer mode skips file graph search. | exhaustive four-vertex oracle and deep-graph tests; CLI filter tests |
| R04 Failure/completion | Exit 0 for no cycles in the selected graph, 1 for cycles, 2 for input/operational/quota/output errors. Parser ERROR/missing tokens reject that scan with the first error's line and column; recovered syntax is insufficient evidence. The pinned grammars must accept valid ecosystem syntax for this to hold; grammar gaps are defects (see build-trust.md). All workers must succeed before a report is staged. | process tests for every quota, config/stdin/syntax/write failures |
| R05 Machine interface | CLI `--json` emits the existing unified object with booleans `has_cycles`, `has_file_cycles`, `has_package_cycles`, arrays `file_cycles` and `package_cycles`. File entries contain name/path/cycle arrays. Package entries contain packages/edges; edges contain from_package/to_package/files, with `path` in each file and `subpath` only when the import named a package subpath (an absent subpath omits the key, never `null`). Edge file evidence is populated only in verbose mode. No source text is emitted. | JSON/snapshot tests and output unit tests |
| R06 Human interface | English UTF-8 diagnostics and reports. Colour is selected from stdout TTY status; JSON is uncoloured. Untrusted controls, all twelve Unicode bidi controls and Unicode line/paragraph separators are escaped in human report labels and argument-error values. Generated diagnostic layout is retained. Valid native arguments keep their actual path identities. Quiet suppresses clean reports, including JSON; errors still fail. Empty stdin is an error. Help/version write and flush failures exit 2. `--stdin` and positional paths conflict rather than silently discarding inputs. | process and formatter tests |
| R07 Resources | One finite shared budget across readers/workers/graphs/enumeration. See [limits](analysis-limits.md) for exact quotas and units. Output is staged before stdout; write/flush/broken-pipe errors can still leave invalid partial bytes and exit 2. | `tests/budgets.rs` and input unit tests |
| R08 Filesystem | Read regular files without following directory symlinks encountered beneath walk roots; an explicitly selected root may be canonicalized through a symlink. File symlinks can resolve to regular files. Canonical source/resolved identity follows host filesystem semantics; no containment or race-proof snapshot is promised. Source/package paths must be UTF-8 for the current output schema. Actual vendor/build directory components beneath a walk root are excluded; similarly named checkout directories and excluded-name ancestors of the walk root are allowed. Relative exclude and ignore globs match paths relative to the walked root (the cwd for automatic scans, an explicit operand, or the package) and ignore a leading `./` and trailing `/` (also `.\` and `\` on Windows); absolute include/exclude/ignore globs match the lexically normalized absolute path, on Windows without a verbatim `\\?\` prefix and with an uppercase drive letter. A file link belongs to the nearest package or excluded boundary containing its target; links to targets outside every package stay with the linking package. A walk root itself is never pruned. | explicit inputs, alias identity, vendor-substring and filesystem tests |
| R09 Configuration | One configuration snapshot per CLI run; no reload/environment interpolation or secret fields. CLI extension selection overrides package overrides, which override defaults. YAML is closed, rejects duplicates and dangling default-config links, and validates every override even when unselected. CLI/YAML extensions are nonempty alphanumeric suffixes without dots; an empty list explicitly selects no source extensions. Save the YAML, arguments, source revision and tool versions to reproduce a scan; config includes no secrets. | Config::parse/validate tests and precedence process tests |
| R10 Ecosystem formats | A leading UTF-8 byte-order mark is ignored in every text input. package.json is strict JSON with an open set of unrelated keys; typed known fields reject duplicates. Tsconfig accepts comments/trailing commas, `extends` as one path or an ordered array (later entries win) with depth ten per chain, declaration-relative baseUrl/paths origins, child paths replacing inherited paths, exact/longest-prefix alias priority, and replacement fallbacks/index files. Exact-to-exact and single trailing-wildcard-to-single-wildcard substitutions are supported; selected unsupported patterns/empty replacement arrays fail. Unknown ecosystem fields remain open. Workspace aliases share the same parser/origin logic; an exact alias whose target does not resolve (for example an unbuilt output) is attributed to the member containing its first replacement path. Package `extends` follows TypeScript's config lookup: `exports` (conditions `node`, `require`, `types`, `default`) decide alone when present, otherwise a subpath names a file or directory and a bare package uses its `tsconfig` field or `tsconfig.json`; explicit tsconfig supplies their selected mappings and no-tsconfig disables automatic alias mapping. Automatic optional aliases tolerate ordinary bad metadata, with a stderr warning naming each ignored tsconfig; explicit tsconfig and exhausted budgets fail. | TypeScript origin/JSONC/fallback tests and fixtures |
| R11 Identity and Unicode | UTF-8 strings compare exactly; no Unicode normalization, case folding, locale-dependent sorting or identifier rewriting is applied. Filesystem canonicalization follows the host's case/symlink rules. Decode JS literal StringValue before resolving; reject NUL and unpaired-surrogate module values. JSON serializers escape controls. Human text and JSON strings are not canonical JSON or signatures. | generated JSON/JS string-value oracle and Unicode escape regressions |
| R12 Cache lifecycle | CLI caches live for one invocation and retain immutable metadata snapshots with Arc. An extractor clears filesystem resolution hits/misses before each extraction; its budget remains cumulative. A TsConfigCache intentionally reuses its snapshot; construct a new cache after metadata changes. Concurrent checkout mutation is unsupported. | repeated-extraction regression and shared-snapshot tests |
| R13 Privacy | Inputs may contain proprietary source and private paths. Production runs read locally, run no project scripts, contact no remote service, and save no report/cache to disk. Reports/stderr can disclose names/paths and configuration error snippets. Users own redirected files and their permissions/retention; do not supply credentials in config. CI retains only approved synthetic/source/build evidence. | architecture review; no runtime network/subprocess/storage adapter |

Fallible discovery selects the same name/canonical-directory package once across
overlapping operands and rejects a selected name at distinct directories,
including inner-only scans without workspace metadata. Workspace include/exclude
globs use path-component separators: `*` selects one component and `**` can
recurse. A missing glob root may match nothing; an existing non-directory prefix
or broken membership metadata entry is an error. Project references accept an
existing directory or named regular config file, then select its named package
when present. Missing reference roots fail. These are membership rules, not a
promise to build or type-check referenced projects.

Overlapping YAML package patterns select the first matching key in lexical
order. Native config-origin paths retain their bytes during alias substitution;
the current JSON contract still requires UTF-8 package/source report paths.
Inherited-config failures identify the actual failing file and preserve its
typed error cause.
Tsconfig discovery and cache-load keys anchor relative paths in the caller's
current directory, including an upward search above that directory. Selected
filesystem paths containing `..` follow the actual directory before walking
ancestors, so a caller's sibling configuration cannot override the selected
package's ancestor. Directory symlinks in those operands follow host filesystem
semantics; a failed physical lookup remains best-effort metadata failure.
Relative module paths and substituted tsconfig paths collapse `.`/`..` lexically
before file lookup, matching the selected Node/TypeScript module-resolution
policy. Existing targets are then canonicalized for graph identity. This order
also applies across nonexistent components or directory symlinks; it does not
ask the filesystem to interpret the uncollapsed parent traversal. Native config
origin bytes are preserved. This is path syntax, not Unicode normalization or
a filesystem containment boundary.
Owned metadata-cache getters admit retained copies under the shared payload/work
budget; shared getters retain Arc snapshots. Missing alias/relative file targets
remain unresolved graph inputs. A declared alias candidate can still identify
workspace membership when its final generated file is absent. Module values
decode braced code points with leading zeros and paired surrogate units before
UTF-8 validation; legacy numeric escapes are explicitly unsupported and fail.

Within one TsConfigCache, canonical raw documents are shared across children and
concurrent readers. Compiled snapshots retain their absolute selected-path key.
Raw values carry no merged child state: each extends occurrence applies its own
origin, depth check and independent baseUrl/paths inheritance. Overridden parents
still parse their known field types. Successful cache hits retain their snapshot
even if the original selected file or discovery link is later removed; this is
reuse, not a filesystem reload guarantee. Construct a new cache after changes.
Assigning a budget with `TsConfigCache::with_budget` starts empty caches so the
new budget pays for its own reads/retention. Ordinary clones share both snapshots
and their original budget.

Workspace alias setup may share target resolution for eligible exact aliases
whose complete ordered replacement lists agree. Replacement order, file/index
fallback and deepest package membership remain unchanged. Public tables that
require general first-match resolution cannot share by those lists alone, because
an earlier wildcard may depend on each alias name. Each setup starts a new target
cache and observes the current stable checkout. A quota failure preserves the
previous published alias map.

File resolution can reuse positive `index.ts` entries from the current extraction's
walk. Only regular first-index files beneath a canonical root are eligible;
encountered directory links are not followed. Exact/extension file candidates
still precede directory indices. Missing or ignored entries, later index types,
case variants and file links retain filesystem fallback. This adds no inferred
absence, case folding or persistent filesystem cache. Workspace-only mode skips
the hints. Every source still uses canonical identity and the bounded regular-file
reader, including physical origins for source links.

Unexpected Rust panic uses default runtime reporting; the parent process/OS owns
fatal signal and cancellation handling. No custom crash reporter, panic recovery,
abort cleanup or crash-redaction guarantee is selected. Consumers accept completed
reports only with the selected clean/cycle statuses 0/1; status 2 and other
unexpected termination invalidate a report, even if bytes were already written.

The supported extension dispatch is `.ts`, `.tsx`, `.js`, `.jsx`, `.mjs`, and
`.cjs`; other configured suffixes use the TypeScript grammar. There is no Flow,
Vue/Svelte embedded-script extraction, package exports/conditions evaluation,
JavaScript execution, dependency installation, or arbitrary computed-import
resolution. A clean result applies to this selected static graph. Missing
external packages and unresolved targets do not establish compiler correctness.
These limits must stay visible when interpreting reports.

Literal dynamic imports retain their first argument when options are present.
Bare require calls support optional calls, TypeScript type arguments, additional
arguments and transparent parentheses around the callee/literal. TypeScript
`import Name = require('module')` contributes an edge too. Member calls such as
`object.require`/`require.resolve`, spread arguments and computed specifiers stay
outside this literal-call selection. Native source lengths must fit u32; raising
file/aggregate/work quotas cannot authorize a truncated native parse.

Unicode escapes in a bare `require` identifier preserve its lexical name. The
callee and literal argument can also be wrapped in TypeScript `as`, `satisfies`,
non-null assertions, angle assertions (outside TSX), or type instantiation.
These wrappers preserve the expression value; a string mentioned only in a type,
a member callee or a computed argument still contributes no literal call edge.

An empty, whitespace-only or comment-only YAML stream selects default settings.
Explicit nulls, sequences and null-valued mapping sections remain errors. This
distinction is checked at the actual CLI after the YAML dependency update.

Relative imports and file aliases share file-candidate selection: an existing
exact file wins, then `.ts`, `.tsx`, `.js`, `.jsx` candidates are checked in that
order. A recognized source suffix is substituted; other dotted suffixes remain
part of the stem (`model.v2` selects `model.v2.ts`, not `model.ts`). Explicit
`.mjs`/`.mts` targets stay within the `.mts`/`.mjs` family, and `.cjs`/`.cts`
targets within `.cts`/`.cjs`. Source discovery still uses the configured extension
list; analyzing these extra families requires opting into those extensions.
This finite policy preserves exact-file precedence and does not emulate every
TypeScript `moduleResolution` mode, package export condition or declaration lookup.
`tests/resolution.rs` checks hidden/invented cycles through the real CLI.

Syntax traversal classifies numeric node IDs using immutable tables derived from
each linked JavaScript/TypeScript/TSX grammar's own metadata, including aliases.
These tables contain only classification enums and persist for the process;
they retain no source, tree, parser or filesystem result. Source admission,
whole-tree syntax-error rejection and every node's source-range check still run.
Every consumed module literal and bare-call identifier additionally uses the
same checked text operation; invalid byte/UTF-8 boundaries return a parse error.

The runtime uses Rayon for bounded host-pool parallelism, not Tokio or detached
application tasks. `RAYON_NUM_THREADS` is an explicit operator/environment choice;
there is no server concurrency or wall-clock SLA. File-descriptor errors fail the
scan. Graph/work admission quotas are not hard OS RSS/time limits. Runtime code
has no wall-clock, random identity, authentication, database, container or service
management boundary. Test temp names and benchmark monotonic time are development
facilities; they are not public stable identifiers or timing guarantees.

The machine-output shape is an existing compatibility contract. Additive fields
must be documented; removals/type/meaning changes require migration notes and an
appropriate release. JSON numbers currently represent no user-provided numeric
payload; counts stay in human summaries, and no normalization/canonicalization
promise is made. Older binaries can be reinstalled without a data migration
because the CLI owns no persistent project data. Discard failed/partial reports
and rerun; there is no resumable report format.

This contract deliberately does not declare a customer latency threshold. Existing
synthetic timings validate a known result and measure that workload only. A new
speed/SLA claim requires a representative user workload, baseline, hardware/load
shape, selected percentile/resource objective, and review by alm.

Public graph/result/configuration records are data containers, not certifications
that caller-created values satisfy every detector/input invariant. Use
Config::parse/load or validate a programmatically constructed Config. The legacy
workspace-map selector and detector methods are explicitly unmetered; fallible
CLI ingress owns resource failures. Output writers can fail after partial writes;
callers needing pre-publication quota checks should use BoundedReport.
