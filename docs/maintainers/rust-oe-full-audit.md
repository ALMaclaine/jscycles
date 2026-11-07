# Full Rust-OE applicability and adherence audit

The subsequent [engineering tool pass](quality-engineering.md) supersedes this
record's Clippy/CRAP/mutation/Miri/Hotpath deferrals. Earlier numeric validation
reports retain their recorded source scope.
The [combined-tree review](rust-oe-current-review.md) reconciles the claim ledger
with those subsequent changes and records fresh checks plus the failed
Mac mutation gate. The [Linux continuation](rust-oe-linux-verification.md) closes
L01 with a fresh complete campaign under the unchanged policy and records
verified gaps in GitHub enforcement and private reporting.

**Full adherence has not been established.** Earlier records documented selected
repairs and passing checks. They were not a complete adoption assessment.
This audit expands to the complete source corpus at
`19e4133c0461129b0f5fdb286abd7b87511242e2`, including the engineering standards,
product-domain activation matrix, all 17 practices, adoption/review guidance and
control register. Owner: alm. Reviewed 2026-11-04.

The source ledger contains 521 claim records across 22 documents. They include
risks, context, conditional rules, upstream facts and Rust-OE/template maintenance
rules; they are not 521 mandatory product gates. The accompanying
[rules inventory](evidence/rust-oe-rule-inventory.json) retains every source ID/hash/section
and maps it to this scope assessment. Inventory completeness is not semantic
compliance, and no compliance percentage is derived from it.
The subsequent [rule-by-rule evidence review](rust-oe-closure.md) began with
configuration/workspace counterexamples and continued through the remaining
practices. Each source claim now has an explicit scoped evidence, context,
conditional, profile or operational disposition. This remains review evidence,
not a proof of every implementation or a full-adherence certification.

## Practice applicability

