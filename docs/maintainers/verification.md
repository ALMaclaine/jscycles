# Verification scope and residual-risk worklist

Owner/triage: alm. Rust-OE TEST, FUZZ, CI, INPUT and TOOLS at
`19e4133c0461129b0f5fdb286abd7b87511242e2`.
The [full audit](rust-oe-full-audit.md) owns applicability and open obligations.

## Required baseline

`cargo +1.98.1 xtask ci` checks formatting in the application/tooling and separate
fuzz workspace, locked workspace builds/Clippy/tests/doctest/docs, and cargo-deny
0.19.8 on both the application and fuzz dependency graphs. The fuzz graph shares
the root policy and adds only its version-scoped development license exception.
It is evaluated without requiring a sanitizer build in ordinary CI.

As of the 2026-11-07 release preparation, the native macOS aarch64 baseline has
583 target tests and one public doctest; platform-conditional counts differ elsewhere. Exact
MSRV 1.93.0 runs the selected tests/doctest too. Python 3.11+ runs 62
guard/release/mutation-gate tests. The
[source-hashed control replay](evidence/rust-oe-combined-controls.json) executes 29 real
passing/defective/repaired cases on Unix with `--boundaries`, including missing
development-license approval and three selected boundary faults. The original
default replay retains 20 cases; the Unix path-byte probe is excluded on Windows.
Unsafe-operation probes are compiled only and never invoked.

The [rule evidence pass](rust-oe-closure.md) records the additional configuration,
workspace/identity/path regressions and their exact scope. Native byte preservation
is tested locally; the end-to-end non-UTF-8 config-origin filesystem case requires
Linux because this Mac filesystem rejects the setup. No platform's omitted test
is counted as executed evidence.

`tests/adherence.rs` adds consumer regressions for discarded configuration,
parser recovery, JS string values, JSONC and TypeScript option origins/overrides,
alias precedence/fallbacks, shared workspace alias selection, cache invalidation,
control escaping, verbose summaries, invalid CLI suffixes, dangling configs,
nonregular files, distinct workspaces and duplicate package names. The Unix stdout
fixture explicitly shuts down writes before closing its peer, avoiding reliance
on socket peer-teardown timing. It checks report/help/version errors and diagnostics.
Help/version previously returned success despite an unwritable stdout; the same
fault now exits 2. Existing graph tests include every four-vertex directed graph
without self-edges against an independent enumerator, separate self-loop cases,
and 20000-node chain/ring cases on a 128 KiB stack. The Unicode decoder uses an
independent JSON-encoding oracle. These are observable contracts, not a count-based
claim that every bug class has been exhausted.

## Coverage and complexity triage

Run `cargo +1.98.1 llvm-cov --workspace --all-targets --all-features --locked --json
--output-path target/tooling/coverage/closure.json` with cargo-llvm-cov 0.9.1
and matching LLVM tools. The retained summary is
[rust-oe-closure-coverage.json](evidence/rust-oe-closure-coverage.json). Native C is not part
of that Rust line report; the source_syntax fuzz target separately instruments C.
Inline unit-test source contributes to line percentages. Doctests, external Python
control probes and unexecuted host configurations are outside this coverage run.

| Decision area | Review/action from this audit | Remaining interpretation |
| --- | --- | --- |
| Configuration/discovery | Validate unused overrides and CLI suffixes; reject dangling configs, package-name collisions and unsupported multiple workspaces | Legacy best-effort/public DTO helpers have explicit separate contracts; a line score does not justify deleting them or changing behavior to hit a threshold. |
| Tsconfig/workspace parsing | Consolidate origin/override/JSONC logic, alias target/priority/fallback and explicit selection; eliminate duplicated workspace parsing | Full compiler/runtime module resolution remains outside the product contract. Ordinary automatic metadata fallback is a named compatibility exception. |
| Source/native boundary | Reject recovered syntax; decode real string values; test computed expressions, invalid Unicode/control input and regular-file requirements | Native bugs and hostile concurrent filesystem races need their own isolation/sanitizer evidence, not a Rust coverage percentage. |
| Graph/cycle logic | Independent exhaustive small-graph oracle, dense cycle/work cutoffs and deep small-stack regression | No mathematical proof, exhaustive large-graph campaign or branch-coverage threshold is claimed. |
| Output/CLI | Test all quota modes, changed summary semantics, untrusted controls and actual stdout/help/version failures | Linux /dev/full and non-Mac targets require their selected hosted execution. |
| xtask/tool policy | Real pass/defect/repair commands, including its own formatting/lock/doctest/unsafe/argument and separate fuzz-policy failures; three additional named boundary faults | xtask's zero Cargo-test coverage is not zero external validation. Broad Cargo-only mutants on this path would miss the Python-invoked command contracts. |

