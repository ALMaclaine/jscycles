# Engineering tool policy and results

Owner: alm. Rust-OE revision `19e4133c0461129b0f5fdb286abd7b87511242e2`;
TOOLS, TEST, PERF, CI, API, INPUT and CLI practices. This pass implements the user's explicit
Clippy, CRAP, cargo-mutants, Miri and Hotpath requirements. It supersedes earlier
decisions to defer these tools. The initial results are in
[quality-results.json](evidence/quality-results.json); the expanded graph/configuration pass
is recorded in [quality-expansion-results.json](evidence/quality-expansion-results.json).
The current production-wide boundary pass is recorded in
[quality-boundaries-results.json](evidence/quality-boundaries-results.json).
The cursor/extraction and runner pass is recorded in
[quality-syntax-results.json](evidence/quality-syntax-results.json).
The subsequent resolver pass is recorded in
[quality-resolution-results.json](evidence/quality-resolution-results.json). Its mutation
campaign covers the two changed production modules; the earlier 715-case campaign
retains its original input scope.
The next [import-semantics and alias-cache pass](evidence/quality-import-semantics-results.json)
also records the targeted YAML patch update and its fresh sanitizer campaigns.
The [diagnostic and release-identity pass](evidence/quality-cli-release-results.json)
adds complete bidi-label handling and verifies the binary identities that
release smoke commands previously assumed.
The [alias-work and path-policy pass](evidence/quality-resolution-work-results.json)
removes redundant matching admissions and aligns alias dot-segment handling
with relative module resolution.
The [workspace alias pass](evidence/quality-workspace-aliases-results.json) removes
quadratic setup for loaded tables and makes name/path lookup agree when an
alias conflicts with a declared package name.
The [tsconfig cache pass](evidence/quality-tsconfig-cache-results.json) shares raw parent
documents across concurrent children, avoids unused inherited-field copies, and
repairs relative discovery and working-directory cache keys.
The [workspace alias-target pass](evidence/quality-workspace-alias-targets-results.json)
shares repeated exact-target lookups within one setup, preserving general
first-match resolution and charging retained target-cache values.
The [source-walk hint pass](evidence/quality-source-walk-hints-results.json) reuses positive
regular first-index paths during full extraction while retaining source reading
and filesystem fallback.
The [native grammar classification pass](evidence/quality-syntax-kind-results.json)
removes repeated native kind-name conversions, retains every node-range check,
and expands nested-call, mixed-grammar and sanitizer verification.
The [combined-tree review](rust-oe-current-review.md) reconciles the current
requirement map and records fresh checks, profiler overhead, runner repairs and
the failed Mac mutation gate. The subsequent
[Linux verification](rust-oe-linux-verification.md) closes **L01** with the complete
current selection and independent strict reviews under the unchanged policy.
Earlier failed and passing campaigns retain their original source scopes.
Raw artifacts stay under ignored
`target/tooling/quality`, `coverage` and `mutation` directories.

| Command | Enforced scope |
| --- | --- |
| `cargo +1.98.1 xtask clippy` | Workspace, all targets/features; all/pedantic/nursery/cargo and selected restrictions; warnings fail |
| `cargo +1.98.1 xtask crap` | Fresh default-feature workspace LCOV; cargo-crap 0.9.0; scores above 30 fail |
| `cargo +1.98.1 xtask mutants` | cargo-mutants 27.1.0; isolated source copy; all 16 production Rust modules; fresh campaign manifest and full Cargo tests |
| `cargo +1.98.1 xtask miri` | nightly-2026-10-19; actual graph, text, reporting, bounded-reader, path, inherited-option ownership, concurrent quota and staged-output implementations |
| `cargo +1.98.1 xtask profile` | Hotpath 0.28.6; release CLI; checked synthetic workload and fresh local JSON profile |

Install pinned tools with `cargo +1.98.1 xtask install-tools <tool>` where tool is
`coverage`, `crap`, `mutants` or `miri`. Baseline CI still runs independently.
GitHub's required aggregate now includes CRAP and Miri jobs. Weekly/manual
GitHub Actions maintenance runs mutation testing. Hosted execution remains
separate from the completed local runs.

Clippy's broad style, metadata, argument-count, type-complexity and dependency
duplicate exemptions were removed. The `redundant_pub_crate` exception resolves
its conflict with the compiler's `forbid(unreachable_pub)`: internal modules keep
crate visibility. A narrow set-lookup exception preserves admission of an edge
before retaining it. Concurrent test workers must all start before joining;
their deliberate collection has a local explanation. There is no blanket
restriction-group enablement; that group contains contradictory policies.

The original coverage-backed CRAP report had five scores at least 30. The CLI
was split into configuration, workspace discovery, options, analysis and
publication stages. The decoder now separates braced Unicode parsing and code
point encoding. Maintenance dispatch uses a command table, and CI uses a task
sequence. No source subtree or uncovered maintenance code was dropped to pass
the gate. Coverage uses the normal product feature set because timing macros
change LLVM function/closure mappings; the full feature set is checked, linted
and tested separately. Line coverage is not a correctness score.

Mutation runs exposed missing assertions for escaped NUL, escaped JSON strings,
comment boundaries, selected quota/error values and sticky output flush failures.
Regressions exercise those actual contracts. Six exact equivalents are recorded
in [.cargo/mutants.toml](../../.cargo/mutants.toml): four only rescan already blanked
delimiter bytes; two skip a byte after a retained backslash outside a string,
which cannot produce valid JSON. They do not exclude a whole function or file.
The first pass selected 135 mutations, followed by 379 across six modules. The
cursor pass selected 715 mutations across all 15 production modules. The regular
gate enumerates the current production selection afresh on every run.
The 0.2.0 release preparation (2026-11-10) ran a targeted pass over the code
added since that campaign: package `extends` resolution, Windows/absolute
pattern spellings, byte-order-mark stripping, type-only import detection and
edge-evidence display, 80 mutants. Four survived at first. Two exposed missing
tests (string `exports` with a subpath; an existing non-JSON `types` target),
and the second also revealed that condition maps were tried in serde_json's
sorted key order rather than written order, now fixed with an order-preserving
reader; a third test pass found and closed bare condition maps. Two are
equivalent and recorded in `.cargo/mutants.toml` with their reasons. The full
899-mutant campaign was interrupted at 698 (none missed) when the disk filled
and has not been rerun on the final tree.
Uncompilable mutants remain separately
reported. Generated campaign manifests bind source/test/config hashes; the static
[review policy](../../.cargo/mutation-review.json) retains exact reviewed timeout diffs. Only reproduced
nonadvancing scanner loops and quota-bypass enumeration hangs are accepted as
detected hangs. A failing baseline, incomplete run, mismatched/duplicate case identities, surviving
mutant, changed recorded input (including edits during execution), or unknown/build
timeout fails the review. Negative gate tests exercise
those failures. Raw cargo-mutants exit 3 is retained as timeout evidence rather
than being presented as an ordinary all-caught run.

The cursor pass interpreted 69 Miri tests: 29 graph cases, four text tests (including independent JSON
encoding across Unicode scalars), 29 reporting cases, two bounded-reader cases,
one lexical-path case, one native-length admission case, and three algorithm/quota/output cases. The algorithm
case independently checks all 64 directed three-vertex package graphs. These
execute production Rust code. Native tree-sitter calls stay in native
tests and sanitizer fuzzing. The pinned nightly emits deprecation warnings for
lint-key spelling and an atomic API retained for the public Rust 1.92 MSRV.

Hotpath found repeated call-pattern compilation and target canonicalization in
the import path. Immutable patterns are now initialized once per supported
grammar, with initialization errors preserved. Per-extraction resolution caches
retain canonical target identities and are still cleared between extractions.
Worker-count comparisons pointed to contention in three consecutive import-byte
reservations. They now use one checked aggregate admission; aggregate overflow
uses the original checked sequential admissions, which must reject overflow.
The opt-in timing build requires `HOTPATH_OUTPUT_PATH`; default builds contain
no profiler. The profiling workload checks a fresh report with actual sampled
scanner calls, as well as cycle results and exit status. Cloud features are off.

