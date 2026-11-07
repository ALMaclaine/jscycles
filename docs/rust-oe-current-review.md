# Combined-tree Rust-OE review

Owner: alm. Local agent review, 2026-11-06 UTC. Source guidance remains pinned to
`19e4133c0461129b0f5fdb286abd7b87511242e2`. This review reconciles the earlier
[applicability audit](rust-oe-full-audit.md) and
[claim ledger](rust-oe-closure-ledger.json) with the subsequent engineering work.
It does not establish organizational certification or complete adherence.

The subsequent [Linux verification](rust-oe-linux-verification.md) closes L01
with a fresh complete 790-case run and both strict reviews under the unchanged
policy. It also passes Linux Clippy and the native Linux-only controls. The
failed Mac attempts below retain their original source/outcome scopes; current
operational gaps remain G01/G02/G03 and conditional C01.

## Scope and findings

The pinned source ledger, inventory and project ledger have the same 521 unique
claim IDs, document paths, sections and claim hashes across 22 source documents.
That identity check establishes which guidance was reviewed; it does not establish
that a row is a mandatory requirement or that every implementation is correct.
Inactive architectures, contextual claims and operational obligations retain
their distinct dispositions.

The review found stale project decisions: the ledger still deferred CRAP and
mutation testing, omitted the optional profiling feature and comparative
measurements, referenced the deleted GitLab workflow and named the removed
general parser adapter. The reconciliation refreshes those decisions and links
them to actual current commands and evidence. Original claim identities remain
unchanged; the validation report records the previous ledger digest and exact
changed records. Historical validation artifacts retain their own input scopes.

The user explicitly selected strict Clippy. The project consequently enables
all/pedantic/nursery/cargo with warnings denied, as a documented local exception
to CI-010's instruction to select nursery lints individually. The pinned compiler
and narrow justified suppressions bound the upgrade work. The restriction group
remains selective, and the
compiler-visibility conflict and local justified exceptions remain documented.
This is an owned project policy, not an upstream guarantee that every lint group
can be enabled without exceptions.

No product runtime defect or new optimization was identified in this bounded
review. A verification defect was found: an inherited `RUSTUP_TOOLCHAIN=1.96.1`
silently overrode the repository pin in a direct Python mutation invocation.
The incomplete 363-case run was interrupted and retained with its unresolved
timeouts; it is not passing current campaign evidence. Explicit Rust CI/MSRV/Miri
commands used their intended compilers and were unaffected.

The runner now selects the repository compiler before fresh selection/execution,
verifies actual Cargo/rustc identities and stores them in schema-3 manifests.
Three regressions check override isolation/restoration, rejection of an incorrect
actual compiler, and missing/mislabeled manifest identities. Historical schema-2
reports retain their original scopes without acquiring invented compiler metadata.
Product Rust, Rust tests, manifests, locks and mutation exclusion/timeout policy
remain unchanged. The complete 790-case campaign used verified 1.98.1 execution;
its strict review failed. Changed-module campaigns and the interrupted
older-compiler run do not establish a current passing full selection.
The full mutation selection uses default-feature Cargo tests; all-feature CI
and the instrumentation experiment separately cover the optional profiling build.

## Reviewed boundaries

