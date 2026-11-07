# Workspace check contract

Owner: alm. Rust-OE source: `19e4133c0461129b0f5fdb286abd7b87511242e2`.
The [full audit](../docs/maintainers/rust-oe-full-audit.md) owns the current applicability
assessment and open obligations; the adoption record retains selection history.

## Selected configuration

Development and required lint/format checks use Rust 1.98.1, edition 2024,
resolver 3. The public minimum compiler is 1.93.0 (raised from 1.92.0 for the willbooster-tree-sitter runtime). Cargo.lock is committed;
the xtask launcher and build/lint/test/doc/dependency commands require `--locked`.
Dependency edits refresh the lock intentionally before running checks. An explicit
`RUSTUP_TOOLCHAIN` or `cargo +VERSION` overrides the toolchain file; use
`cargo +1.98.1 xtask ci` to select the documented development checks. The optional local timing feature, `profiling`, is disabled by default. These flags select every workspace package
and Cargo target kind, not every operating-system target.

| Command | Contract |
| --- | --- |
| `cargo xtask fmt` | `cargo fmt --all --check` plus separate fuzz-workspace formatting, including xtask |
| `cargo xtask check` | Workspace, all Cargo targets/features, locked build check |
| `cargo xtask fuzz-check` (step of `ci`) | Stable, locked `cargo check` of every fuzz target with `-D warnings`; running a campaign still needs the pinned nightly |
| `cargo xtask clippy` | Same selection; all/pedantic/nursery/cargo and selected restrictions; `-D warnings` |
| `cargo xtask test` | Workspace unit, binary, integration, example/benchmark tests |
| `cargo xtask doctest` | Separately execute library examples with all features |
| `cargo xtask deny` | cargo-deny 0.19.8, locked application and separate fuzz graphs with all features |
| `cargo xtask doc` | Build workspace documentation without dependency docs |
| `cargo xtask ci` | All eight steps above; stop at the first failure |
| `cargo xtask coverage` | cargo-llvm-cov 0.9.1; default-feature workspace target tests; LCOV at `target/tooling/coverage/lcov.info` |
| `cargo xtask crap` | cargo-crap 0.9.0; fresh coverage; fail above 30; JSON at `target/tooling/coverage/crap.json` |
| `cargo xtask mutants` | cargo-mutants 27.1.0; all production Rust modules in an isolated copy; fresh source/case manifest and strict outcome review |
| `cargo xtask miri` | nightly-2026-10-19; actual pure Rust graph/text/reporting/reader/path/quota/output tests |
| `cargo xtask profile` | Hotpath 0.28.6 release build; checked fan-in, call-heavy, monorepo and directory-index workloads; local JSON reports |

The mutation runner explicitly selects the repository's `rust-toolchain.toml`
compiler and verifies actual Cargo/rustc identities before selection/execution.
Inherited `RUSTUP_TOOLCHAIN` values do not override fresh mutations or timeout
retries. Schema-3 campaign manifests retain those identities; missing/mislabeled
compiler records fail review. Ordinary read-only review does not launch a compiler.
Historical schema-2 campaigns retain their original scope without retroactively
claiming a compiler identity they did not record.
Fresh campaigns and retries also pass `--jobs=2` to each Cargo build, in addition
to the two mutant workers. This is an explicit compilation bound, not a cure for
host launch failures. The [combined review](../docs/maintainers/rust-oe-current-review.md)
records the failed Mac gate and pre-harness launch stalls. The subsequent
[Linux verification](../docs/maintainers/rust-oe-linux-verification.md) closes L01 with a fresh
complete 790-case run and independent reviews against unchanged inputs/policy.
Its 12 reviewed timeouts and one resource-killed reviewed loop remain distinct
from failing tests and uncompilable cases; the earlier failures retain their scope.