| Practice | Decision for jscycles | Evidence and remaining limit |
| --- | --- | --- |
| AUTH | Applicable: distinguish upstream contracts, local choices and tool evidence | Immutable source pins; explicit product/limit contracts. No keyword/coverage-count or external-standard certification claim. |
| API | Applicable: public library, owned graphs/results, borrowed inputs, shared caches and error enums | [Product contract](../product-contract.md), [0.2 migration](../migration-0.2.md), public docs/doctest, consumer regressions. DTO constructors do not certify caller-created cycles; the detector establishes its own graph invariants. No release-baseline semver-tool claim. |
| INPUT | Applicable: arguments, YAML/JSON/JSONC, stdin, paths, source and graphs | Shared quotas and syntax/config validation; decoding, alias-origin, UTF-8 and boundary regressions; configuration fuzz target. Hostile concurrent filesystem confinement and hard OS deadlines/RSS are unsupported, not claimed controls. |
| CLI | Applicable: machine/human stdout and parallel batch completion | Status 0/1/2, errors before publication, staged report and write/flush tests, TTY/control escaping, worker completion. Stdout has no rename boundary; OS failure can still leave invalid partial bytes. |
| CI | Applicable: published target/MSRV/toolchain and release contract | Locked xtask, exact MSRV, explicit native-target workflow matrix, stable/fresh advisory maintenance, combined status. Hosted execution and enforcement are OPEN; local configuration is not a verified host gate. |
| TOOLS | Applicable to each proposed tool | Deny/coverage/control pilots, owned pinned fuzz harnesses, exact commands and artifacts. No automatic Vet/mutants/Miri/CRAP/semver/profiler requirement follows from catalog availability. |
| FUZZ | Applicable to configuration grammar and native syntax residual risk | [Verification plan/results](verification.md), stable seeds, deterministic bounded harnesses, reproducer/triage ownership, separate locks/nightly/compiler requirements. Findings and unsupported native sanitizer setups must remain visible. |
| TEST | Applicable: residual risk after baseline tests | Coverage triage, independent graph/string oracles, process regressions and defect/repair checks. Coverage reaches test source too; no percentage proves behavior. Current CRAP, mutation and Miri selections follow the owned pilots and explicit project request; see the combined review. |
| PERF | CLI command measurement applies; service instrumentation does not | Semantic-checked synthetic timing command, recorded workload/hash/warm-up and privacy limits. No customer latency/SLA/speedup claim or production profiler/telemetry is selected. |
| DEPS | Supply-chain review applies; cargo-vet audit policy is conditional | Explicit deny policy, lock review and [build trust inventory](build-trust.md). No approved Vet criteria/imports/exemptions exist; do not fabricate them. |
| SAFE | No product-owned unsafe operation/interface; dependency trust remains applicable | Application and xtask forbid unsafe. Native dependencies are not covered by that ban; source/ABI boundary is reviewed in the threat model and exercised through safe provider adapters. |
| FFI | No product-owned extern ABI or raw-pointer wrapper; provider adapter is in scope | Typed static grammar handles, tree/node ownership and native tests/fuzz target. No claim that a passing sanitizer proves provider soundness. |
| BUILD | Applicable after this work added measurable coverage/fuzz/compiler storage | [Storage procedure](maintainer-runbook.md#evidence-and-development-storage), project-local outputs, measured growth and explicit cleanup owner. No global cache relocation or automatic cargo clean. |
| TYPE | Not activated | No columnar/batch storage engine, erased runtime type catalog, typed-null kernel or multi-buffer publication. Ordinary enum/graph data does not activate this architecture. Revisit if that boundary is introduced. |
| ACTOR | Not activated | No Tokio actor/request channel. Synchronous cache mutexes and Rayon do not require adding an actor. Revisit on an asynchronous command/resource protocol. |
| ASYNC | Not activated | No Tokio service/runtime performance boundary. Graph/reader parallelism still has its own worker/error/resource contract. Revisit on runtime adoption. |
| PG | Not activated | No database, migration history or persistent schema. No migration-linter job is meaningful. Revisit before adding persistent database state. |

## Engineering-standard closure

| Guide section | Product assessment |
| --- | --- |
| 1 Toolchain/package | Explicit edition/resolver/MSRV and lint policy, locked checks, package verification, intentional crates.io publication policy and one optional development-profiling feature. Build scripts/macros/native source are inventoried. Cross-platform execution remains host evidence to obtain. |
| 2 API/docs | Public behavior/error/ownership/cache contracts and migration notes; doctest and rustdoc link checks, missing-docs warning stage. Release comparison is a required preparation step, not an invented checker result. |
| 3 Safety/concurrency | Safe-only owned crates; dependency native boundary separated from the ban; native adapter tests/fuzzing. No async cancellation protocol or arbitrary-pointer acceptance. |
| 4 Verification/performance | Real defects reproduced and repaired; generated independent oracles; coverage triage; bounded fuzzing and retained failures; timing claims scoped to their workload. |
| 5 Supply chain | Resolved graph and executable inputs reviewed; 22 grammar packages reduced to 2, unused direct dependencies/derive removed; no overrides/custom sources/trust exemptions. |
| 6 Operations/delivery | Stateless CLI recovery and release preparation/verification procedure. No service deployment/database rollback is invented. Actual release approval/publication/consumer-download verification happens at release time. |
| 7 Platform/runtime | Five supported release triples and exact MSRV, native workflow cells. No no_std/Wasm/TLS runtime contract. Configured jobs are not proof they passed. |
| 8 Provenance/security | Stronger source/workflow/run/lock/digest/binary-smoke identity records and negative tests. No signed provenance/reproducibility claim. Operational private reporting/support policy is OPEN. |
| 9 Domain activation | Every guide domain is assessed below with a concrete selected profile or a trigger for revalidation. |
| 10 Governance/lifecycle | Durable architecture/threat model/product contract/migration/recovery records now exist. Real review/access/handoff and security route/support facts remain OPEN. No organizational SSDF/SAMM conformance is asserted. |

## Product-domain activation

| Domain | Scope decision / contract |
| --- | --- |
| Untrusted inputs | Active: R01–R04/R07/R08/R10; limits, validated ingress and four owned configuration/native-syntax/text/graph fuzz targets. |
| JSON/schema evolution | Active: R05/R09/R10/R11 and migration notes; typed known fields, ecosystem openness, strict project YAML, duplicate handling and existing JSON shape. No canonical JSON/signature claim. |
| HTTP/RPC/messaging | Runtime inactive: no endpoint/client. Release/CI network operations belong to the maintainer/supply-chain boundary. Revisit before adding runtime networking. |
| Persistent data/database | Inactive: no project-owned persistent state. Redirected reports are user-owned files. Revisit before adding durable storage. |
| Authentication/authorization | Runtime inactive. Forge/registry publisher privileges are active operational controls and remain externally unverified. |
| Credentials/signing | Runtime holds none. Job tokens have explicit workflow permissions; no registry/signing/telemetry key is added. Real owner/revocation/handoff facts remain in the runbook's open items. |
| Telemetry | Runtime inactive: no exporters/metrics. Development coverage/fuzz/build reports have local/CI retention and data policy. Revisit before introducing an exporter or external triage. |
| Service deployment | Inactive: release CLI binaries only. Revisit before hosting scans or deploying a daemon/service. |
| Availability/privacy/legal obligations | Local batch resource/failure/privacy contracts are active. No service SLO, safety/financial/regulatory certification or organization policy is supplied by Rust-OE. Named legal requirements would activate a separate profile. |
| CLI | Active: R01–R06, help/version, argument conflicts/defaults, stdout/stderr/status and compatibility. |
| Configuration | Active: R09/R10, one snapshot/no reload, explicit precedence/type/empty/invalid semantics and reproduction inputs. No secret interpolation. |
| Files/process/temp | Active: R08/R13; regular-file/symlink policy, read limits, staged output. Production runs no project process/temp writer. Dev fuzz/release subprocesses are structured, fixed tools with limits and isolated artifacts. |
| Unicode/localization | Active: R06/R08/R11; UTF-8, exact comparison, host canonicalization, no NFKC/case folding, JS StringValue decoding, control escaping, English messages. |
| Containers | Inactive: no selected runtime/container image contract. Hosted runner images remain moving build inputs. Revisit before adding a distributed container. |
| Resources/caches | Active: R07/R12; shared counters, logical-work semantics, cache keys/lifetime/reset, overload failures, native/RSS isolation limits and build-storage procedure. |
| Data lifecycle | Active: R13; local private source, report/path disclosure, no runtime uploads/storage, synthetic dev evidence, user-owned redirected output and explicit CI retention. |
| Time/random/IDs | Runtime inactive: no time-based state, generated public IDs or cryptographic randomness. Development benchmark uses monotonic time; fuzz seeds/artifacts and CI run IDs are scoped development evidence, not stable public IDs. |

## Concrete repairs in this audit

- Reject invalid config/ignore/extension/quota settings at the public text ingress,
  including overrides for packages not selected in the scan.
- Reject recovered/malformed source trees; decode escaped literal module values;
  omit computed expressions; reject NUL/unpaired-surrogate module values.
- Accept JSONC without joining tokens or accepting malformed comments; preserve
  inherited configuration origins, replace child paths correctly, prioritize
  exact aliases, and try replacement fallbacks/directory indexes.
- Match vendor/build path components instead of excluding a checkout whose name
  merely contains node_modules. Sort package/source queues under the work budget.
- Reject ambiguous workspace roots/duplicate package names, invalid CLI suffixes,
  dangling default config links and nonregular source devices.
- Reset resolution-cache hits/misses between extractions; reject non-UTF-8 source
  and package paths under the existing JSON string contract.
- Escape untrusted human report controls, sanitize error rendering, and make the
  verbose human summary count the detailed package cycles it displays.
- Reduce native build trust surface and add explicit grammar-provider ownership,
  threat/resource/privacy/cache contracts, migration and release/incident/handoff procedures.
- Require exact release artifact inventory, identity, hash and smoke evidence;
  reject branch dispatch/version mismatches and mixed/changed/unverified binaries.

`tests/adherence.rs` contains the new consumer regressions. Existing tests that
required incorrect TypeScript alias merging and the affected two snapshots were
changed to the reviewed compiler contract, not weakened to accept both meanings.
The source [inheritance specification](https://www.typescriptlang.org/tsconfig/extends.html)
and [versioned parser](https://github.com/microsoft/TypeScript/blob/v5.9.3/src/compiler/commandLineParser.ts#L3372-L3395)
explain the change. The generated string oracle independently uses JSON encoding
for the shared JSON/JS literal subset.

## Initial local verification

The results below retain the input hashes from this initial audit on macOS
15.7.4 aarch64. The subsequent [closure evidence](evidence/rust-oe-closure-validation.json)
covers the newer working source. Neither establishes execution on other hosts.

- `cargo +1.98.1 xtask ci` passed: both-workspace formatting and dependency
  policy, locked application/workspace checks, Clippy, 412 target tests, one
  doctest and public documentation.
- Exact MSRV `1.92.0` passed the same 412 target tests and one doctest.
- Guidance integrity and all 16 Python guard/release-record tests passed.
  [Control replay](evidence/rust-oe-full-validation.json) passed 20 actual
  passing/defective/repaired cases, with all 46 recorded input hashes rechecked.
- The [configuration](evidence/rust-oe-fuzz-validation.json) and
  [native syntax](evidence/rust-oe-native-fuzz-validation.json) sanitizer smoke campaigns
  passed 354774 and 19548 units respectively in 31 seconds each. Their 26 input
  hashes each match the working source. Earlier failures and scope limits remain
  in [verification](verification.md); these are short pilots.
- [Rust coverage review](evidence/rust-oe-coverage-review.json) records per-file line
  evidence and its limits, with the decision/weak-boundary review in verification.
  It is not a branch, native-code or organizational assurance metric.
- The current release binary passed help/version and known-cycle JSON/status
  smoke checks through the actual release-record builder. Forge identity was
  simulated locally; no release or hosted attestation was created. Locked source
  packaging compiled successfully. Actionlint 1.7.12 checks
  passed; shellcheck was not available to the Actionlint run.
- The [current synthetic benchmark](evidence/rust-oe-full-benchmark.json) checked its
  known cycle on every run: seven warm samples after one warm-up, median 17.9 ms
  for 1000 files. Its binary hash is recorded. This is no customer SLA, comparative
  speedup or timing gate.

Raw local logs, the LLVM report and the explicitly simulated release probe remain
under ignored `target/tooling`. Older validation/benchmark reports retain their
own input scope; they are not relabeled as current results.

## Open obligations that prevent a full-adherence claim

| Gap | What is still required | Owner / evidence needed |
| --- | --- | --- |
| G01 Private security route and support window | Private vulnerability reporting is confirmed disabled by the 2026-11-06 public API snapshot. Establish and verify a real private route, actual receivers, supported security-update versions and communication path. | alm; tested route and actual support decision; see SECURITY.md and the Linux verification record |
| G02 Privileged review and handoff | Verify maintainer account/team mapping, review/tag/publisher permissions, designate an alternate recovery owner and rehearse recovery/access revocation. | alm; real principals/settings and rehearsal record, not a fictitious CODEOWNERS entry |
| G03 Supported-host enforcement | Main is confirmed unprotected with no repository/parent/applicable rules. Execute the proposed workflows on remaining supported hosts/current stable and enforce required statuses/review. | host administrator/alm; actual runs/settings. Local Mac and ARM Linux evidence retains its own scope. |
| C01 Conditional release-boundary evidence | Review version/API baseline, release authorization and actual publication/download verification when preparing the next release. | alm; approved release record. No release is performed by this audit. |

C01 is a release-time obligation, not a requirement to publish this working
change. G01–G03 are unresolved operational facts in the current scope. The former
G04 review work is recorded in the complete rule ledger; new findings or changed
scope still require renewed investigation and evidence. The subsequent local
L01 mutation-verification gap is closed by the complete unchanged-policy Linux
campaign; the unsuccessful Mac records are retained rather than relabeled.

These are not the only possible future risks. A changed scope can activate a
previously inactive domain. The rule inventory records applicability, not a
certification of every upstream implementation or organizational practice.
