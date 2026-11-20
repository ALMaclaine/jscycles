# Rust-OE adoption

Owner: alm, repository maintainer. Adopted 2026-11-04 (UTC) from Rust-OE
`19e4133c0461129b0f5fdb286abd7b87511242e2`.

The [managed guidance index](../.rust-oe/INDEX.md) records the immutable bundle
pins. This file owns project decisions and exceptions; managed guidance is
updated with `rust-oe plan --diff`, `rust-oe sync`, and `rust-oe check`.

## Scope

This record retains the original adoption history. The [full audit](maintainers/rust-oe-full-audit.md)
owns the applicability assessment. The [engineering tool results](maintainers/quality-engineering.md)
supersede earlier deferrals of strict Clippy, CRAP, mutation testing, Miri, and
profiling. Installation and passing checks do not establish full adherence.

jscycles is a synchronous local CLI and reusable Rust library. Applicable bundles
are standards, agents, API, CLI/input boundaries, dependencies, verification,
observability/performance, and development tools. Tokio actors, PostgreSQL,
columnar storage, and service telemetry have no current implementation boundary.
Unsafe code remains forbidden in application and build tooling; native parser
dependencies remain part of the build trust boundary.

## Implementation decisions

This adoption applies CI-003 through CI-009, TOOLS-001 and TOOLS-007, INPUT-003
through INPUT-009, CLI-003 through CLI-008, API-008 and API-010, TEST-003, and PERF guidance
at the source revision above. See [checks](../tooling/checks.md) for the selected
commands, support matrix, limits, exception handling, and verification results.


## Adopted behavior and compatibility

- C01–C04: format every member, reject stale locks, execute a real public example,
  and retain explicit unsafe-operation lints. All baseline commands select the
  workspace, all target kinds, and all features. Clippy all/pedantic/nursery/cargo
  are now enforced; mutually incompatible restriction lints are not enabled as a group.
- INPUT/CLI: reject malformed/unknown/duplicate user configuration, bad selectors,
  missing explicit configs, traversal failures, invalid/failed stdin reads, and
  oversized files. A fallible optional-config API preserves real errors. CLI
  extensions override YAML. Relative import sources and targets use consistent
  path spelling so a cycle cannot disappear just because the scan uses `.`.