Baseline commands reject extra arguments. Optional coverage has no percentage
threshold and does not include doctests. Install the baseline tool explicitly
with `cargo xtask install-tools`; install coverage plus matching LLVM tools with
`cargo xtask install-tools coverage`. Missing/mismatched tools and failed
installations fail. Advisory fetching requires network access and its freshness
is distinct from the locked graph. Locking does not make artifacts reproducible.

The separate required guidance job uses Python 3.13.12 (minimum 3.11):
`python3 -B scripts/check_adoption.py` validates the local schema-1 adoption
fingerprint, managed hashes/inventory, dependency closure, and agent block;
`python3 -B -m unittest discover -s scripts -p 'test_*.py'` exercises its failures
and repairs. No private source credentials or network are needed by that guard.
Use Rust-OE itself for full source/provenance validation and updates.

## CI and release support

GitHub is the origin forge. Required Linux checks invoke `cargo xtask ci`.
`Required checks` combines Rust, platform, MSRV, guidance, CRAP, and Miri outcomes and rejects
failed, cancelled, or skipped prerequisites. Main-branch protection must require
that status to make it a merge gate. The 2026-11-06 public API snapshot confirms
unprotected `main`, no repository/parent rulesets and no applicable branch rules;
the configured status is not currently enforced. The
[Linux/forge verification record](../docs/maintainers/evidence/rust-oe-linux-verification.json) retains
those responses. This local change does not modify GitHub settings.
The [rule evidence pass](../docs/maintainers/rust-oe-closure.md) maps the latest boundary
repairs to code and regressions; earlier adherence records retain their scope.
Native test/doctest jobs cover Linux aarch64, macOS x86_64/aarch64, and Windows
x86_64 with explicit target triples. These match the five distributed release
targets; musl is not a release/support promise. An additional Linux lane tests
the exact MSRV; it sets `RUSTUP_TOOLCHAIN` because `rust-toolchain.toml` would
otherwise override the installed compiler, and asserts the active `rustc`. Hosted runner execution and branch-protection settings require
forge verification; local macOS results are not evidence for another platform.

The compatibility-maintenance workflow runs current stable (also forced through
`RUSTUP_TOOLCHAIN`) with the locked graph
and with freshly resolved latest allowed dependencies each week or on request.
The latest lane explicitly allows dependencies above the MSRV. These jobs are
advisory; alm triages failures and makes dependency/compiler changes in a normal
reviewed change. The normal build and optional profiling feature are both
selected by the all-features baseline. Actions are pinned to source commits; hosted OS images still move.

Releases run the required CI workflow before building. Release publication is
limited to version tags or explicit manual invocation on a tag, the tagged commit
must be reachable from `main`, and a partial re-run may reuse artifacts from an
earlier attempt of the same workflow run, with artifact checksums
and source/version metadata. Tag and manifest versions must agree. Local changes
do not publish anything. GitHub Actions owns the repository's CI and maintenance
workflows.
Release build records additionally require exact binary version output, expected
format/CPU and native runner, usable help, a typed known-cycle witness, and an
unchanged binary digest across smoke commands. Collection independently checks
downloaded format/CPU, recorded version/runner, inventory, identity and hashes.
These unsigned schema-2 records do not verify a complete ABI or establish
independent provenance; see the [release runbook](../docs/maintainers/maintainer-runbook.md).

## Lint policy and exceptions

Cargo.toml is authoritative. The application retains its compiler restrictions,
including forbidding unsafe code and implicit unsafe operations. Both product
and maintenance code enforce Clippy all/pedantic/nursery/cargo with warnings
denied. Selected restrictions are explicit in Cargo.toml; the contradictory
restriction group is not enabled wholesale. Broad style, metadata, argument and
type-complexity allowances have been removed. The documented compiler-visibility
conflict and local quota/concurrency exceptions remain narrow and justified.
Tests permit expect/unwrap/panic and indexing; helpers retain explicit reasons.
Passing Clippy is not a semantic code review. See the current
[engineering record](../docs/maintainers/quality-engineering.md) for tool scope and evidence.