| Area / guidance | Current implementation and meaningful evidence | Limit or outstanding obligation |
| --- | --- | --- |
| API / API, STD | Owned results, borrowed inputs, explicit cache lifetimes and typed causes; public doctest, consumer regressions, migration notes | DTO construction does not certify a graph/cycle; release baseline and authorization remain C01 |
| Inputs/configuration / INPUT, FUZZ | Bounded regular-file readers, closed project schema/open ecosystem schema, origin/precedence validation, sticky shared budgets; boundary/process tests and configuration/text fuzzing | Automatic ordinary metadata fallback and legacy best-effort APIs are named compatibility exceptions |
| Native syntax / FFI, SAFE | Published typed grammar handles, checked u32 input range, recovery rejection, checked UTF-8 slices and bounded immutable classifications; native mixed-grammar regressions and Rust/C ASan | Dependency native soundness and hostile-input process isolation are not proved; Miri does not execute native C |
| Graphs/resources / INPUT, TEST | Iterative traversal, work/payload/cycle admissions, independent exhaustive small-graph and deep-stack tests; real pure-code Miri selection | Logical admissions do not bound exact allocator/RSS or preempt native/OS calls |
| Identity/caches / API, INPUT | Canonical source identities, per-extraction resolution reset, bounded positive hints, per-cache metadata snapshots and ordered alias resolution; filesystem/concurrency/quota regressions | Stable checkout required; file links/outside selected paths are permitted; no race-proof confinement |
| CLI/output / CLI | Status 0/1/2, all-worker completion, staged bounded output, native paths and escaped controls; actual process/write/flush tests and subsequent ARM Linux-only controls | OS write failure can leave invalid partial stdout; remaining supported/hosted executions stay G03 |
| Tools / TEST, TOOLS | Strict Clippy, fresh coverage/CRAP, complete subsequent Linux mutation review, 75-test pure-code Miri scope, 60 Python checks and 29 pass/fault/repair controls | L01 is closed by named Linux evidence; coverage/interpreter/mutation results are scoped signals; xtask has separate command controls |
| Delivery/platforms / CI | Locked workspace/all-feature checks, exact MSRV, GitHub-only workflows for five supported native targets, aggregate status and release artifact verification | Current uncommitted tree has no hosted execution/protection evidence; G03 remains open |
| Dependencies/build trust / DEPS | Both fresh locked metadata graphs match manifest/lock hashes and the recorded executable build inputs; fresh cargo-deny evaluates both policies | Metadata and advisories are not source audits; no invented cargo-vet trust/exemptions or reproducibility claim |
| Performance / PERF | Source-bound semantic-checked comparisons and Hotpath profiles; separate normal/dormant/active instrumentation-cost experiment after other checks finish | Warm synthetic local results and profiler cost include command/report work; no customer SLA, universal speedup, per-span exclusive cost or timing gate |
| Data/storage / BUILD, STD | No runtime networking/project execution/upload; local/CI artifact retention policy, measured development storage and explicit review decision | Local evidence can retain paths; operator owns redirected reports; no global cache relocation or automatic deletion |
| Inactive architectures / TYPE, ACTOR, ASYNC, PG | No columnar engine, Tokio actor/service or database boundary has appeared | Reassess before introducing one; catalog availability does not require its tooling |

## Combined validation

The [source-bound combined report](rust-oe-combined-validation.json) records
actual commands, exits, counts, failed/incomplete mutation attempts, reviewed
hangs, fuzz statistics, profiler overhead and source/test/configuration hashes.
The [29-case control report](rust-oe-combined-controls.json) binds its inputs too.
Normal timing starts only after builds, CI, coverage, MSRV, Miri, controls,
sanitizers and all mutation processes have exited. Passing non-mutation checks
and failed mutation review are recorded separately. No new optimization or
before/after speedup is claimed by the instrumentation-cost experiment.

The complete run records 578 caught cases, 134 uncompilable cases and 78 raw
timeouts, with zero survivors. Thirteen timeouts match existing exact-diff loop
reviews. All 65 other cases subsequently passed their own unmutated baselines
and failed actual tests in exact single-worker replays with recorded 1.98.1
execution and two Cargo compilation jobs. The raw outcomes remain intact. One
original timeout occurred during **build**, so the unchanged strict review still
rejects the complete run even though its later replay caught the mutation. No
original build timeout is approved as a detection.

The runner now explicitly limits each Cargo build to two jobs. A second full
attempt reproduces test-startup stalls despite that bound. It was interrupted
after 404 completed cases, rather than spending another full campaign on the
same unresolved host failure. Its incomplete results fail review too. Neither
attempt is current passing full-mutation evidence. A failed diagnostic replay
baseline at doctest startup is also retained with zero credited mutants.

A sampled stalled executable remains in `_dyld_start` before Rust/test-harness
execution with a 96 KiB footprint. Its signature verifies, and it links only
system libraries. Identical-byte fresh-path probes usually stall too; one byte
copy starts, so copying is not an established repair. macOS logs show Gatekeeper
assessments requiring a prompt, but their exact relationship to these stalls is
not established. No host security setting or provenance attribute was removed.
The sampling/probe artifacts support a pre-main launch problem, not a specific
Gatekeeper or machine-capacity root-cause claim.

**L01 was left open at the end of this Mac review.** The later complete Linux
campaign now closes it under the unchanged selection, baseline, compiler and
timeout policy. Every approved timeout replay must still identify the same diff,
pass its baseline, build successfully and fail an actual test. No new exclusion,
hang exemption, longer timeout or rewritten raw outcome approves the Mac failure.

After all verification processes exited, the instrumentation experiment checked
complete JSON equality on every invocation, with 21 rotating/reversed samples
and one warm-up for normal, feature-enabled dormant and active profiling builds.
The saved binaries match their earlier source-bound digests. These measurements
compare profiling modes of the same implementation, not earlier optimization
results or a representative customer workload.