- Dependencies: update crossbeam-epoch 0.9.18 to 0.9.20 for
  [RUSTSEC-2026-0204](https://rustsec.org/advisories/RUSTSEC-2026-0204).
  Replace serde_yaml/unsafe-libyaml with serde-saphyr 2.1.0 for
  [RUSTSEC-2024-0449](https://rustsec.org/advisories/RUSTSEC-2024-0449).
  The advisory check covers build/development dependencies too. Remove the unused
  SurrealDB BUSL allowance and inactive ring clarification; retain the existing
  applicable license/source policy with no new advisory exceptions.
- Delivery: test each advertised release target, test the exact MSRV, and schedule
  advisory current-stable/fresh-resolution checks. Release builds require CI;
  tags/manual invocation replace automatic publication on every main-branch push.
  Attach source/version metadata and artifact checksums. An explicit source-package
  include list prevents shipping the repository's 38 MB prebuilt Linux executable.
- PERF/TEST: remove unsupported README speed ratios, mark editorial timing claims
  unverified, add a reproducible synthetic measurement command, and provide optional
  pinned LCOV reporting. Neither timings nor coverage percentages are gates.

The public `ConfigError::Parse` payload changes from `serde_yaml::Error` to
`serde_saphyr::Error`. This is a source compatibility change for callers naming
or constructing that payload. Classify the next library publication as a 0.2
release; this adoption does not publish or bump a release. Existing legacy
best-effort loader signatures remain. Configuration typos/duplicates, excessive
input, and errors previously skipped now fail intentionally. Valid configuration
and the CLI's 0/1/2 status and JSON structures remain compatible.

## Exclusions and revalidation

No Tokio runtime/actors, database migrations, columnar typestate, service telemetry,
or OpenTelemetry endpoint is introduced. No new audit trust, Vet exemption, API
baseline, or numeric performance budget is claimed. The engineering tool pass
adds opt-in local Hotpath timing, pinned Miri and mutation campaigns, and a
coverage-backed CRAP gate. The subsequent full audit selects
owned configuration/native syntax fuzz harnesses and coverage triage. These require project-specific evidence and an
owner when the corresponding boundary actually appears. [Analysis limits](analysis-limits.md) records finite scan-wide quotas and iterative
graph traversal. Exact RSS/wall-time isolation and legacy ordinary alias-error
fallbacks remain explicit local-tool limits in [the check contract](../tooling/checks.md). Rust-OE's bundled tool assets are
references; the project commands listed there are the installed integration.

alm revalidates the selected rules and exceptions on compiler/MSRV, dependency,
platform, input/exposure, CLI schema, public API, or control changes. Project policy
and exceptions remain here, outside managed bundles.

## Local verification

On native macOS aarch64, Rust 1.98.1 passed `cargo xtask ci`: 369 target tests and
one executable library doctest, plus formatting, locked builds, Clippy, dependency
policy, and documentation. Rust 1.92.0 passed the same target tests and doctest.
All 12 pass/defect/repair control cases passed; [the report](maintainers/evidence/rust-oe-validation.json)
records commands, input hashes, exits, and limits. No unsafe probe was executed.

The pinned coverage wrapper produced LCOV, and its earlier failing CLI tests
propagated failure before repair. The synthetic 1000-file, one-cycle release
workload completed one warm-up and seven checked samples; its median was about
14 ms on this host. These are local synthetic observations, with binary/corpus
hashes in [the retained experiment](maintainers/evidence/rust-oe-benchmark.json); they support no madge speedup.
The updated source crate passed `cargo package --locked --allow-dirty` including
verification; its size fell from 37.8 MiB to about 508 KiB. GitHub workflows passed
local actionlint 1.7.12 checks. Guidance passed an
offline `rust-oe check`.

Hosted jobs, branch protection, and the Linux-only `/dev/full` test were not
executed by this local macOS run. LCOV and benchmark artifacts stay under ignored
`target/tooling`; there is no external code/telemetry upload. The manual advisory
coverage job retains LCOV for seven days for alm's review. Review uncovered
configuration/discovery paths before expanding assurance; line coverage includes
test source and is not a product correctness score.


## Library API style pass

The 0.5 library API changes, listed in the [upgrade notes](migration-0.5.md),
apply the Google Rust style guide's API design rules for library code
(<https://google.github.io/styleguide/rustguide.html>). They apply API-004,
API-006, API-007, API-008 and API-010 at the source revision above.

- API-004 (borrowed forms): package override lookups return borrowed slices
  instead of cloned vectors.
- API-006 (`?Sized`): the formatter traits take `W: Write + ?Sized`. The
  caller benefit is that existing `&mut dyn Write` arguments keep compiling.
  The implementations forward through the sized `&mut W`, so the
  non-generic bodies are not monomorphized per writer type.
- API-007 (trait promises): the added `PartialEq`/`Eq`/`Hash` implementations
  are derived and structural, so equal values hash alike. `Debug` for
  `ImportExtractor` shows its settings and omits caches
  (`finish_non_exhaustive`). The crate denies
  `missing_debug_implementations` so later public types keep `Debug`.
- API-008 (consumer tests): tests construct results and metadata through the
  new constructors. A malformed nearest tsconfig is checked as an error from
  `TsConfig::discover`, and as a recorded rejection from `TsConfigCache`.
- API-010 (compatibility):
  - `#[non_exhaustive]` is applied where expansion is concrete:
    - result and metadata structs, which gain fields as reports grow;
    - classification enums, which gain variants as resolution and workspace
      formats grow.
  - `Config`, `ScanConfig` and `AnalysisLimits` stay exhaustive, because
    record-update construction is their intended use.
  - `Config::load_or_default` is removed, and `TsConfig::discover` now
    reports errors. Both used to report a bad input as a clean default,
    contrary to the boundary policy in [checks](../tooling/checks.md).

### Code organization

The Google Rust style guide's untagged code organization rules are applied.
Its two Chromium-scoped rules (small crates, flat layout) are not:

- **One crate:** the library is a single crate of leaf modules, each named
  after its primary item, with the command in `src/main.rs` behind the `cli`
  feature. The crate root, a non-leaf module, holds only module declarations
  and re-exports; each error type is defined in the module that returns it.
- **Small crates were tried and reverted:** before 0.5 was released the
  library was split into seventeen crates under `crates/`, one per module.
  Measured on this repository, only release rebuilds got faster (the release
  profile's `codegen-units = 1` had recompiled the whole library), clean
  builds were no faster, and every crate added a manifest, a published
  version, a publish step against crates.io's new-crate rate limit, and
  per-crate handling in the release, fuzz, Miri and mutation tooling. The
  guide's rationale for the rule is build-graph scale in Chromium, which a
  15,000-line project does not have. The split was undone before release, so
  no `jscycles-*` crates were published.
- **Imports:**
  - No glob imports outside `use super::*` in tests; rayon's prelude became
    trait imports scoped to the two functions that use them.
  - Every `use` imports from one module.
  - Free functions are called through their module (`budget::units`), after
    `use crate::budget`.
  - Types are imported and used bare. Ambiguous names (`io::Error`,
    `fmt::Result`) and collision-prone modules (`unix_fs`) keep their module.
    `clippy::absolute_paths = "deny"` enforces this.
- **Private modules stay private:** `analysis`, `input`, `module_path`,
  `module_syntax`, `syntax` and `terminal` are crate-private. Their former
  `bool` parameters became enums (`analysis::Evidence`,
  `module_syntax::TypeOnlyImports`). The fuzz targets compile `syntax` and
  `module_syntax` through `#[path]` rather than widening the public API.
- **Tests:** the fixture-based `tsconfig` and `workspace` tests are root
  integration tests beside the fixtures.
- **Mutation review:** the reviewed mutation exclusions and timeout names were
  remapped to the current lines. Their diff digests and selection digest
  change with any source change; the next `cargo xtask mutants` campaign
  re-reviews them.

### Full-text review of the guide

A second pass checked the API against the guide's full text rather than a
summary:

- **Naming:**
  - Fallible constructors are `try_new`.
  - Further constructors are `new_with_..`/`new_from_..`; that includes
    `TsConfigCache::new_with_budget`, since the old `with_budget` discarded
    `self`.
  - By-value builder setters use plain names (`ImportExtractor::workspace`,
    `HumanFormatter::verbose`), because the guide reserves `with_..` for
    closure accessors.
  - Type lifetimes are descriptive (`'shared`, `'imports`, `'specifier`).
- **Tuples:** public tuples became named structs: `EvidenceFile`,
  `IgnoredTsConfig` and `PackageSpecifier`. The `(name, imports)` iterator
  items stay pairs, because key/value pairs are how Rust iterates maps
  (API-004).
- **Iterator parameters:** functions that iterate once take `IntoIterator`.
  - Import iterators are bounded by `Borrow<Import>`, so owned and borrowed
    imports both work; the bound uses only `Borrow`'s reference blanket
    impl, with no comparison or hash law (API-007).
  - `PackageDiscovery`'s `only`/`exclude` stay slices, because they are
    iterated twice: quota admission, then compilation.
- **`Arc`:** `ImportExtractor` borrows the scan-wide workspace, tsconfig and
  ownership tables, since rayon's parallel iterators are scoped, and its cache
  is a plain `Mutex`. The `TsConfigCache` snapshots stay `Arc`, because sharing
  them between clones is the cache's documented purpose.
- **Poison:** poisoned locks are never read. A `lock` helper in each module
  that locks propagates the other thread's panic, under a narrow
  `#[expect(clippy::expect_used)]`. Poison implies a bug that rayon re-raises
  anyway.
- **Traits:**
  - Structural `PartialEq`/`Eq` (and `Hash` for value types) on types whose
    fields all support them, so results and configuration can be compared
    and used as keys (API-007).
  - `FromStr` for `Config` and `FromIterator` for `DependencyGraph`.
  - Derived `Default` for `Results`.
- **`for` loops:** combinator chains in `for` headers were named first.

Lint policy exception: Clippy's `missing_const_for_fn` (nursery) conflicts
with treating public `const fn` as a permanent promise. Each public builder,
report accessor or graph accessor that may need non-const work carries a narrow
`#[expect]` with that reason. Plain-data constructors and accessors stay `const`.

## Subsequent adherence audit

The [adherence record](maintainers/rust-oe-adherence.md) closes concrete selection, output-mode,
alias-path, and explicit-input gaps with regression tests. It also adds the
network-independent guidance-integrity gate and combined required status. Its
validation report supersedes the original control report for current inputs;
the original report and synthetic benchmark remain evidence for their own inputs.

The library extractor now returns canonical absolute source/resolved paths;
this behavioral compatibility change ships with the YAML error payload change in
0.2. CLI file-cycle paths retain their package-relative presentation.
The subsequent [analysis-budget implementation](analysis-limits.md) adds shared
aggregate quotas, iterative graph traversal, bounded enumeration, and staged
reports. Its public Config field/error variants also require the planned 0.2
compatibility release. Hosting protection and hosted job execution still require
verification.