The release comparison alternates saved before/after binaries on the same
10,000-file corpus and checks identical JSON results. Measurements include
process startup and use warm filesystem caches. They support only this local
synthetic workload, with no madge ratio, cold-cache claim or performance SLA.

The final default-worker comparison measured 148.94 ms before and 127.40 ms after
on 10,000 files (14.5% lower median elapsed time), and 16.94 ms before versus
12.96 ms after on 1,000 files (23.5% lower). Both used 21 alternating samples
and checked identical JSON. See [10,000-file data](evidence/quality-performance.json) and
[1,000-file data](evidence/quality-performance-1000.json). Inclusive profiling durations
overlap across functions and workers; they are not summed into elapsed speedups.

## Expanded graph and metadata pass

The four newly examined modules initially produced 44 survivors in 271 mutations.
New consumer regressions cover exact 16 MiB configuration boundaries, package
settings and lexical precedence, deterministic cycle order, multi-file SCC
witnesses, Unicode label byte admission, alias validation, wildcard path suffixes,
extensionless parent configuration lookup, cached parent selection, and workspace
question/class/literal-brace patterns. They also distinguish real package manifests
from directories sharing the same filename.

The new cache regression reproduced a defect in the exact saved pre-fix source:
`discover_shared` could return a previously parsed configuration after directory
cache population exhausted the budget. The regression fails with two work units
remaining before discovery and passes after error propagation was added. The CLI
already checked the scan's sticky budget; this repair also stops the shared cache
helper from returning a known-invalid result. Cache keys and values are admitted
before cloning, and no cache lock spans parsing or file I/O.

Configuration absence now uses the shared metadata classifier. Existing malformed,
nonregular and dangling inputs still fail. Redundant `exists && is_file` probes,
unreachable parent-equality handling and duplicate vendor exclusions were removed.
Glob prefix detection now follows the supported metacharacters instead of treating
literal braces/closing brackets as expansion syntax.

One additional mutation is explicitly excluded: changing `> 2` to `>= 2` before
recomputing a package SCC suffix only processes an extra singleton that cannot
produce a distinct-package cycle. Exact internal work-unit counts are intentionally
not a public stage-by-stage contract. Exhaustive graph oracles and bounded deep-ring
regressions remain the alternative evidence. This optimization exception is distinct
from the six original text-adapter equivalents.

The mutation gate records the exact case-identity digest and hashes 313 production,
fixture, test and configuration inputs. The graph-semantic fuzz target adds independent
five-node path-enumeration and reachability oracles under sanitization. Its
[30-second campaign](evidence/quality-graph-fuzz.json) completed 132210 inputs without a finding,
with 206 MiB peak RSS and unchanged source/locks. That is sampled coverage, not
exhaustive verification of all five-node graphs.

The profiling command now runs both fan-in and monorepo workloads. The latter
includes per-package inherited TypeScript configuration, exact workspace aliases,
wildcard file aliases, one file cycle per package and a package ring. A profile
identified repeated per-pattern quota reservations in workspace alias mapping;
these are now aggregated with checked overflow handling before each lookup.

The full 379-case run caught 331 mutations directly, rejected 34 uncompilable
mutations, and reported 14 timeouts. Thirteen are the reproduced nonprogress/quota
hangs. One cache mutation was caught on an isolated 60-second retry with a fresh
passing baseline and the identical diff. The gate now retries at most three
unknown test timeouts once, verifies their case identities/diffs and successful
build plus actual test failure, and rejects survivors, failed baselines, build
timeouts and repeated timeouts. The raw reports remain intact. The final reviewed
result is 332 caught, 34 uncompilable and 13 reviewed hangs, with zero survivors.

Benchmark fixtures now supply explicit finite configuration, avoiding ambient
project YAML. The 4096-alias stress workload uses a 64000000-unit work allowance
for both binaries; product defaults are unchanged. The 512-alias median measured
33.63 ms before and 34.01 ms after (about 1% higher). The alias-heavy workload
measured 155.45 ms before and 127.57 ms after (17.9% lower elapsed time), with
identical JSON across 15 alternating samples. See the [512-alias comparison](evidence/quality-monorepo-performance.json)
and [4096-alias comparison](evidence/quality-monorepo-large-aliases.json). These remain
local warm synthetic observations with no universal speed or SLA claim.

## Previous production boundary pass

The next pilots examined the remaining eight production modules: 331 mutations
initially left 30 survivors. Consumer regressions in
[source_boundaries.rs](../../tests/source_boundaries.rs) and
[mutation_contracts.rs](../../tests/mutation_contracts.rs) now cover discovery component
boundaries and overrides, disabled TypeScript aliases, symlinked workspace
prefixes, relative imports colliding with workspace names, external-target byte
admission, isolated package graph members, reporting counts/plurals and write
failures. Bounded reads are tested with exact EOF, a growing input, invalid UTF-8,
sentinel overflow and an injected failure after a valid prefix. The native suite
then had 468 target tests and one doctest; exact Rust 1.92 also passed.

Three defects reproduced before repair. Directory resolution used a synthetic
`__index__` cache suffix that collided with an actual `__index__.ts` module;
either import order could hide a real cycle and exit clean. File and directory
cache entries now have separate maps. A directory named `index.ts` could shadow
a real `index.tsx` source and hide its cycle; index candidates must now be regular
files. Finally, `.jsx` incorrectly used the TSX grammar and accepted TypeScript
annotations. It now uses the JavaScript grammar, with valid JSX/TSX cycles also
checked. Successful file resolution no longer repeats `exists` before `is_file`.

That pass used recursive mutation selection (`src/**`), with 705 exact case
identities across its 14 production modules and 317 reviewed input hashes,
including the Cargo helper. New compiled modules therefore change the reviewed
selection. The campaign stays outside the fast merge-request lane: GitHub runs
weekly/manually on GitHub Actions, with 90-minute job limits and seven-day
artifact retention. These configurations have local
syntax checks; hosted execution is not claimed.

One additional exact equivalent is excluded: forcing the optional-metadata
symlink guard true repeats a metadata syscall for ordinary entries, returning
the same classification on the documented stable checkout. The false guard
remains selected and is caught by symlink-prefix behavior. This is separate
from the six text equivalents and the singleton-SCC optimization exception.

Fresh coverage-backed CRAP still has zero scores above the enforced threshold
of 30. The highest score is 30 for the profiling command dispatcher, whose
separate smoke execution is outside Cargo-test LCOV. Its three real workloads
check graph results, fresh reports and measured scanner calls; that execution
is not presented as line coverage. The highest product-function score is 28.

Hotpath now also exercises a directory-index ring with one source per directory.
The benchmark verifies all ring members as well as the closed cycle. The saved
normal-build before/after binaries are compared separately from instrumentation;
the correctness regression with colliding `__index__` names is not used as a
performance baseline because its pre-fix output is wrong.

The complete run recorded 535 direct catches, 104 uncompilable mutations and 66
test timeouts. The automatic gate rejected the excessive unknown timeouts;
its three-case retry limit is unchanged. Thirteen match previously reproduced
nonprogress/quota hangs. All other 53 were caught in explicit single-case
replays with fresh passing baselines and identical diffs. Later replays reused
one owned temporary copy, checking its inputs against the original before/after
every case; no product source was changed in place. The final reviewed result
is 588 caught, 104 uncompilable and 13 reviewed hangs, with zero survivors.

Replays also reproduced an upstream 27.1.0 filtering defect: an exact `--re` can
still select unrelated struct-field deletions. The strict one-case verifier
rejected that output. The runner now adds a synthetic one-line `--in-diff` scope,
then queries `--list --json` and requires exactly the requested case and original
diff before running it. Four new guard tests reject extra cases, changed identities
and changed diffs, and accept a valid selection. Those 35 Python tests passed.