| Workload | Normal median | Dormant median | Active median |
| --- | ---: | ---: | ---: |
| Fan-in, 1000 files | 31.90 ms | 31.89 ms | 35.13 ms |
| Call-heavy, 1000 files | 46.51 ms | 46.55 ms | 48.19 ms |
| Monorepo, 1000 files | 45.98 ms | 44.10 ms | 47.55 ms |
| Directory-index, 1000 files | 69.47 ms | 70.58 ms | 71.25 ms |
| Dense syntax, 64 files | 88.59 ms | 88.30 ms | 92.48 ms |

Active command medians are 2.6–10.1% above normal, including report writing.
Dormant differences range from -4.1% to +1.6%; no statistical-significance,
per-span exclusive-cost or production-latency claim follows.

The accumulated native syntax corpus used the first bounded invocation's entire
duration in startup replay: 1787 executed inputs, zero newly added units. It is
recorded as corpus replay. A separate scratch corpus seeded from the nine stable
repository seeds exercised 2471 inputs in 31 fuzzer seconds and added 773 units.
Neither run changed source or locks. No corpus was discarded or renamed to
conceal a finding. Other fresh campaigns exercise configuration, text adapters
and graph semantics with their selected limits and independent oracles.

Development output already exceeds the advisory 12 GiB review threshold. The
storage decision is to retain evidence and reuse installed toolchains/caches.
The initial plan allowed one full campaign and bounded fuzz/profile outputs;
compiler diagnosis, 65 isolated replays and the interrupted repeat exceeded that
plan. Further full repeats on this host are not selected. The report records
measured categories and available disk; no automatic cleanup is performed.
The mutation investigation also exceeded the proposed 30–60-minute review
window. Its unresolved host failure is now an explicit stopping boundary.

## 2026-11-06 follow-up correctness review

An independent source review of the combined tree, against adopted revision
`19e4133c0461129b0f5fdb286abd7b87511242e2`, found a false-clean defect that the
previous passing checks did not cover:

- **R08 / INPUT-009, CLI-001:** built-in vendor/build exclusions and the
  `scan.exclude`/package `ignore` globs were evaluated on absolute paths. After
  walk roots became canonical, a checkout under an ancestor named `build`,
  `dist`, `node_modules`, `.git` or `.next` was excluded as a whole and reported
  clean (status 0). Workspace glob expansion had the same pre-existing defect.
  `input::walk` now passes root-relative paths to every skip predicate.
  Regression tests: `tests/adherence.rs`
  `excluded_names_above_the_scan_root_do_not_hide_the_checkout`,
  `excluded_names_beneath_the_scan_root_remain_excluded`,
  `workspace_under_an_excluded_ancestor_still_reports_package_cycles`,
  `ignore_globs_match_package_relative_paths`. Three of them fail with the
  fix reverted.
- **INPUT-009 / AGENTS.md no-silent-default:** resolved-target canonicalization
  failures used `unwrap_or(path)`, which could leave a non-canonical vertex and
  drop a cycle edge. They now return `ExtractError::IoError`. Resolution-cache
  quota refusals propagate directly instead of becoming `None`; the later
  sticky budget check was already a backstop. The CLI's first scan operand now
  reports a canonicalization failure like the others.
- **R02 scope:** TypeScript 5.0 array `extends` is accepted, applied in order and
  charged per entry to shared work. It previously failed to parse.
  Regression test: `tsconfig_array_extends_applies_entries_in_order`.

Retained decision: existence probes in import/alias resolution (`is_file`/`is_dir`)
still classify an unreadable candidate as missing (Unresolved), and automatic
tsconfig discovery remains best-effort as documented in analysis limits. Both are
recorded limits, not new exceptions.

## 2026-11-06 second follow-up review

Three further independent reviews (CLI/config/workspace, resolution/graph, and
documentation/tests) against the same adopted revision found and this change
fixed, each with a regression test in `tests/adherence.rs` or `src/output.rs`:

- **R02 / INPUT-004:** directory specifiers (`./b/`, `.`, `..`, alias `@/b/`)
  matched a sibling file before the directory index, inventing cycles Node and
  TypeScript do not have.
- **R01 / INPUT-009, CLI-001 (false clean):** `./`-prefixed workspace globs and
  negations and `./`-prefixed `scan.include` roots matched nothing; a nameless
  explicit manifest reported "No packages found." with status 0.
- **R01:** a member's tsconfig `references` captured the workspace root ahead of
  the npm/pnpm root (status 2 on valid input); a bare `package.json` operand had
  an empty directory (status 2); every extra operand re-expanded the workspace,
  so `--stdin` operand lists exhausted the file quota.