The [engineering tool policy and results](quality-engineering.md) supersede the
original tool deferrals. Coverage-backed CRAP scores above 30 now fail the
workspace gate. Mutation testing selects all production Rust modules
in an isolated copy, with source-bound case identities and survivor review.
Miri interprets the actual pure Rust graph, text, reporting, bounded-reader,
lexical-path, inherited-option ownership, concurrent quota and staged-output
implementations (75 selected tests on the local host);
native tree-sitter execution remains in native tests and sanitizer fuzzing.
No external duplicate triage is enabled. Revalidate on a coverage producer,
runner, compiler, boundary or support change.

`tests/tsconfig_cache.rs` covers relative/CWD-qualified discovery and load keys,
parent-component and symlink search context, concurrent shared-parent reads,
independent child overrides, per-occurrence depth/cycle rejection, malformed
overridden parents and retained raw-document quotas. The 16-worker fixture fits
exactly 17 physical document reads. Miri covers the pure owned/shared option
selection and parent-field decisions; native filesystem/concurrency regressions
remain native evidence. See the [source-bound results](evidence/quality-tsconfig-cache-results.json).
An additional quota regression checks rebinding a cache to a new budget with
both raw-parent and compiled-child snapshots; ordinary clones remain healthy.

`tests/workspace_alias_targets.rs` checks repeated ordered replacement lists,
reversed fallbacks, deepest package membership, outside targets, filesystem reload
between setups, name-dependent wildcard precedence in public tables, and sticky
payload exhaustion that preserves the previously published map. These filesystem
regressions are native evidence; Miri's pure-code selection remains 75 tests.

`tests/source_walk_hints.rs` adds six native cases for canonical regular identities,
file/index precedence and refresh, ignored/later indices and host case behavior,
bounded hint retention with a workspace-only control, symlink physical origins
and pruned directory links, and FIFO/dangling-link rejection. The last two run
on Unix. These filesystem tests are native; the pure Miri selection stays at 75.

The syntax-kind pass adds native checks for nested runtime calls, type-name
decoys and wrappers, rejected short/split-UTF-8 text ranges, and mixed JS/TS/TSX
graphs under one/four-thread pools and changed sources. All-node range and syntax
rejection checks remain active. See [current evidence](evidence/quality-syntax-kind-results.json).

## Owned fuzz targets

Use cargo-fuzz **0.13.2**, libfuzzer-sys **0.4.13**, and
**nightly-2026-06-20**. The separate fuzz manifest and Cargo.lock are committed
inputs. Install the named tools explicitly; scripts/fuzz.py rejects mismatches,
validates locked metadata and rejects any source/lock drift during a run.

- `configuration`: UTF-8 YAML up to 32768 bytes into Config::parse; no panic,
  hang or resource-exhaustion result within the process budgets. No filesystem,
  network, clock, credentials or application code is invoked.
- `source_syntax`: UTF-8 text up to 32768 bytes through the actual production
  JS/TS/TSX cursor visitor, with its source-range checks and work admission.
  Generate static, re-export, dynamic-options, optional/parenthesized/generic
  require and TypeScript import-equals/type forms from up to 256 Unicode scalars.
  Escaped require identifiers and transparent assertions/instantiations contribute
  to seven JavaScript, twenty TypeScript and eighteen TSX forms per generated value.
  Independent JSON encoding fixes the expected module values and edge counts;
  member/computed/spread forms must contribute no edges. Generated programs are
  capped at 32768 bytes, including a stable worst-expansion seed. No project
  filesystem resolution is executed. This now
  exercises extraction semantics as well as parser/provider ownership; it does
  not establish full compiler/runtime module resolution.
  A separate bounded nested-call program adds nine literal edges in each grammar:
  additional arguments/options, an IIFE, computed outer calls and exported
  function/arrow bodies. The original 256-scalar limit and 7/20/18-form programs
  remain intact; each program is independently capped at 32768 bytes.
- `text_adapters`: the actual private production syntax module is compiled with
  a path include, avoiding a public API added only for testing. Arbitrary UTF-8
  reaches the literal/JSONC adapters; independent JSON encoding and UTF-16 braced
  unit encodings must preserve string values. JSONC must preserve byte offsets
  and strict-JSON semantics. The generated UTF-16 literal is capped at 256 Unicode
  scalars / 512 code units. The input cap remains 32768 bytes. No filesystem,
  network, clock or subprocess is used in the harness.
  JSON-generated/annotated buffers are bounded by six times input bytes plus
  64 bytes; generated comments/trailing commas must normalize successfully and
  retain the original string value. Valid strict JSON must normalize successfully.