## Boundary and failure policy

Configuration bootstrap is capped at 16 MiB. Source/metadata files are regular
UTF-8 inputs, defaulting to 16 MiB each, checked from opened-file metadata and
while reading. [Analysis limits](../docs/analysis-limits.md) defines the shared
entry/file/byte/import/node/edge/cycle/work/payload/output quotas and exact units.
YAML uses serde-saphyr 2.1.1's
default parsing/alias budgets and duplicate-key rejection. The jscycles schema
is closed; package.json, pnpm, and tsconfig inputs remain open to unrelated
ecosystem fields. Tsconfig extends is limited to ten configurations. Stdin is
bounded to 1 MiB and 10000 nonempty paths; read/UTF-8/limit failures are errors.

Directory-walk errors fail the scan. Child-directory symlinks are not followed; selected roots may be canonicalized; local
file symlinks can resolve to regular files. Paths can select outside the current
directory. These are local checkout semantics, not a containment/security boundary
against a concurrent hostile filesystem. No project code or package script runs.
Rayon owns parallel scan completion; a worker error prevents a successful report.

Config parse/read failures and explicitly selected missing configs fail. An
absent default config uses defaults. The public legacy `Config::load_or_default`
retains its best-effort fallback; new callers should use `try_load_or_default`.
Auto-discovered tsconfig and optional workspace aliases remain best-effort for
ordinary input errors; an exhausted shared budget always fails the scan.
An explicit `--tsconfig` is validated in every output mode and
conflicts with `--no-tsconfig`. Outer mode reads source imports while skipping
file-cycle search and file-target resolution. Both modes use the same package
selection and package graph. Automatic discovery applies `scan.include` recursively;
explicit directories bypass includes, while package.json operands select one manifest.

Reports are staged under the byte quota before stdout is written. Quota failure
publishes no stdout; a subsequent OS output failure can still leave partial data.
Stdout is a stream; no artifact rename is owned by the CLI. Exit statuses remain
0 clean, 1 cycles, 2 errors, including write/flush/broken-pipe errors. Partial
stdout is invalid on error. Diagnostics go to stderr. OS termination retains its
platform signal behavior; the standard panic report distinguishes bugs from
ordinary errors. There is no custom crash uploader or telemetry endpoint.

## Differential testing

Correctness of discovery, ownership and resolution is checked against oracles
rather than by review alone (Rust-OE TEST rules at
`19e4133c0461129b0f5fdb286abd7b87511242e2`):

- `tests/spelling_matrix.rs` (in `cargo test`) requires every path-taking input
  (operands, `scan.include`, `scan.exclude`, package `ignore`, `--config`,
  `--tsconfig`) to give the same result for `x`, `./x`, `x/`, absolute and `..`
  spellings, against non-trivial baselines.
- `python3 scripts/differential/difftest.py --cases N [--seed S]` generates
  random projects (members, nested packages, workspace and file aliases,
  package/inherited tsconfigs, excludes, includes, ignores, filters, operands,
  file and directory links) and compares `jscycles --json`, `--inner` and
  `--outer` with a reference model that resolves files with TypeScript 5.9.3's
  own resolver and applies R01/R02/R08 directly. Mismatches are shrunk to
  minimal projects under `target/tooling/differential/failures/`; `--replay S`
  regenerates one case. It needs Node.js and installs TypeScript under
  `target/tooling/` on first use, so it is not part of `cargo xtask ci`.