- **R10:** explicit YAML `null` package overrides were accepted as omitted.
- **Documentation/tests:** the README usage, human and JSON samples now match
  actual output; R05 states the existing omitted-`subpath` encoding (unchanged
  since 0.1.1) instead of `null`; R10 states array `extends`; the runbook no
  longer implies a CODEOWNERS file; vacuous tsconfig/timing tests now assert
  results; the tsconfig-extends integration tests state and assert that child
  `paths` replace inherited aliases.

Reviewed and not changed: a closed stdout is reopened on `/dev/null` by the Rust
runtime and cannot be distinguished from an intentional redirect; scope-unaware
`require` recognition and quoted YAML numbers are recorded in analysis limits;
error text keeps `escape_debug` (R06) although it also escapes quotes. A named
workspace root package still contains its members' files (README usage notes);
excluding nested package subtrees would be a product decision, not a repair.

## 2026-11-06 third follow-up review

An adversarial review of the two previous follow-ups, a library/portability
review and short pure-Rust fuzz runs (graph_semantics, text_adapters,
configuration: about 9.2M executions in 360 s, no findings; source_syntax needs
upstream Clang and was not run) found and this change fixed:

- **R01 (false clean, regression from the second follow-up):** pnpm/npm
  precedence let an unrelated workspace higher up capture a TypeScript
  references repo. Precedence now requires that a member package contain the
  references project.
- **R08 (regression from the first follow-up):** `./`-prefixed and absolute
  `scan.exclude`/`ignore` patterns stopped matching; explicit operands and
  automatic scans interpreted relative excludes from different bases.
- **R02:** wildcard alias targets ending in `/` resolved to a sibling file; on
  Windows, multi-segment wildcard suffixes were pasted into one verbatim path
  component and never resolved (hidden cycles). The suffix is now pushed per
  segment; `multi_segment_wildcard_alias_suffixes_resolve` runs in Windows CI.
- **API (API-010):** the legacy `discover_workspace_packages` compared canonical
  members with non-canonical scan paths; public error enums are now
  `#[non_exhaustive]`, as the migration notes promised; `# Errors` docs updated.
- **CLI-003:** invalid `extends` and explicit-null overrides name the expected shape.

Recorded, not changed: directory-symlink limits and Windows JSON separators
(analysis limits); the `IO error:` prefix and single-line `escape_debug`
rendering of multi-line YAML diagnostics remain the existing R06 error format.

## 2026-11-06 fourth follow-up review

An adversarial review of the third follow-up, targeted mutation testing of
every function changed by the follow-ups (82 mutants: all viable mutants caught
after adding host-independent separator tests and restructuring an
optimization-only guard), and a `source_syntax` fuzz run without the C
sanitizer (about 135K executions in 300 s, no findings) found and fixed:

- **R01 (regression from the third follow-up):** after a references project,
  discovery kept expanding every ancestor workspace to `/`, so a broken or
  unrelated `~/package.json` or `~/pnpm-workspace.yaml` failed every references
  repo below it. Only the first package-manager workspace above is now read, by
  its declared patterns without expansion; nameless members count.
- **R08 (regression from the third follow-up):** cwd-relative exclude matching
  re-excluded the contents of explicit operands (`*` crosses separators), and
  an empty or `./` pattern pruned the whole scan root. Relative excludes are
  again walk-root relative, as the README states, and walk roots are never pruned.
- **R02:** alias `.` targets follow TypeScript (extension probing first).

## 2026-11-06 fifth follow-up review (real-repository differential)

The CLI was run against six local repositories (5,461 JS/TS files) and compared
with the HEAD binary. Rust-OE TEST/DEPS rules at
`19e4133c0461129b0f5fdb286abd7b87511242e2`.

- **R04:** the strict syntax policy turned upstream grammar gaps into exit 2 on
  two repositories (valid `String.raw` templates with `\u` escapes and
  `typeof import()` type arguments; 23 files in total). The upstream grammars
  are their latest releases, so the parser moved to WillBooster's pinned forks,
  which accept all 5,458 valid files and still reject the three intentionally
  broken fixtures. Decision by alm: keep strict rejection, change the grammar.
  The MSRV rose to 1.93.0 for the fork runtime; the inventory and build-script
  review are in build-trust.md. Errors now report the first error's location.
- **R01 (decision by alm, nearest package owns files):** a named workspace root
  repeated every member file cycle under its own name. Packages now skip nested
  scanned package directories. Cycles formed only by cross-package relative
  imports remain undetected, as at HEAD; recorded in analysis-limits.md.
- **R01:** duplicate package names from nested worktrees now suggest
  `scan.exclude`.

## 2026-11-06 sixth follow-up review