- `graph_semantics`: at most five vertices and 25 directed edges, from at most
  16 input bytes. Compare actual bounded production detectors with an independent
  depth-five elementary-path oracle (at most 84 package cycles) and a transitive
  closure oracle for file SCCs. Include Unicode labels, self-loops, canonical
  ordering, multi-file witnesses, and complete-success/sticky-failure behavior
  under varied positive quotas. This harness performs no project reads, network
  access or subprocesses. The [30-second sanitizer campaign](evidence/quality-graph-fuzz.json)
  completed 132210 inputs with no finding and 206 MiB peak RSS; source and locks
  stayed unchanged. Sampling five-node graphs is not exhaustive verification.

All four targets have stable synthetic seeds under fuzz/corpus. Generated corpus and
crash files go under ignored target/tooling/fuzz, preserving checked-in seed
inputs. The runner selects 30 seconds (operator range 1..600), a five-second
per-input timeout and 512 MiB RSS; a total subprocess timeout includes build time.
Rust and native C use address sanitization; C also has coverage instrumentation
through `-fsanitize=fuzzer-no-link,address`. ASan's freed-allocation quarantine
is explicitly limited to 64 MiB and recorded. Campaigns link the library;
CLI process/filesystem behavior has separate tests.

Example after installing the selected tools and an upstream LLVM Clang:

```sh
cargo +1.98.1 install cargo-fuzz --version 0.13.2 --locked
# Select compatible CC/CXX executable paths; do not change machine-global Cargo settings.
CC=/path/to/clang CXX=/path/to/clang++ python3 scripts/fuzz.py --target configuration
CC=/path/to/clang CXX=/path/to/clang++ python3 scripts/fuzz.py --target source_syntax
CC=/path/to/clang CXX=/path/to/clang++ python3 scripts/fuzz.py --target text_adapters
python3 scripts/fuzz.py --target source_syntax --replay /absolute/path/to/artifact
```

Native C sanitization was verified locally with upstream Clang 21.1.8 and Rust
LLVM 22.1.7 on macOS aarch64, setting MACOSX_DEPLOYMENT_TARGET=15.0 in that process.
Apple Clang 17 emitted a different ASan runtime-version symbol and failed to
link; the runner now rejects that setup with a diagnostic. A supported Linux
Clang environment needs its own run; Windows is not an owned fuzz execution lane.
No new global compiler/cache configuration is installed by the runner.
Use a configured compiler driver/wrapper that can locate the macOS SDK and C/C++
standard headers. Clang's reported InstalledDir may identify an unwrapped binary
without those include paths. A failed native build executes no fuzz inputs; retain
the failure and rerun with the configured compiler, without relabeling it as a
parser finding or a successful sanitizer campaign.

The first Rust-sanitized configuration campaign hit the original 512 MiB RSS
limit after 389430 units (peak 527 MiB). Its 21-byte malformed input replayed in a
fresh process at 53 MiB without reproducing OOM. The malformed bytes are an
ordinary rejection regression. The artifact was classified as campaign/sanitizer
resource pressure, not a confirmed input-triggered parser defect; it was not
silently counted as a passing campaign. Later runs with the explicit quarantine,
compatible C instrumentation and immutable inputs are recorded in
[configuration evidence](evidence/rust-oe-closure-fuzz-configuration.json) and
[native syntax evidence](evidence/rust-oe-closure-fuzz-syntax.json). Those reports retain
actual commands, tool/compiler flags, input hashes, exits, runtime and final
libFuzzer statistics. They are short smoke campaigns, not long-run assurance.
The subsequent [text-adapter campaign](evidence/rust-oe-closure-fuzz-text.json) follows
reproduced valid Unicode values that the earlier native-tree target never decoded.
It exercises the owned adapters with semantic oracles instead of treating native
tree traversal as evidence for those functions. Its added serde_json oracle was
already in the independent lock graph; only the local fuzz package's direct
dependency list changed, with no resolved-version updates.

On a finding, retain the artifact locally, replay with the same graph/compiler,
minimize a reproducible failure, classify it, repair the actual behavior or record
its deliberate unsupported boundary, and promote a stable non-sensitive
reproducer to an ordinary test. Do not upload private source/artifacts to an
unapproved provider. alm owns triage before extending these pilots into a required
or scheduled CI lane; no empty generic fuzz job is introduced.