- The generator also covers `.mts`/`.cts`/`.mjs`/`.cjs` sources, `--extensions`
  and YAML extension overrides, tsconfig `extends` through `node_modules`
  packages (file paths, `tsconfig` fields, `exports` maps, subpaths and
  conditions), type-only imports with `--skip-type-imports` and YAML
  overrides, byte-order marks with CRLF, case-variant specifiers (resolved by
  the host filesystem's rules), and on Windows `\` spellings of operands and
  excludes. CI's `differential` job runs 1,000 cases on Linux and Windows and
  uploads shrunk failures.
- `python3 scripts/differential/real_repos.py record|check BASELINE REPO...`
  snapshots default/`--inner`/`--outer` results on local repositories and
  reports every change. Keep baselines outside the repository.

Stop rule for behavior changes in these areas: the spelling matrix passes, a
fresh `difftest.py` run of at least 10,000 cases reports no mismatch kinds, and
`real_repos.py check` differences are each explained. A model change that makes
a mismatch disappear must cite the contract rule it implements.

### Windows from macOS or Linux

Windows runtime behavior is tested only by CI (`platforms` and `differential`
on `windows-2025`). Locally, cfg(windows) code can be compiled and linted with
zig as the C cross compiler:

```sh
T=$PWD/target/tooling/windows
python3 -m venv $T/venv && $T/venv/bin/pip install ziglang==0.13.0
rustup target add x86_64-pc-windows-gnu
# $T/zig runs `python -m ziglang`; $T/zigcc runs `zig cc -target
# x86_64-windows-gnu` after dropping the cc crate's `--target=` argument.
CC_x86_64_pc_windows_gnu=$T/zigcc AR_x86_64_pc_windows_gnu="$T/zig ar" \
  cargo clippy --target x86_64-pc-windows-gnu --workspace --all-targets --all-features --locked -- -D warnings
```

## Verification and remaining scope

[Current control evidence](../docs/maintainers/evidence/rust-oe-combined-controls.json) comes from
`python3 scripts/verify-controls.py --boundaries --output docs/maintainers/evidence/rust-oe-combined-controls.json`
with Python 3.11+. It runs real xtask commands in disposable
copies for passing, injected formatting/lock/doctest/unsafe-operation failures,
and repaired cases. Unsafe probes are compiled only and never called; production
retains its safe-only policy. This evidence must be regenerated after relevant
compiler, command, manifest, formatter, or public-example changes.
The optional boundary selection adds three named disposable-copy fault probes
on Unix (package identity, component globs and native path bytes), for 29 cases;
Windows omits the Unix-only path-byte probe. It is scoped test evidence rather
than a general mutation-coverage gate. The report checks the original inputs
remain unchanged after the commands.

Native process tests cover config errors, explicit missing inputs, stdin invalid
UTF-8, exit/JSON agreement, CLI extension precedence, and closed stdout. A Linux
`/dev/full` test covers a real storage/output failure. Unit tests check exact byte
and path limits and an input error after a valid prefix. Existing fixtures and
snapshots remain ordinary correctness evidence. Coverage reports identify
unexercised code; they do not prove correctness or coverage of other targets.

The CLI shares finite aggregate quotas and uses iterative graph traversal;
[boundary regression tests](../tests/budgets.rs) exercise quota failures, exhaustive
small graphs, and deep graphs on a small stack. Counters bound admissions and
logical work rather than exact process RSS or elapsed time. Native parsing and
filesystem operations are not preempted. Hosted hostile-repository scanning still
needs OS resource limits and isolation. The [verification record](../docs/maintainers/verification.md) selects owned configuration
and native syntax fuzz pilots. The [engineering tool record](../docs/maintainers/quality-engineering.md)
adds required CRAP and Miri CI jobs, mutation campaigns and measured Hotpath
profiling. No third-party Vet trust, semver baseline or performance threshold is
claimed.
The Miri selection includes pure owned/shared tsconfig option selection and
independent inheritance masks. Cached-document concurrency, filesystem origins
and physical read counts run in the native suite.
Revisit on remote-input exposure, new public stability promises, or evidence of
a relevant failure. Performance experiments use `scripts/benchmark.py`; synthetic
timings do not establish a madge speedup or a representative product budget.