The original 316-input campaign passed review before this runner-only change.
Production Rust, Cargo tests/fixtures/settings and the result-verification
functions remained unchanged. The new selector was then checked against the
actual tool and its negative tests, its input hashes were reviewed, and the
same complete native results passed the strengthened runner's read-only review.

The final 21-sample comparisons checked identical JSON for every before/after
execution. Directory-index medians were 51.81 versus 50.54 ms for 1000 files
and 521.91 versus 515.78 ms for 10000 files. Fan-in medians were 127.85 versus
127.20 ms for 10000 files. These are small changes relative to sample variation;
this pass makes no substantial additional speedup claim. See the
[1000-directory](evidence/quality-boundaries-directory-index-1000.json),
[10000-directory](evidence/quality-boundaries-directory-index-10000.json) and
[fan-in](evidence/quality-boundaries-fan-in-10000.json) comparisons. The correctness fixes
preserve these workloads' output while repairing the separate false-clean cases.

## Cursor extraction and fresh campaign manifests

The literal-form regressions reproduced clean exits for real cycles using dynamic
import options, parenthesized literals/callees, optional or generic require calls,
additional call arguments, and TypeScript import-equals. These are covered in
[import_forms.rs](../../tests/import_forms.rs), alongside negative member/computed/
spread cases and a 2048-parenthesis source. The options and import-equals forms
are grounded in the [ECMAScript import-call grammar](https://tc39.es/ecma262/multipage/ecmascript-language-expressions.html#sec-import-call)
and [TypeScript module reference](https://www.typescriptlang.org/docs/handbook/modules/reference.html#export--and-import--require).
The pinned grammar still rejects unsupported syntax rather than accepting recovery.

The production visitor now walks one native cursor, preserving source ranges and
checking the complete tree before delivering literals. It avoids repeated pattern
searches and debug ancestry checks observed in a stack sample during the deep
source test. This removes ast-grep-core and two pure Rust support packages while
retaining the same parser/facade/grammar versions. Source bytes are admitted as
work before parsing; a checked u32 native length prevents truncating an input
when an operator raises the normal quotas. The boundary test needs no huge
allocation and runs under Miri. Native visitor tests are explicitly excluded
from Miri's substring filter; parser execution remains in native tests/fuzzing.

The source-syntax sanitizer harness now compiles the actual private visitor.
Independent JSON encoding supplies known module values across all three grammars;
static/re-export/dynamic/require/TypeScript forms must contribute exactly once,
while member/computed/spread expressions contribute nothing. Arbitrary source also
reaches production extraction. Corpus and source/lock drift remain checked, with
bounded generated programs, 32 KiB inputs and a 512 MiB process RSS limit.

Fresh mutation executions now enumerate their own exact case set and hash their
current source, Cargo configuration, tests and fixtures. The resulting
`mutants.out/campaign.json` belongs to that execution. Read-only review rejects
changed/deleted/new inputs, incomplete/duplicate/different cases, survivors,
unreviewed hangs and wrong tool versions. The static policy retains only the
pinned tool, source scope and reviewed exact hang diffs; ordinary source edits no
longer require manually rewriting hundreds of historical hashes. The GitHub weekly/manual job has a 180-minute hard limit after the local full
campaign exceeded the earlier allowance; hosted runtime is unverified. The automatic
three-case retry cap remains, and the test timeout is 60 seconds following the
previous host's demonstrated test-start delays.

An exclusive controller lock prevents overlapping runs from replacing pending
metadata. Snapshotting excludes generated Cargo registry trees, including when
CARGO_HOME is placed inside the checkout. Negative tests cover both conditions,
source additions/deletions/edits, policy changes, empty/duplicate selections and
wrong tool versions. All 45 Python tests pass. CI is GitHub Actions only.

The native suite now has 474 target tests and one doctest. Miri passes 69 tests,
and the coverage-backed CRAP maximum is 28 across the full workspace, with no
function at or above 30. The profiling dispatcher was consolidated into a checked
workload loop, including a new call-heavy corpus. Timing comparisons use saved
normal-build binaries, alternating execution and equal-output checks after the
mutation workload finishes. Hosted execution remains separate from local checks.

The complete fresh-manifest campaign selected 715 cases across 15 production
modules and bound 322 current input files. It caught 580 mutations directly,
rejected 99 uncompilable mutations and reported 36 test timeouts. Thirteen match
the reproduced hang diffs. The automatic guard rejected the other 23 as too many
for automatic retries. Each was then caught in a quiet single-case replay with
a fresh passing baseline, identical diff and actual failed-test report. Both the
original and owned temporary source copies were checked after every case; the
temporary copy was removed. Read-only review passed with 603 caught, 99
uncompilable, 13 reviewed hangs and zero survivors. No hang exceptions were added.

The 21-sample alternating normal-build comparisons retained identical JSON on
all executions. Call-heavy medians fell from 35.18 to 32.79 ms at 1000 files
(6.8% lower) and 299.92 to 271.54 ms at 10000 files (9.5% lower). Fan-in medians
were 174.34 versus 176.84 ms, and directory-index medians 632.14 versus 639.29 ms
at 10000 files. Those latter differences are small relative to sample variation;
no general speedup is claimed. The [1000-file call comparison](evidence/quality-syntax-call-heavy-1000.json),
[10000-file call comparison](evidence/quality-syntax-call-heavy-10000.json),
[fan-in](evidence/quality-syntax-fan-in-10000.json) and
[directory-index](evidence/quality-syntax-directory-index-10000.json) reports retain the
binary/corpus hashes and individual samples.

The final [sanitizer campaign](evidence/quality-source-semantics-fuzz.json) began with stable
synthetic seeds, including the deep-parenthesis and missing-form regressions,
and completed 3783 executions in 61 seconds with 995 added corpus units and
230 MiB peak RSS. Source and both lockfiles stayed unchanged. An earlier
generated-corpus replay was archived locally before starting fresh mutations;
its initialization-only executions are not counted as new fuzz coverage here.

## Shared source-file resolution

This pass applies INPUT-007, TEST-003/004 and PERF-001/003 at Rust-OE
`19e4133c0461129b0f5fdb286abd7b87511242e2`, against product R02/R03/R10.
The previous resolver could both hide and invent file cycles: alias completion
replaced `.v2` in `model.v2` with `.ts`, selecting `model.ts` instead of
`model.v2.ts`. Relative `.js` specifiers also failed to reach `.ts` sources, and
could incorrectly reach an unrelated `model.js.ts` instead.

Aliases and relative imports now share the same file-candidate routine. Complete
dotted stems and native path bytes are retained. Recognized source suffixes are
substituted, and explicit modern suffixes stay in the `.mjs`/`.mts` or
`.cjs`/`.cts` family. The configured scan extensions still control source selection.
Existing exact-file precedence remains the project's selected policy, rather
than a claim of complete TypeScript compiler emulation. See the
[TypeScript module reference](https://www.typescriptlang.org/docs/handbook/modules/reference#file-extension-substitution)
for the runtime/source extension relationship and the
[versioned compiler resolver](https://github.com/microsoft/TypeScript/blob/v5.9.3/src/compiler/moduleNameResolver.ts)
for its broader resolution behavior.

Nine new CLI tests exercise exact and wildcard aliases, replacement fallbacks,
competing shorter names, every existing inferred extension, directory indexes,
runtime suffixes, exact-file precedence and incorrect extension-family selection.
The original dotted-alias regressions failed before repair; the additional
relative and modern-family regressions also failed before their respective fixes.
The local baseline now passes 483 target tests, one doctest, exact Rust 1.92.0
MSRV checks, 69 Miri tests and 45 Python guard tests. The fresh workspace CRAP
maximum remains 28; the shared resolver scores 8 with 100% reported line coverage.
That coverage figure is not branch coverage or a correctness proof.

A parser-reuse experiment tested the Hotpath hypothesis that per-file native
parser setup dominated the extraction path. Its four-workload pilot found no
useful elapsed-time improvement and it was removed. Extraction and semantic
fuzz source hashes were restored exactly. The final 21-sample alternating
normal-build comparisons retain identical JSON on all four 10,000-file workloads:
fan-in 129.17 versus 128.43 ms, call-heavy 149.42 versus 149.74 ms, directory-index
537.35 versus 537.60 ms, and monorepo 207.64 versus 208.15 ms. These changes are
within 0.6%; this correctness repair makes no speedup claim. Hotpath remains
optional and its four fresh workload checks pass.

The [resolver evidence](evidence/quality-resolution-results.json) records the fresh mutation
campaign across both changed production modules, command logs, input hashes,
individual timing samples and scope limits. The previous 715-case campaign is
historical evidence for its bound source; it is not relabeled as the current run.

The focused campaign completed all 114 selected mutations: 85 caught, 29
uncompilable, no survivors and no timeouts. It used a fresh passing baseline and
passed both automatic and read-only review with unchanged input hashes. No
exclusions or timeout exceptions were added. Reviewing the old 715-case results
against the new source correctly fails as stale. Ten synthetic before/after CLI
counterexamples cover six hidden cycles and four invented cycles; the saved
baseline is wrong and the current binary correct in every case.

## Import semantics and alias caching

This pass applies INPUT-003/007, TEST-003/004 and PERF-001/003 at Rust-OE
`19e4133c0461129b0f5fdb286abd7b87511242e2`, against product R02/R04/R07/R11/R12.
Fifteen CLI counterexamples reproduced clean results for real cycles through
escaped `require` identifiers and TypeScript assertions or instantiations around
the callee/literal. The current binary reports the cycle in every case.
The [ECMAScript identifier contract](https://tc39.es/ecma262/2024/multipage/ecmascript-language-lexical-grammar.html#sec-names-and-keywords)
and [TypeScript assertion semantics](https://www.typescriptlang.org/docs/handbook/2/everyday-types.html#type-assertions)
explain why the lexical name and underlying expression value must be retained.

The classifier compares Unicode identifier escapes without allocating a decoded
callee. Native cursor traversal unwraps only known transparent expressions and
skips type arguments while locating an angle assertion's value. Member callees,
computed/sequence arguments and strings mentioned only in types remain excluded.
A 512-level assertion regression verifies the iterative Rust walk. Independent
identifier encodings and rejection tests run under Miri; the interpreted total
is now 71. The native baseline passes 491 target tests and one doctest, with exact
Rust 1.92.0 checks and all 45 Python guards passing.

Hotpath identified repeated filesystem lookup/canonicalization in alias
resolution. Per-extraction memoization now retains canonical resolved targets and
matched missing targets in a separate specifier namespace. Workspace package
classification takes precedence; unmatched external names are not retained.
Cache locks do not span I/O. A second lookup after filesystem work prevents
duplicate payload reservations when workers race. Admission failures propagate
as typed limit errors. Regressions cover files appearing/changing/disappearing
between extractions, missing-target classification, payload rejection, and 96
repeated references across sixteen parallel sources. The latter was added after
coverage exposed an unexercised warm-cache path; the interrupted mutation run
was archived and restarted against the stronger Cargo-test scope.

The seven-sample cache pilot held the YAML version fixed and reduced the
10,000-file monorepo median from 187.45 to 129.30 ms (31.0%). A fresh 21-sample
alternating comparison of the final normal binaries measured 195.46 versus
131.41 ms (32.8% lower), with identical JSON throughout. Fan-in, call-heavy and
directory-index medians changed by less than 0.8%. The final binary also contains
the YAML patch described below, so the final comparison measures the combined
change. Four fresh Hotpath workload checks pass. These are warm local synthetic
measurements, not a universal speedup or customer timing budget.

Fresh cargo-deny data rejected yanked serde-saphyr 2.1.0. Cargo.toml now requires
2.1.1, and both independently locked graphs update only that package. The packaged
changelog and five changed deserializer files were reviewed; declared Rust 1.89
support, features, licenses and executable build inputs are unchanged. No advisory
ignore or trust exception was added. Blank/comment-only configuration streams now
use defaults; explicit nulls and invalid shapes still exit 2 without a report.
Three repaired blank-stream CLI probes and six preserved shape rejections retain
the before/after evidence. Both locks and the build-trust inventory were refreshed.

The expanded native semantic fuzz oracle checks seven JavaScript, twenty
TypeScript and eighteen TSX forms per generated string value. Generated programs
are capped at 32 KiB, with a stable worst-expansion seed. The final 60-second
campaign completed 10266 executions and added 1742 units; the fresh configuration
campaign completed 1850797 executions and added 13264 units. Both peaked at
260 MiB RSS and retained unchanged source/lock hashes. These short sanitizer
campaigns are bounded evidence, not exhaustive language or dependency assurance.

Fresh coverage keeps the workspace CRAP maximum at 28. The identifier comparator
scores 7 with 100% reported line coverage; alias memoization scores about 5 with
97.2% line coverage. Line coverage is not branch coverage or a correctness proof.
The [current evidence](evidence/quality-import-semantics-results.json) records the final
mutation selection, reviewed outcomes and exact input hashes separately from
earlier full-tree campaigns and the deliberately interrupted partial run.

The existing `jsonc` cursor-multiplication hang review needed one context update:
adding the identifier helper changed only the declaration following `jsonc` in
the unified diff. The mutation and every context line through the function end
match the historical review, and the fresh case again timed out after a passing
baseline. Its hash was re-anchored, the old hash removed, and the five-module
campaign restarted with a fresh manifest. The logical reviewed-hang set remains
thirteen; no new hang mechanism or mutation exclusion was accepted.

The final five-module campaign completed all 275 selected cases: 233 caught,
33 uncompilable and nine reviewed non-progress hangs, with zero survivors.
Automatic and read-only review both passed against unchanged source/test/control
hashes. The two interrupted attempts remain separate partial artifacts and are
not counted as completed campaigns.

## Argument diagnostics and release identity

This pass applies CLI-003/004/009, INPUT's domain-validation boundary,
STD-042–045 and TEST/PERF at Rust-OE
`19e4133c0461129b0f5fdb286abd7b87511242e2`. Owner remains alm. The
[evidence record](evidence/quality-cli-release-results.json) binds the repaired behavior
and current local checks; earlier campaigns retain their original source scope.

Clap previously embedded raw controls from malformed arguments into diagnostics,
including bidi controls and embedded newlines. Successful parsing now retains
native arguments exactly; only errors are regenerated from visible argument
labels. Clap still owns usage layout, suggestions and styles. The formatter's
shared label operation exposes all twelve Unicode `Bidi_Control` characters,
including the previously missed U+061C/U+200E/U+200F, and line/paragraph separators
U+2028/U+2029. Ordinary Unicode remains unchanged. The property set follows
[Unicode 17 PropList](https://www.unicode.org/Public/17.0.0/ucd/PropList.txt).
`HumanFormatter::escape_label` makes that display conversion available to CLI
consumers; its result must not replace original identity/path data.
Process regressions cover 36 malformed argument cases, valid Unicode/bidi paths,
and preservation of error status, empty stdout and generated layout. Formatter
regressions exercise all twelve bidi controls plus both separators under Miri.
Saved before/after binaries reproduce five additional human-label defects.

Release smoke checks previously treated a successful `--version` command as
version evidence and trusted the requested artifact name as architecture evidence.
Schema-2 records now require exact version output, usable help, the actual known
cycle witness with strict boolean/status agreement, matching executable format/CPU
and native runner, and an unchanged digest across smoke commands. Streaming hashes
replace whole-file allocation. Collection checks the downloaded headers and
recorded version/runner independently of the supplied filename and digest.
Header inspection uses fixed-size reads, including a size-checked PE offset.
The identifiers follow the [Linux ELF definitions](https://github.com/torvalds/linux/blob/master/include/uapi/linux/elf-em.h),
[Apple Mach-O headers](https://github.com/apple-oss-distributions/xnu/blob/main/EXTERNAL_HEADERS/mach-o/loader.h)
and [Microsoft PE format](https://learn.microsoft.com/en-us/windows/win32/debug/pe-format).
These are format/CPU checks, not full ABI validation or independent provenance.
Eighteen release tests include all five format/CPU pairs and malformed headers,
wrong versions/runners, mislabeled binaries, binary replacement during smoke
commands, inconsistent records and forged cycle reports. The actual macOS arm64
binary also passes the builder with explicitly simulated forge identity; other
platform execution and publication still belong to their native GitHub jobs.

Source-package inspection also found that Cargo's unanchored `README.md` include
matched the managed Rust-OE tooling README in a nested directory. Anchoring the
root README removes that unintended file: the verified package now has 320
entries and still includes the production helper and process regressions.
Fresh locked metadata confirms that the package filter changes no dependency,
feature or executable build input; the build-trust manifest fingerprint is
updated. The manifest change also triggers a fresh disposable-copy replay of all
29 selected passing, defective and repaired controls. The post-filter mutation
campaign replaces the earlier completed campaign as the current source binding;
the prior passing run is retained separately.

A measured parser experiment resolved three grammar field names once per parse
and used numeric IDs during traversal. On 10,000 call-heavy files, nine
alternating samples measured 143.95 versus 143.90 ms with identical JSON, a 0.04%
difference. This did not justify keeping the optimization; the final parser is
unchanged. The saved pilot and fresh Hotpath checks are retained without claiming
a new speedup or timing gate.

The final native baseline passes 494 target tests and one doctest; exact MSRV
1.92.0 passes the same selection. All 57 Python checks and 72 Miri tests pass.
Fresh CRAP remains at a workspace maximum of 28, with label escaping at 4
and 100% reported line coverage. The defensive diagnostic fallback is uncovered;
it handles the case where escaped labels unexpectedly become valid arguments.
No implementation-pinned test or mutation exception was added for that fallback.
The fresh campaign covers main/output/terminal: 105 cases, 97 caught and eight
uncompilable, with no survivors or timeouts. Automatic and read-only review pass
against unchanged source/test/control hashes. Initial duplicate selection from
compiling the same helper into two targets was rejected before execution; the
CLI now uses the library helper, leaving one production mutation identity.

## Alias lookup work and dot-segment policy

This pass applies INPUT-003/004, TEST-003/004, PERF-001/003 and R02/R07/R08/R11/R12
at Rust-OE `19e4133c0461129b0f5fdb286abd7b87511242e2`; alm remains owner.
The [source-bound record](evidence/quality-resolution-work-results.json) retains the
counterexamples, normal-binary comparisons, profiling and current checks.

Every full-mode import previously reserved the entire tsconfig pattern and
replacement table before target classification, including relative imports,
workspace imports and warm file-alias hits/misses. The per-pattern reservations
also repeatedly contended on the shared work counter. Admission now precedes
actual uncached alias traversal inside the resolver. Source bytes, import records
and retained payloads remain charged per import. Distinct uncached searches still
exhaust their finite quota with a typed, sticky error. No budget is raised or
disabled. Canonical source paths are copied directly instead of rebuilding a
lexically normalized copy for every retained record.

A 10,000-file monorepo cache-only pilot reduced the nine-sample median from
278.09 to 249.30 ms (10.4%). A separate 21-sample comparison uses 1,000 files in
one package with 4,097 actual file-alias mappings, rather than treating an unused
workspace mapping count as the per-file table size. On the same corpus, the old,
cache-only and final normal binaries measured 771.49, 20.22 and 21.38 ms;
the final median is 97.2% lower than the original. Order rotates/reverses and each
binary gets a warm-up; all complete JSON reports match byte for byte. The final
binary also contains the correctness repair below. These are warm synthetic local
results, not a customer budget, universal speedup or statistical significance claim.
The final 21-sample monorepo comparison measured 277.01 versus 253.22 ms (8.6%
lower); fan-in, call-heavy and directory-index changes were -0.50%, -0.06% and
+0.24%. Fresh Hotpath checks pass on all four standard workloads and the actual
4,097-alias single-package corpus. Instrumented timings are kept separate from
the normal-binary comparisons.

Aliases previously passed uncollapsed replacement paths to filesystem lookup,
while relative modules correctly normalized them first. An absent component
before `..` could hide an alias cycle; crossing a directory symlink before `..`
could hide or invent one. Both paths now use the same native-byte-preserving
lexical helper before candidate lookup, then canonicalize existing targets for
graph identity. Substitution still happens before normalization, so wildcard
suffixes follow the same policy. This follows the
[TypeScript 5.9.3 alias resolver](https://github.com/microsoft/TypeScript/blob/v5.9.3/src/compiler/moduleNameResolver.ts#L3107)
and [Node's path resolution](https://github.com/nodejs/node/blob/v22.15.0/lib/internal/modules/cjs/loader.js#L705).
An independent Node 22.17.0 `require.resolve` probe confirms relative-module
normalization across missing and symlinked components without executing fixture
modules. The exploratory physical-parent interpretation was rejected against
those sources; existing relative behavior and its regression are preserved.

Saved native binaries reproduce four hidden alias cycles and one invented alias
cycle, all repaired with equivalent relative imports unchanged. Two finite-work
CLI probes formerly failed on 96 repeated relative/cached imports and 1,025
aliases; they now complete under the same 50,000-unit limit. Public API
regressions additionally cover workspace precedence over a file alias, matched
missing hits, and distinct uncached pattern/replacement searches that still fail
their 10,000-unit limit. Pure path cases continue under Miri. The symlink cases
run on Unix; this Mac does not establish Windows path behavior.

Strict all-feature Clippy and the native baseline pass: 498 target tests plus
one doctest, exact Rust 1.92.0 tests/doctest, 57 Python checks and 72 Miri tests.
Fresh CRAP remains at a maximum of 28. The shared path helper scores about 6.02
with 92.3% reported line coverage; its leading-CurDir arm is uncovered in the
native selection. No score-only assertion or new lint/mutation exception was
added. The fresh imports/tsconfig campaign completes all 116 selected cases:
86 caught, 30 uncompilable, no survivors and no timeouts. Automatic and read-only
review both pass against unchanged source/test/control hashes. Earlier campaigns
retain their own scopes. Parser/FFI code, dependencies, manifests, check commands
and public doctest examples are unchanged; prior sanitizer/control campaigns
remain historical evidence rather than being relabeled as current runs.

## Workspace alias setup and package identity

This pass applies PERF-001/003, INPUT-003/004 and API-008/010 with R02/R07/R12
at Rust-OE `19e4133c0461129b0f5fdb286abd7b87511242e2`; alm remains owner.
The [evidence record](evidence/quality-workspace-aliases-results.json) binds the native
counterexamples, measured comparisons and current checks.

Hotpath identified workspace `map_aliases` as a remaining setup cost. It resolved
each known exact mapping by searching the complete config table again, giving
quadratic pattern work and conservative admissions. Setup now checks a sufficient
invariant once: nonempty replacements and strictly ordered unique patterns, with
exact entries before wildcard entries. Eligible exact mappings reuse the same
substitution/fallback resolver directly. A public caller can still construct
TsConfig/PathAlias vectors in another order, with duplicates or empty replacement
lists; those retain general first-match resolution. No public field or signature
was removed, and no alias index or long-lived cache was added.

Loop work, selected replacement work, package-membership searches and retained
strings remain admitted. Real exhaustion stays sticky and preserves the previous
published alias map, including after a partial private build. Regressions cover
1,024 exact aliases plus a wildcard under a 10,000-unit mapping budget, early and
partial work failures, payload failure, and three caller-constructed precedence
cases. Native CLI probes repair avoidable quota failures with 1,024 aliases at
50,000 work units and 4,096 aliases at the unchanged default 20 million units.

The canonical-name getter also prioritized an alias over an identically named
real package, while the path getter prioritized the package. A declared package
name now wins consistently. Saved binaries reproduce hidden and invented package
cycles in both full and outer modes; the repair preserves the source graph and
corrects the four statuses/JSON results. A pure public-API identity/path regression
also runs under Miri, including scoped names, subpaths and ordinary aliases.

The nine-sample 10,000-file monorepo pilot measured 244.42 versus 211.08 ms
(13.6% lower), with identical JSON. Final timings and instrumented reports remain
separate in the evidence record; these synthetic local measurements establish no
universal speedup or customer latency budget.
The quiet final comparison alternates 21 samples per normal binary: 249.39 versus
216.56 ms (13.2% lower). Fan-in, call-heavy and directory-index medians change by
+0.29%, +0.35% and +0.09%, with identical JSON throughout. Fresh Hotpath checks
pass on all four standard workloads.

Strict all-feature Clippy, 502 native target tests plus one doctest, exact Rust
1.92.0 tests/doctest, 57 Python checks and 73 Miri tests pass. Fresh workspace CRAP
remains at a maximum of 28. `map_aliases` falls from complexity 18 / CRAP 21.15 to
complexity 13 / CRAP 13.21; its line coverage rises from 78.7% to 89.2%. The direct
lookup eligibility check scores 2 with 100% reported line coverage. The general
resolver's theoretical overflow fallback remains uncovered; no score-only test
or exception is added for an unmaterializable replacement count. Coverage is not
branch coverage or a correctness proof. Dependencies, manifests, parser/FFI,
check commands and public doctest examples remain unchanged; older control and
sanitizer campaigns retain their own input scope.
The final workspace/tsconfig mutation campaign completes 183 cases: 131 caught,
52 uncompilable, no survivors and no timeouts. Automatic and read-only review
pass against unchanged source/test/control hashes; no mutation exclusion or hang
review is added. The checked source package compiles with all 321 intended files,
including the new workspace regression suite.

## Shared tsconfig parents and discovery context

This pass applies PERF-001/003, INPUT-003/004, API-008/010 and TEST-003/004 with
R07/R08/R10/R12 at Rust-OE `19e4133c0461129b0f5fdb286abd7b87511242e2`;
alm remains owner. The [source-bound results](evidence/quality-tsconfig-cache-results.json)
retain the counterexamples, scoped campaigns, measurements and current controls.

Each package's inherited configuration previously reread and parsed a shared
parent, then copied parent mappings even when the child replaced the whole option.
TsConfigCache now shares immutable raw documents by canonical identity, with one
initializer per file after releasing the map mutex. Initializers parse one document
only; parent loading happens outside initialization, so cyclic extends cannot
wait recursively on the same cell. Every inheritance occurrence still checks the
depth-ten limit and parses known parent types. Base URL and paths are selected
independently, empty paths still overrides, and each extends spelling supplies
its own declaration origin. Owned selection moves fields; shared selection copies
only fields the child still needs. No public field/signature or dependency changes.

The cache admits keys, cells and known raw payloads before retention, including
overridden parent strings. A 16-worker regression loads sixteen child documents
and their one shared parent under exactly seventeen file admissions, asserting
each child's targets and empty/independent overrides. Separate cases exercise warm
parent depth checks, extends cycles, failed-initialization retry and malformed
overridden parents. Miri adds pure ownership/field-selection checks. Filesystem
and concurrent physical-read behavior remain native evidence.

Relative automatic discovery previously stopped at the caller directory, and
relative cache keys could reuse another working directory's config. Absolute
context keys repair both. A further counterexample showed a syntactic parent walk
from `caller/../pkg` could descend into the caller and select its unrelated config.
Parent-containing filesystem operands now resolve their actual selected directory
before walking ancestors. Ordinary spellings retain their logical symlink context;
this selection policy is separate from lexical module-specifier normalization.
Saved normal binaries repair hidden cycles for `pkg`, `./pkg` and `../pkg` while
both absolute controls retain the cycle. A Unix public-API regression distinguishes
symlink/parent filesystem selection from lexical collapse and verifies successful
cached discovery after the link and config are removed. Snapshots intentionally
persist; create a new TsConfigCache to reload changed metadata.

Final review also found budget rebinding retained cache entries paid for by the
previous budget. `TsConfigCache::with_budget` now starts empty caches, including
when invoked on a used clone. A separate regression warms a raw parent, with and
without a compiled child, then rebinds to a 512-byte read quota. Both operations
must read/admit their own parent and fail that quota. The original clone remains
healthy with its snapshots and original budget. Ordinary cloning still shares
both cache state and budget. The successful pre-fix campaign remains archived
with its source; subsequent checks and measurements use the rebinding fix.
An isolated copy and separate build directory confirm the new regression fails
against the previous implementation; a shared-build exploratory probe was
discarded. The final native suite passes the same public contract with the fix.

Mutation testing first found a missing quota assertion: removing raw-document
admission passed because existing fixtures rejected later compiled data. A new
child overrides a 5120-byte parent mapping under a 4096-byte payload quota, so only
correct raw retention accounting rejects it. That partial run was stopped and
archived before edits. A subsequent completed 120-case run (75 caught and 45
uncompilable) retains its earlier source scope. The first parent-search repair
could reenter the search indefinitely under two mutants; it was simplified to
resolve directly into the search and the partial campaign archived. No new
mutation exclusion, timeout review, lint exception or raised quota was added.

The final fresh tsconfig campaign completes all 124 selected cases: 79 caught,
45 uncompilable, zero survivors and zero timeouts. Automatic and read-only review
pass against unchanged source/test/control hashes. Strict all-feature Clippy,
511 native target tests plus one doctest, exact Rust 1.92.0 tests/doctest,
57 Python checks and 75 Miri tests pass. Fresh workspace CRAP remains at 28;
the main config compiler scores 21, raw admission about 14.13 and the document
loader about 11.00. Uncovered admission-error lines and the bounded-reader JSONC
error conversion remain visible; line coverage does not establish branch coverage
or correctness. The source package verifies all 322 intended files, including
the new cache regression suite. The changed Miri command selection has a fresh
[29-case control replay](evidence/quality-tsconfig-cache-controls.json).

The focused normal-binary comparison uses 128 files in 64 packages sharing a
4096-mapping parent, with `--inner` to isolate inheritance from workspace alias
setup. Across 21 alternating samples and one warm-up per binary, its median falls
from 24.71 to 13.30 ms (46.2% lower); complete JSON reports match byte for byte.
The four 10000-file controls are effectively flat: monorepo 216.42 to 215.21 ms
(-0.56%), fan-in +0.51%, call-heavy -0.44% and directory-index +0.79%.
The focused result is not a claim of a 46% improvement on large source scans.
These warm synthetic local timings establish no customer budget, cold-cache
result, universal speedup or statistical significance. Builds, mutation runs and
Miri complete before normal timing; instrumented profiling runs separately.
Fresh Hotpath checks pass on all four standard workloads and the focused
inheritance fixture, which records 65 document reads for 64 children and one
shared parent. The normal release binary is restored after profiling. Dependencies,
manifests, public doctest examples and parser/FFI code are unchanged; previous
sanitizer campaigns retain their original source scope.

## Repeated workspace alias targets

This pass applies PERF-001/003, INPUT-003/004, API-008/010 and TEST-003/004 with
R02/R07/R10/R12 at Rust-OE `19e4133c0461129b0f5fdb286abd7b87511242e2`;
alm remains owner. The [source-bound evidence](evidence/quality-workspace-alias-targets-results.json)
retains current checks, scope, comparisons and profiling. Earlier campaigns keep
their own input bindings.

After the preceding lookup and inheritance improvements, Hotpath still attributed
9.17 ms of the 512-alias/16-package synthetic command to workspace alias setup.
The old loop resolved every exact alias independently even when many replacement
lists were identical. Setup now memoizes normalized target paths by the complete
ordered list, only when the existing ordering/uniqueness/nonempty-table invariant
allows direct exact resolution. Keys borrow the immutable config. Values and
known cache records are admitted before retention; lookup hashing consumes work
on every hit/miss, and package membership and published strings retain their
admissions. The temporary cache drops after setup and no quota is raised.

Public caller-created tables retain general first-match lookup. An earlier
wildcard can depend on the alias name, so equal later exact replacement lists do
not justify sharing there. A native public-API regression constructs precisely
that case and requires different target packages. Other regressions cover repeated
and reversed replacement order, first-existing fallback, deepest nested membership,
outside targets and a new first fallback appearing between setup calls. Sixty-four
distinct generated targets exhaust a 4096-byte payload quota without replacing the
published map; a subsequent healthy setup succeeds. These are native filesystem
tests; the unchanged 75-test Miri selection covers its named pure-code scope.

The nine-sample 10000-file/64-package/4096-alias pilot measured 213.95 versus
152.76 ms (28.6% lower). After all checks, builds, mutation and Miri jobs exited,
the final comparison alternates 21 samples per saved normal binary with one
warm-up on each same corpus. Monorepo median falls from 220.32 to 152.96 ms
(30.6% lower). Fan-in, call-heavy and directory-index changes are +0.24%, +0.64%
and +0.55%; complete JSON reports match byte for byte. These warm local synthetic
measurements establish no customer latency budget, cold-cache result, universal
speedup, madge comparison or statistical significance. Instrumented profiling
runs separately from normal timing.
Fresh Hotpath checks pass on all four standard workloads and the large monorepo.
They observe 16 uncached target resolutions for 512 aliases/16 replacement lists
and 64 for 4096 aliases/64 lists. The previous per-alias resolution frequency
follows from the old loop; it was not measured with this newly added function
span. The normal release binary is restored after profiling.

Strict all-feature Clippy, 514 native target tests plus one doctest, exact Rust
1.92.0 tests/doctest, 57 Python checks, 75 Miri tests and a fresh 29-case control
replay pass. Fresh workspace CRAP remains at a maximum of 28. The cache helper
scores 8 with 100% reported line coverage; map_aliases scores about 13.11 with
91.4%. The theoretical replacement-count overflow fallback and several admission
error lines remain uncovered; no score-only assertions or exceptions were added.
Coverage is not branch coverage or correctness proof. The fresh workspace-only
mutation campaign resolves all 78 selected cases: 67 caught and 11 uncompilable,
zero survivors and zero timeouts. Automatic and read-only review pass against
unchanged source/test/control hashes, with no new exclusion or timeout review.
The source package verifies all 323 intended files, including the new regression
suite. Dependencies, manifests, check commands, public examples, tsconfig and
parser/FFI code remain unchanged; prior sanitizer campaigns retain their scope.

## Positive first-index hints from source traversal

This pass applies PERF-001/003, INPUT-003/004, API-008/010 and TEST-003/004 with
R02/R07/R08/R12 at Rust-OE `19e4133c0461129b0f5fdb286abd7b87511242e2`;
alm remains owner. The [source-bound evidence](evidence/quality-source-walk-hints-results.json)
retains measured alternatives, final comparisons, tool scope and controls.

The directory-index profile still spent substantial time resolving directories
whose regular `index.ts` entries had already been observed by source traversal.
Full extraction now admits a positive set of those paths during the canonical-root
walk. Child directory links are not followed. Hint records/path bytes are admitted
before retention and probe work before lookup; hints occupy a separate namespace
and reset on every extraction. Each hint owns one path, rather than a directory
key plus a copied target. No map lock spans traversal. Workspace-only mode builds
no hints. Exact/extension file candidates still win before index lookup. Unknown,
ignored, later-extension, differently spelled and linked indices retain filesystem
fallback; the walk is never evidence of absence.

A combined prototype also bypassed per-source canonicalization and pre-open
metadata using regular-entry classification. Its nine-sample fan-in median
regressed from 127.62 to 147.99 ms (16.0%); changing dispatch still regressed
130.81 to 151.48 ms (15.8%). Both were rejected. Source reading is byte-identical
to the previous pass, and every source retains canonicalization and both regular
file checks. Index-only and smaller storage variants were measured separately;
the final nine-sample directory-index pilot measured 513.14 to 476.12 ms
(7.2% lower). These earlier variants retain their own hashes and workloads.

After all builds, checks, mutation and Miri jobs exited, the final comparison uses
21 alternating samples plus one warm-up per saved normal binary on each same
corpus. The 10000-file directory-index median falls from 515.34 to 481.63 ms
(6.5% lower). Fan-in, call-heavy and monorepo changes are -1.24%, -0.50% and
-0.62%; no general improvement is claimed for those controls. Complete JSON
reports match byte for byte. These warm synthetic local timings establish no
customer budget, cold-cache result, universal speedup, madge comparison or
statistical significance. Instrumented profiling runs separately.
Fresh Hotpath checks observe 1000 and 10000 positive hints with zero uncached
directory-index resolver calls in those workloads. A separate fixture with only
index.tsx observes one uncached lookup and the expected cycle, confirming that
later-index fallback still executes. The normal release binary is restored after
profiling.

Six new native tests cover canonical/Unicode identities, exact-file precedence
and refresh, ignored/later indices, host case behavior, bounded hint retention
with a workspace-only control, linked file origins and directory-link pruning,
and FIFO/dangling-link rejection. The last two cases are Unix-only; the local
case probe observes this host's filesystem rather than asserting case folding.
The unchanged 75-test Miri selection covers its named pure-code scope; these new
filesystem contracts are native evidence.

Strict all-feature Clippy, 520 native target tests plus one doctest, exact Rust
1.92.0 tests/doctest, 57 Python checks, 75 Miri tests and 29 fresh control cases
pass. The first CRAP command could not launch target/debug/xtask while concurrent
root Cargo jobs were active; it never executed CRAP. The failure log is retained,
the stale report discarded, and a retry after those jobs exited passed fresh
coverage/CRAP. Workspace maximum remains 28. Hint retention scores 4 with 100%
reported line coverage; extraction scores about 19.63. Legacy invalid-ignore and
missing-root error lines remain uncovered in this default-feature coverage, as
does one existing directory-cache hit line. No score-only test or exception is
added; line coverage is not branch coverage or a correctness proof.

The fresh imports-only mutation campaign completes all 48 selected cases:
31 caught and 17 uncompilable, zero survivors and zero timeouts. Automatic and
read-only review pass against unchanged source/test/control hashes, without a
new exclusion or timeout review. Source packaging verifies all 324 intended
files. Dependencies, manifests, check commands, public examples, source reading,
workspace/tsconfig and parser/FFI code remain unchanged; older sanitizer and
mutation campaigns retain their original input scope.

## Native grammar classifications and nested traversal

This pass applies PERF-001/003, INPUT-003/004, API-008/010 and TEST-003/004,
plus the FUZZ and FFI boundary practices at Rust-OE
`19e4133c0461129b0f5fdb286abd7b87511242e2`; alm remains owner.
The [source-bound evidence](evidence/quality-syntax-kind-results.json) records current
checks, alternatives, timing samples, compiler setup and native sanitizer scope.

Syntax traversal previously converted native node kinds into names repeatedly
for each visited node. It now derives immutable numeric classifications from
every symbol ID in each linked JavaScript, TypeScript and TSX grammar, including
aliases sharing a name. There are no hardcoded grammar IDs. Three process-lifetime
tables retain only Rust enum values, bounded by linked grammar sizes; parsers,
trees and source data remain local to each file. Checked table access preserves
the existing handling of other node kinds. Every visited node still undergoes
checked UTF-8 source slicing. Consumed literal and identifier text uses the same
typed parse-error helper; invalid identifier ranges now propagate an error.
Source representability/work admissions and whole-tree recovery rejection remain.

Four new native tests cover nested runtime edges in all three grammars and
initialization orders, type-name decoys and transparent wrappers, invalid short
and split-UTF-8 source ranges using a real native node, and a 96-file mixed-grammar
ring under one and four workers. The ring is rescanned after a source change,
requiring the cycle to disappear. The semantic fuzz harness adds a separate
bounded program with nine nested literal edges per grammar, including calls in
arguments, import options, computed outer arguments and exported bodies. Its
independent JSON StringValue oracle, original 256-scalar bound, 7/20/18-edge
programs and 32768-byte per-program bounds are retained.

A job-scoped parser-reuse prototype improved two nine-sample medians by only
about 0.5–0.6%; Hotpath still observed 611 and 681 parser constructions for
1000-file workloads. It was rejected and parser allocation/lifetimes restored.
Other pilots tested removing per-node range checks separately from numeric
metadata. Removing those checks alone improved the dense median by about 0.7%,
which did not justify the change. The final implementation retains all checks;
these alternatives and the chosen pilot keep distinct hashes and measurements.

After all checks, builds, mutation, Miri and sanitizer processes exited, final
normal-binary comparisons used 21 alternating samples and one warm-up per binary
on each same corpus. The 64-file syntax-dense fixture has 1000 ordinary calls per
file and 128 literal edges; its median changes from 27.92 to 27.16 ms (2.7% lower).
The 10000-file fan-in, call-heavy, monorepo and directory-index controls change
by -0.27%, -1.30%, +0.86% and +0.34%. Complete JSON output matches byte for byte.
This supports a modest dense-syntax improvement, with no broad speedup claim.
Warm synthetic local measurements establish no customer budget, cold-cache
result, statistical significance or cross-platform result. Separate instrumented
Hotpath checks pass for all four standard workloads and the dense fixture;
the latter observes 64 extracted files and 128 import resolutions. The normal
release binary is restored and matches the saved measured binary's digest.

Strict all-feature Clippy, 524 native target tests plus one doctest, exact Rust
1.92.0 tests/doctest, 57 Python tests, 75 Miri tests and 29 fresh control cases
pass. Miri retains its named pure Rust scope; native grammar execution is covered
by native tests and the separate sanitizer campaign. Fresh workspace CRAP remains
at a maximum of 28. Traversal scores about 23.09 with 94.5% reported line coverage;
the new metadata and checked-text helpers have 100% reported line coverage.
Native parser-admission and unavailable-tree error lines remain uncovered. No
score-only assertion or exception was added; line coverage is not branch coverage
or a correctness proof.

The initial sanitizer build selected an unwrapped Nix compiler driver and failed
to find SDK headers before executing inputs. The failure is retained as compiler
setup evidence. A retry used the configured C/C++ compiler wrapper after C
stdlib.h and C++ vector syntax-only preflights. Rust and native C address
sanitization then completed 3059 inputs in 31 fuzzer seconds, peaking at 254 MiB
RSS. The 5-second input timeout, 512 MiB RSS limit and 64 MiB ASan quarantine are
unchanged; source and lock hashes match before/after. This bounded campaign is
not exhaustive native-soundness evidence.

The fresh module_syntax-only mutation campaign resolves all 28 selected cases:
21 caught and seven uncompilable, zero survivors and zero timeouts. Automatic
and read-only review pass against unchanged source/test/control hashes, without
new exclusions or timeout reviews. Packaging verifies all 325 intended files,
including the mixed-grammar regression. Imports/resolution, source I/O,
workspace/tsconfig, dependencies, manifests, locks, maintenance commands and
mutation policy are byte-identical to the previous pass; earlier campaigns
retain their original input scopes.

## Combined-tree requirement review

The [current report](evidence/rust-oe-combined-validation.json) verifies all 521 pinned
claim identities across 22 source documents and refreshes 111 stale local
decisions/references. This is separate from evidence that a requirement is
implemented. Inactive domains, justified policy exceptions and unresolved
operational/release obligations retain their own dispositions.

An inherited `RUSTUP_TOOLCHAIN=1.96.1` overrode the repository compiler during a
direct mutation invocation. The incomplete 363-case attempt was interrupted.
The runner now explicitly selects/verifies repository Cargo/rustc and records
their identities in schema-3 manifests; three new regression cases cover
override restoration, a wrong actual compiler and invalid manifest identities.
It also explicitly limits Cargo compilation to two jobs per command. Product
Rust, Rust tests, manifests, locks and mutation exclusion/timeout policy remain
unchanged by this review.

Strict all-feature Clippy, 524 native target tests plus one doctest, exact MSRV
1.92.0 tests/doctest, 75 pure-code Miri tests, 60 Python tests, 29 control cases
and verified 325-file packaging pass. Fresh workspace CRAP maximum is 28 under
the unchanged threshold of 30. Both locked metadata graphs match the recorded
build-input inventory. Four bounded ASan fuzz targets pass; the native target's
accumulated-corpus replay and separate fresh-generation campaign are recorded
with their distinct statistics and unchanged source/lock hashes.

**Full Mac mutation verification did not pass.** The complete 790-case 1.98.1 run
records 578 caught, 134 uncompilable and 78 timeouts, with zero survivors. Thirteen
timeouts match existing reviewed loop diffs. All other 65 cases have exact
single-worker replays with healthy baselines, successful builds and actual
failing tests. One original build timeout still fails the unchanged review
policy despite its later caught replay. A subsequent run with bounded Cargo
compilation was interrupted at 404 cases after reproducing native test launch
stalls. Sampling places one executable in `_dyld_start` before Rust; valid
signatures and inconsistent identical-byte copy probes do not establish a
repair or a precise OS cause. Failed/incomplete results remain intact, with no
new exclusion, hang exemption or longer timeout.

After all verification processes exited, the same-binary normal/dormant/active
profiling experiment checked complete JSON equality across five workloads with
21 samples and a warm-up per mode. Active profiling adds 2.6–10.1% to median
command time on this host, including report writing. Dormant differences range
from -4.1% to +1.6%. These are local synthetic observations, without a timing
gate, significance claim or comparison against previous optimization medians.

This Mac review left **L01** open and stopped repeated runs on the failing host.
The subsequent complete Linux verification below closes it. G01/G02/G03 and
release-time C01 remain open. The original review window was exceeded by mutation
diagnosis, not closed by a full-adherence claim. No further optimization or host
security change is selected.

## Complete Linux mutation verification

The [Linux continuation](rust-oe-linux-verification.md) and its
[evidence](evidence/rust-oe-linux-verification.json) close L01 with the same 790 selected
identities across all 16 production modules. A dedicated ARM Linux VM/container
uses a digest-pinned Rust 1.98.1 image and an archive whose source/test/configuration
hashes match the current tree exactly. No production Rust, tests, manifests,
locks, runner, exclusions or timeout policy changed in this continuation.

The complete run finishes in 53 minutes with 644 raw caught, 134 uncompilable,
12 reviewed timeouts, zero survivors, zero unexplained timeouts and zero build
timeouts. One raw caught case is an existing reviewed nonadvancing `circuit`
loop killed at the container's memory limit, recorded separately from the other
643 actual failing-test detections. Its exact diff matches the existing review;
the single memory-kill event is accounted for. Uncompilable cases have real Rust
syntax/type diagnostics, with no killed compiler or storage failure credited.
Both Linux and host read-only reviews pass against the same source hashes.

Linux passes 526 native target tests plus one doctest, 60 Python tests and
guidance integrity. This includes actual `/dev/full` output-failure and native
non-UTF-8 configuration-origin cases. Linux workspace/all-target/all-feature
Clippy passes with warnings denied. The original build-timeout mutation now
builds in about 9.4 seconds and fails a test in about 0.2 seconds. The unsuccessful
Mac run and interrupted attempts remain failed records, rather than being
combined or relabeled as a passing result.

The ledger refreshes 44 project decisions/limits while preserving all 521 source
identities. Public GitHub responses also establish concrete remaining gaps:
unprotected `main`, empty repository/parent/applicable rules and disabled private
vulnerability reporting on canonical `ALMaclaine/jscycles`. G01/G02/G03 and
release-time C01 stay open. The owned container/VM are stopped, their evidence
and caches retained, and the original Docker context preserved. Further synthetic
optimization is not selected by this verification pass.
