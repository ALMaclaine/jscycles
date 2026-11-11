# Historical repairs toward Rust-OE adherence

The [full audit](rust-oe-full-audit.md) supersedes this selected-repair record as
the scope assessment. This file records evidence for its own inputs and does not
establish full adherence.

Source: `19e4133c0461129b0f5fdb286abd7b87511242e2`. Owner: repository maintainer
alm. Reviewed 2026-11-04 (UTC). The [adoption record](../rust-oe-adoption.md) selects
the practices; this record connects observable contracts to code, tests, and gates.

## Audit findings and repairs

| Contract / rule | Observed gap and resulting behavior | Evidence |
| --- | --- | --- |
| Selection, INPUT-004 / API-008 | `scan.include` was parsed but ignored. Automatic scans now apply recursive include roots relative to the scan directory. Explicit directory operands recurse and override automatic includes; a package.json operand selects one manifest. | Process tests plus filesystem tests for `.`, glob roots, descendants, multiple roots, and empty selection |
| Filter semantics, API-008 / CLI-004 | `--outer` substituted manifest dependency declarations for source imports. It could invent cycles from unused dependencies or miss cycles from undeclared imports. Both output modes now use the same package selection and actual import graph. | Both failure cases reproduced before repair; default and explicit-root process tests now compare statuses and package-cycle JSON |
| Graph identity, INPUT-004 / API-008 | Valid tsconfig aliases could produce a clean result because source/target path spellings differed. Extracted sources and resolved local targets now use canonical absolute paths. The CLI rebases displayed file-cycle paths to the reported package. | Automatic, relative-config, and absolute-config process cases; a library consumer test with a relative root and absolute config |
| Input failure, INPUT-009 / CLI-003 | Unsupported file operands and ignored explicit tsconfig errors could produce a clean scan. Non-directory/non-manifest operands now fail; selected tsconfigs are validated in outer mode too. Conflicting tsconfig flags fail. Diagnostics retain the selected path and underlying cause. | Real process failures, including contextual stderr and empty successful-output stream |
| API meaning, API-010 | File-cycle docs claimed all cycles. They now specify one deterministic representative per cyclic SCC, including self-loops; package analysis enumerates elementary cycles. | Existing graph fixtures and reviewed detector implementation |
| Aggregate inputs/search, INPUT-003/005/009 / CLI-008 | File counts, aggregate bytes, graph scale, recursive traversal, and elementary-cycle explosion lacked scan-wide bounds. The CLI now shares atomic admissions, uses iterative traversal, bounds cycle/work/evidence expansion, and stages output before publication. | [Limits and API contracts](../analysis-limits.md); all 4096 four-vertex graphs, 20000-node small-stack cases, shared-worker and CLI quota failures |
| Adoption integrity, CI-004 / TOOLS-007 | Guidance could drift after a passing Rust build. The offline project guard checks config fingerprints, managed inventory/hashes, dependency closure, and the agent block. | Ten filesystem/config pass/defect/repair tests and control-pilot cases |
| Required scope, CI-011 | Individual jobs lacked one stable combined status. `Required checks` now fails unless Rust, all platform cells, MSRV, and guidance jobs succeed. | Actionlint plus local passing, failed, skipped, and repaired result fixtures |

The import-extraction path contract is a library behavior change alongside the
YAML error-type change already recorded for the next 0.2 release. No release is
published by this work. Canonical paths identify graph nodes; they do not establish
filesystem confinement or protection against concurrent path replacement.

## Every change

Run the same checks used in the required jobs:

```sh
cargo +1.98.1 xtask ci
python3 -B scripts/check_adoption.py
python3 -B -m unittest discover -s scripts -p 'test_*.py'
```

The Python guard requires 3.11+; CI selects 3.13.12. On a machine with an older
system Python, `uv run --python 3.13 --no-project python -B ...` can select a
compatible interpreter. The Rust check command retains its own Rust-only scope.