- **Grammar switch verified at edge level:** the old and new grammars were run
  through the same visitor over the six repositories; no import edge was lost,
  and the only additions come from the 23 files the old grammar rejected.
- **R08/R01:** `scan.exclude` pruned package discovery but not package source
  walks, so a named root still parsed (and failed on) excluded worktrees, which
  also made the duplicate-package hint ineffective. Members removed by
  `--only`/`--exclude` were re-analyzed under the enclosing root. Discovery now
  returns these directories as boundaries that every enclosing package skips.

## 2026-11-06 seventh follow-up review (independent reviewer)

An independent reviewer with fixtures found, and this pass fixed:

- **R01/R08:** `scan.include` globs with a trailing `/` or an absolute spelling,
  and excludes with a trailing `/`, matched nothing (false clean for include).
  Absolute excludes were bypassed by operands spelled with `..`. Spellings are
  now normalized and absolute matching uses the lexical absolute path.
- **R01:** a `package.json` operand removed by a name filter did not keep its
  directory, so the root re-reported its cycles.
- **R01:** a file link into a nested package was analyzed by both packages.
- **R10:** a failing automatic tsconfig silently dropped its aliases, which can
  hide cycles. The tolerance is the recorded R10 decision (uninstalled
  `node_modules` extends are common); it now warns on stderr. Owner alm may
  choose to make it fatal instead.
- Recorded, not changed: cross-package relative cycles (analysis-limits.md) and
  manifest operands not discovering nested packages.

## 2026-11-06 eighth follow-up review (oracles and second reviewer)

- **Oracles:** an independent SCC computation over the CLI's own resolved edges
  matched every package's reported cycles in four repositories (both modes),
  and TypeScript 5.9.3's resolver agreed on 16,836 imports; the remaining 19 are
  CSS files jscycles resolves and tsc does not, which cannot form cycles.
- **R02 (fixed):** `.d.ts` candidates were never probed (missed declaration
  cycles); `.jsx` preferred `.ts` over `.tsx`; exact workspace aliases with extra
  `/` segments were never recognized (false clean); a package importing its own
  name lost the file edge in workspace mode (false clean). Evidence duplicates
  and empty subpaths are removed.
- **Kept:** an exact workspace alias also classifies its subpaths, which keeps
  the common `@x/a` + `@x/a/*` pair as package edges; a nonexistent subpath can
  therefore add a package edge. Closed stdout remains the recorded runtime limit.

## 2026-11-06 ninth follow-up: systematic differential testing

Manual review rounds kept finding new defects, so correctness is now checked
against oracles (tooling/checks.md "Differential testing"): a spelling matrix
in `cargo test`, a generated-project differential harness using TypeScript's
resolver and the contract rules, and real-repository snapshots. The harness
caught HEAD's defects in 82 of 100 cases (validating it) and found four new
defects in the current tree, all fixed with regression tests:

- **R08 (regression from the seventh follow-up):** every file link inside its
  own package was skipped, because the package root was in its nested set.
- **R01:** a link into a sibling package was analyzed by both packages (false
  package edge, duplicate cycle with `--inner`).
- **R02:** an exact alias into a non-member package nested in a member was
  attributed to the member (false package edge, lost self-cycle).
- **R02:** self-imports were judged by the extracting package, not the file's
  location, so a link into an unscanned member changed results between modes.

Model decisions recorded: one tsconfig per package (analysis-limits.md), and
duplicate workspace member names are an error even when unselected. After the
fixes, 3,000 and 2,000-case runs and the 10,000-case gate are clean.

## Stop condition and remaining obligations

The intended local stop condition is a reconciled ledger, a complete passing
mutation review against unchanged inputs, passing named local checks and retained
instrumentation evidence. This Mac pass did not meet it. The later Linux
continuation closes L01 with a fresh complete run and strict review, while
preserving the Mac failures. Further synthetic optimization is not selected.
Full adherence still requires the real operational/release evidence below.
- **G01:** actual private security-reporting route, receivers and supported
  security-update versions remain unverified.
- **G02:** actual privileged principals, publisher/reviewer/tag permissions,
  alternate recovery owner and a recovery/revocation rehearsal remain unverified.
- **G03:** execution of this tree on the other supported hosts, current-stable
  compatibility jobs and enforced GitHub required statuses/review policy remain
  unverified locally.
- **C01:** next-release version/API baseline, clean reviewed source, authorization,
  publication and actual consumer-download verification remain release-time work.

These are explicit outstanding obligations, not satisfied exceptions. No contact,
reviewer, host setting, release or handoff is invented to close them. Their
existence prevents a full-adherence claim.