For a behavior change, identify the triggering practice, state the caller-visible
contract, and add a test that would fail on the defect. Record rule/revision,
compatibility effects, and meaningful exceptions in the change description or
adoption record. The [PR template](../../.github/pull_request_template.md) prompts
for that evidence. Ownership, trait laws, cancellation, and measurement claims
remain semantic review questions; keywords and a green build cannot answer them.

For guidance changes, use `rust-oe plan --diff`, `rust-oe sync`, and
`rust-oe check --offline`. The project guard runs without private source access.
It detects drift against the committed schema-1 lock; review matching changes to
both lock and content normally. Rust-OE retains full source/provenance validation,
update planning, and publication/recovery ownership. A schema change requires
reviewing this guard against the new upstream contract.

After compiler/control changes, replay the passing, defective, and repaired cases:

```sh
python3 scripts/verify-controls.py --output docs/maintainers/evidence/rust-oe-budget-validation.json
```

The [current budget validation report](evidence/rust-oe-budget-validation.json) records the
selected commands, source/config hashes, and actual results. The earlier
[adoption report](evidence/rust-oe-validation.json) and [benchmark](evidence/rust-oe-benchmark.json)
remain historical evidence for their recorded inputs.

## Hosting enforcement and remaining work

The workflow defines `Required checks`; verify the host's main-branch protection
requires pull requests and that exact status. Workflow configuration alone does
not establish merge protection. Hosted execution and protection settings have
not been verified by the local run. Release builds call the same CI workflow.

[Analysis limits](../analysis-limits.md) now bounds shared admissions, graph depth
through explicit stacks, elementary-cycle enumeration, evidence, and encoded
output. A quota failure exits 2 before publishing stdout. These controls do not
supply exact RSS/wall-clock limits or filesystem confinement; hostile remote
scanning still needs process isolation. Native parsers and OS calls are not
preempted by the logical work counter.

Legacy best-effort library loaders and automatic alias discovery retain their
documented fallback policy for ordinary metadata errors. A shared quota failure
always propagates through the CLI. New strict configuration paths use
fallible APIs. The maintainer revalidates that choice on an input/API change.
Specialized fuzzing, Miri, mutation, audit-trust, and performance gates still need
an applicable boundary, owned workload/harness, budget, and real pilot. Their
absence is not replaced with empty jobs or fabricated assurance.


## Historical local validation of the selection audit

Rust 1.98.1 passed the complete `cargo xtask ci` with 374 target tests and one
library doctest. Rust 1.92.0 passed the same tests and doctest. The guard's ten
Python tests and all 17 control-pilot cases passed; recorded input hashes match
the final source/configuration. Source-package verification, offline Rust-OE
checking and actionlint also passed locally.

The [selection-audit experiment](evidence/rust-oe-adherence-benchmark.json) preserved its
known cycle across one warm-up and seven samples (median about 19 ms on this
host). It establishes no customer workload budget or comparative speed claim.
Canonical graph identity adds filesystem lookups, and outer mode now parses
source files for correct package edges instead of relying on manifests. Measure
real affected repositories before making optimization or latency claims.

These are native macOS aarch64 results. Hosted platform jobs, branch protection,
and the Linux-only output-storage failure test remain outside this execution.

## Local validation of the budget implementation

Rust 1.98.1 passed `cargo xtask ci` with 389 target tests and one library doctest;
Rust 1.92.0 passed the same tests and doctest. Ten Python guard tests passed.
The source-hashed [budget control report](evidence/rust-oe-budget-validation.json) records
the fresh pass/defect/repair replay. Earlier reports remain evidence for their
own recorded inputs. Source-package verification passed (about 568 KiB), along
with offline Rust-OE integrity and actionlint checks.
The [budget implementation benchmark](evidence/rust-oe-budget-benchmark.json) retained the
known cycle in one warm-up and seven samples, with a median about 28 ms on this
host. This is a warm synthetic observation, not a performance gate or comparison.
See [analysis limits](../analysis-limits.md) for the regression
cases and precise boundaries.

GitHub merge protection and hosted execution remain unverified. No GitHub API
token or connected browser was available for this session; the configured HTTPS
credential helper did not return within its bounded lookup. The workflow defines
`Required checks`, but local success does not establish that main requires it.
