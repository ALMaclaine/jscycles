# Rust engineering standards guide

## Purpose and authority

This guide defines a cross-project engineering baseline. Its recommendations are local guidance informed by the named upstream contracts; it is not a Rust specification, certification, or proof of product completeness. A consuming project selects its support contract and activates applicable product-domain requirements in the [coverage matrix](https://gitlab.maclaine.io/alm/rust-oe/-/blob/19e4133c0461129b0f5fdb286abd7b87511242e2/docs/rust-standards-coverage.md).

The [practice adoption contract](practice-adoption.md) names ownership, exceptions, consumers, and revalidation. The [guidance review](https://gitlab.maclaine.io/alm/rust-oe/-/blob/19e4133c0461129b0f5fdb286abd7b87511242e2/docs/guidance-review.md) and [rule evidence ledger](https://gitlab.maclaine.io/alm/rust-oe/-/blob/19e4133c0461129b0f5fdb286abd7b87511242e2/docs/guidance-evidence.json) record the reviewed claims, supporting sections, and dispositions. Proposed automation has its own [validation and adoption status](https://gitlab.maclaine.io/alm/rust-oe/-/blob/19e4133c0461129b0f5fdb286abd7b87511242e2/docs/control-adoption.md).

Authority precedence follows the [authority map](../practices/rust-standards-authority-map.md#authority-ladder): upstream behavior > Rust-project convention > tool contract > adopted domain/organization policy (including product/security/safety requirements) > community/research.

A local policy may be stricter than an upstream convention only when its scope, owner, rationale, escape hatch, and revalidation trigger are recorded. Sources: [authority map](../practices/rust-standards-authority-map.md), [coverage matrix](https://gitlab.maclaine.io/alm/rust-oe/-/blob/19e4133c0461129b0f5fdb286abd7b87511242e2/docs/rust-standards-coverage.md), and [source catalog](https://gitlab.maclaine.io/alm/rust-oe/-/blob/19e4133c0461129b0f5fdb286abd7b87511242e2/sources/rust-standards-catalog-2026-10-19.md).

## 1. Toolchain and package contract

- Declare `edition` explicitly. Declare `rust-version` for a reusable or published package and test that exact MSRV plus current stable (a check-only MSRV lane is a documented cost compromise); a private deployed application may instead pin its toolchain in its deployment contract.
- Commit the lockfile by default (`cargo new` tracks it; Cargo leaves the final choice to the package's needs) and use it for repeatable CI inputs. `--locked` rejects required resolution changes; it does not freeze the toolchain, environment, fetched code, native inputs, or runtime behavior. Library consumers resolve the manifest, so lockfile-only tests do not establish dependency-range compatibility.
- Set `resolver` explicitly in a virtual workspace; it has no package edition to infer it from. Edition 2024 defaults packages to resolver "3", which prefers dependency versions compatible with `rust-version`. Record the chosen resolver and test resolution changes.
- Check formatting with `cargo fmt --all --check` under a pinned rustfmt version and its configuration. Declare lint levels in `[lints]`/`[workspace.lints]` (Cargo 1.74+); suppression and group policy follow the [CI practice's lint exceptions](https://gitlab.maclaine.io/alm/rust-oe/-/blob/19e4133c0461129b0f5fdb286abd7b87511242e2/practices/cargo-ci-control-selection.md#lint-exceptions): narrow `#[expect]` where the MSRV supports lint expectations, enable pedantic selectively or with documented allowances, never whole restriction or nursery groups.
- Every published package supplies intentional identity metadata: version, description, license/license file, repository, and publishing policy. Run `cargo package` or `cargo publish --dry-run` before publishing.
- Use dependency lower bounds that reflect required APIs and verify a compatible selection for the supported MSRV. Exhaustive/minimal-version resolution is an optional supplementary experiment; this guide adopts no minimal-versions command.
- Make features additive and semver-aware. Test advertised default, no-default, and relevant supported feature combinations. Removing a default feature can be semver-incompatible.
- Treat `build.rs`, procedural macros, and build dependencies as executable build-time trust boundaries. Build scripts declare `rerun-if-*` inputs, use `CARGO_CFG_*` for target information in cross-compiles, and write generated material only in `OUT_DIR`.

**Evidence:** [manifest](https://doc.rust-lang.org/cargo/reference/manifest.html), [`[lints]`](https://doc.rust-lang.org/cargo/reference/manifest.html#the-lints-section), [lockfile FAQ](https://doc.rust-lang.org/cargo/faq.html#why-have-cargolock-in-version-control), [2024 resolver](https://doc.rust-lang.org/edition-guide/rust-2024/cargo-resolver.html), [Style Guide](https://doc.rust-lang.org/style-guide/), [Clippy lint groups](https://doc.rust-lang.org/clippy/lints.html), [Rust version](https://doc.rust-lang.org/cargo/reference/rust-version.html), [features](https://doc.rust-lang.org/cargo/reference/features.html), [build scripts](https://doc.rust-lang.org/cargo/reference/build-scripts.html), [publishing](https://doc.rust-lang.org/cargo/reference/publishing.html).

## 2. Public API and documentation

- Specify ownership, borrowing, mutation, concurrency, error, panic, cancellation, and compatibility behavior at every public boundary. Model invalid states out of APIs where practical; do not convert fallible or safety-relevant behavior into hidden defaults. See the [API semantics practice](https://gitlab.maclaine.io/alm/rust-oe/-/blob/19e4133c0461129b0f5fdb286abd7b87511242e2/practices/rust-api-semantic-boundaries.md).
- Follow Rust API Guidelines as review criteria, not mechanical law. Public API changes need a compatibility classification and migration story. A semver checker supplements review only after the [CI practice](https://gitlab.maclaine.io/alm/rust-oe/-/blob/19e4133c0461129b0f5fdb286abd7b87511242e2/practices/cargo-ci-control-selection.md) pilot.
- Public libraries provide crate-level docs, consumer-facing examples, and `# Safety`, `# Errors`, or `# Panics` sections where applicable. Use doctests for executable examples.
- Keep documentation links resolvable and avoid exposing implementation details as API. Stage `missing_docs` from warnings to denial only when the intentional public surface is documented.

**Evidence:** [API Guidelines](https://rust-lang.github.io/api-guidelines/), [API documentation guidance](https://rust-lang.github.io/api-guidelines/documentation.html), [rustdoc content](https://doc.rust-lang.org/rustdoc/write-documentation/what-to-include.html), [rustdoc tests](https://doc.rust-lang.org/rustdoc/documentation-tests.html).

## 3. Safety, FFI, and concurrency

- Safe clients must not trigger UB through a sound safe abstraction. Document the actual caller/implementor obligations of unsafe interfaces and justify unsafe operations from established facts. A local `SAFETY:` explanation is a review convention, not proof by itself; the [unsafe-boundary practice](https://gitlab.maclaine.io/alm/rust-oe/-/blob/19e4133c0461129b0f5fdb286abd7b87511242e2/practices/unsafe-abstraction-boundaries.md) includes safe code that can alter trusted invariants.
- Treat Miri as a targeted dynamic check for exercised undefined-behavior classes, not a proof of soundness.
- FFI boundaries specify ABI, representation, ownership/deallocation, nullability, lengths, callback lifetime/threading, errors, and unwind behavior. Use target-compatible representations, including `repr(C)` where appropriate; it does not validate values or recursively set nested layout. Pointer-validity limits and safe-wrapper conditions follow the [FFI practice](https://gitlab.maclaine.io/alm/rust-oe/-/blob/19e4133c0461129b0f5fdb286abd7b87511242e2/practices/ffi-security-boundaries.md#boundary-contract).
- Async ownership, cancellation, backpressure, retries, and shutdown are API behavior. Test time-dependent behavior with controlled time and I/O seams; passing unit tests is not a proof against races or deadlocks. See [task actors](https://gitlab.maclaine.io/alm/rust-oe/-/blob/19e4133c0461129b0f5fdb286abd7b87511242e2/practices/tokio-task-actors.md) and [Tokio performance](https://gitlab.maclaine.io/alm/rust-oe/-/blob/19e4133c0461129b0f5fdb286abd7b87511242e2/practices/tokio-performance.md).

**Evidence:** [Rust Reference unsafety](https://doc.rust-lang.org/reference/unsafety.html), [recognized UB](https://doc.rust-lang.org/reference/behavior-considered-undefined.html), [Nomicon FFI](https://doc.rust-lang.org/nomicon/ffi.html), [Miri](https://github.com/rust-lang/miri), [Tokio testing](https://tokio.rs/tokio/topics/testing).

## 4. Verification and performance

Choose the cheapest check that can expose the relevant plausible failure:

| Risk | Required evidence | Typical control |
| --- | --- | --- |
| Type/borrow/error-path defect | compile and focused behavior test | `cargo test` for the affected target |
| Public example or documentation regression | compiling/running example | `cargo test --doc`; `cargo doc --no-deps` with the project's selected rustdoc lint levels (`missing_docs` per its stage) on the pinned toolchain |
| Parser/protocol/untrusted-input defect | bounded edge cases and protocol tests; generated inputs where residual risk warrants | property/fuzz target with budget and triage owner when selected ([FUZZ](https://gitlab.maclaine.io/alm/rust-oe/-/blob/19e4133c0461129b0f5fdb286abd7b87511242e2/practices/fuzzing-boundaries.md)) |
| Public API compatibility regression | release-boundary API comparison | semver checker after the [CI practice](https://gitlab.maclaine.io/alm/rust-oe/-/blob/19e4133c0461129b0f5fdb286abd7b87511242e2/practices/cargo-ci-control-selection.md) pilot |
| Dependency advisory/license/source policy | locked graph evaluated against explicit policy | `cargo audit`/`cargo deny check` per adopted policy; cargo-vet only after the [DEPS](https://gitlab.maclaine.io/alm/rust-oe/-/blob/19e4133c0461129b0f5fdb286abd7b87511242e2/practices/dependency-audit-policy.md) project pilot ([register](https://gitlab.maclaine.io/alm/rust-oe/-/blob/19e4133c0461129b0f5fdb286abd7b87511242e2/docs/control-adoption.md#priority-2-remaining-project-prerequisites)) |
| Performance regression | representative benchmark/profile with baseline | project benchmark harness plus profiler evidence ([PERF](https://gitlab.maclaine.io/alm/rust-oe/-/blob/19e4133c0461129b0f5fdb286abd7b87511242e2/practices/performance-observability.md)) |

Do not add a tool merely because it exists. Mutation, fuzzing, Miri, and benchmark lanes require named boundaries, ownership, runtime budgets, and a response path ([TEST](https://gitlab.maclaine.io/alm/rust-oe/-/blob/19e4133c0461129b0f5fdb286abd7b87511242e2/practices/verification-strength.md)).

**Evidence:** [Cargo tests](https://doc.rust-lang.org/cargo/guide/tests.html), [Clippy lints](https://doc.rust-lang.org/clippy/lints.html), [Rust Fuzz Book](https://rust-fuzz.github.io/book/introduction.html), [Rust Performance Book](https://nnethercote.github.io/perf-book/).

## 5. Dependency and build supply chain

- Review the resolved dependency graph, not only direct `Cargo.toml` entries. Update intentionally; preserve the committed lockfile evidence.
- `cargo-audit` detects published RustSec advisories; it does not establish absence of compromise. `cargo-deny` enforces declared advisory/license/source/ban policy. `cargo-vet` matches resolved dependencies to recorded audits against project-chosen criteria. `cargo-auditable` inventories dependencies embedded in a released binary. State each tool’s limit exactly; audit-criteria adoption follows the [dependency audit practice](https://gitlab.maclaine.io/alm/rust-oe/-/blob/19e4133c0461129b0f5fdb286abd7b87511242e2/practices/dependency-audit-policy.md).
- Build scripts, proc macros, vendored native code, registries, and CI actions are part of the trusted build boundary. Limit/record exceptions and revalidate them when dependencies or toolchains change.
- Cargo source replacement assumes identical code and no extra packages absent from the original source; it is not a mechanism for modifying a dependency. Vendored checksums guard accidental modification, not malicious change. Record overrides and confirm the resolved graph; remove a temporary override when upstream resolves the need, or document intentional continued use and review ownership.

**Evidence:** [RustSec advisory database](https://github.com/RustSec/advisory-db), [cargo-audit](https://github.com/rustsec/rustsec/tree/main/cargo-audit), [cargo-deny](https://github.com/EmbarkStudios/cargo-deny), [cargo-vet](https://github.com/mozilla/cargo-vet), [cargo-auditable](https://github.com/rust-secure-code/cargo-auditable).

## 6. Operations and delivery

- Services define externally observable success/failure, timeouts, retry policy, overload/backpressure behavior, shutdown, telemetry fields, and recovery ownership. These are product contracts, not Rust defaults.
- Select OpenTelemetry only where the service telemetry contract adopts it; define cardinality, privacy, sampling, retention, alerting, and incident response separately.
- A selected release contract records source revision, toolchain, resolution, build inputs, verification, and deployment identity. Define an executable rollback or forward-recovery procedure that accounts for state/schema compatibility and preserves independent recovery access. Reverting a Deployment template alone does not restore data.

**Evidence:** [OpenTelemetry Rust](https://opentelemetry.io/docs/languages/rust/), [Cargo CI guide](https://doc.rust-lang.org/cargo/guide/continuous-integration.html).

## 7. Platform and constrained-runtime contracts

- Treat a target triple as part of the product contract. Rust target tiers have materially different guarantees: tier 1 is built and tested by the Rust project, tier 2 is built but may not be tested, and tier 3 has no automated build/test guarantee. CI covers every supported target or records why a hardware/runtime validation lane is required.
- `no_std` is an explicit runtime boundary, not a size optimization toggle. Name the selected target, allocator/`alloc` use, panic handler, startup/runtime owner, I/O model, and memory/interrupt assumptions. `core` does not supply operating-system integration.
- Select a WebAssembly target based on the intended host, not merely `wasm`. For `wasm32-unknown-unknown`, filesystem APIs fail, thread spawning panics, runtime APIs may be unavailable, and final artifacts must be validated against the selected engine’s supported feature set.
- Choose cryptographic/TLS providers for the actual platform and compliance requirement. Never disable certificate verification to make an integration work. Treat provider features, native build requirements, root trust, key/certificate rotation, and plaintext-secret exposure as separate review concerns.

**Evidence:** [Rust target tiers](https://doc.rust-lang.org/rustc/platform-support.html), [`wasm32-unknown-unknown`](https://doc.rust-lang.org/rustc/platform-support/wasm32-unknown-unknown.html), [Embedded Rust `no_std`](https://docs.rust-embedded.org/book/intro/no-std.html), [`rustls`](https://docs.rs/rustls/latest/rustls/).

## 8. Artifact provenance and security response

- A release record identifies source revision, build definition, builder identity, resolved dependencies, artifact digest, verification results, publication event, and deployment authorization. This is stronger than a Git tag and separate from Cargo registry publication.
- If provenance is in scope, select the actual format/track/version and verifier policy, then attest and verify the exact consumer artifact. The retained SLSA overview provides vocabulary, not a complete attestation implementation or level assessment. Artifact identity, trusted builder, authorization, and source safety remain separate decisions.
- Reproducible builds are a release property requiring controlled inputs, not only deterministic timestamps. `SOURCE_DATE_EPOCH` is one standardized input for tools that support it; verify reproducibility by independently rebuilding/comparing the relevant artifact.
- Every maintained Rust product has a private vulnerability reporting route, scope, supported-version policy, triage owner, embargo/disclosure process, patch/release procedure, and security-update communication channel. Do not copy Rust Project contacts or timelines into another project.

**Evidence:** [SLSA provenance](https://slsa.dev/spec/v1.2/provenance), [SOURCE_DATE_EPOCH](https://reproducible-builds.org/docs/source-date-epoch/), [Rust security policy](https://www.rust-lang.org/policies/security).

## 9. Product-domain contract activation

Rust does not define data governance, service behavior, authentication, database, or deployment policy. Treating an incidental crate choice as policy creates unreviewed behavior. Before implementation, select the applicable rows below, record their authority and owner, and add reviewable requirements to the product contract.

| If the product… | Required domain contract | Closest authority |
| --- | --- | --- |
| accepts untrusted text, bytes, files, or structured values | maximum request/message sizes, nesting/count/string/number limits, grammar/schema, Unicode/canonicalization, invalid-input response, and fuzzing target | [INPUT](https://gitlab.maclaine.io/alm/rust-oe/-/blob/19e4133c0461129b0f5fdb286abd7b87511242e2/practices/untrusted-input-boundaries.md); protocol specification; parser/format documentation; [OWASP input validation](https://cheatsheetseries.owasp.org/cheatsheets/Input_Validation_Cheat_Sheet.html) |
| exchanges JSON or evolves serialized records | selected format, duplicate/unknown-field behavior, numeric and Unicode interoperability, compatibility/versioning, canonicalization, and migration/rollback plan | [RFC 8259](https://www.rfc-editor.org/rfc/rfc8259) plus the selected schema/format specification |
| exposes HTTP, RPC, or messaging endpoints | protocol version, request/deadline/cancellation limits, idempotency, retry ownership/budget, overload behavior, response/error mapping, and compatibility policy | [HTTP Semantics (RFC 9110)](https://www.rfc-editor.org/rfc/rfc9110) or the selected protocol specification |
| stores persistent data | supported database/version, query and transaction boundary, migration ordering/locks/timeouts, backward-compatible rollout, backup/restore objective, and rollback/recovery procedure | database vendor documentation; [PostgreSQL migration practice](https://gitlab.maclaine.io/alm/rust-oe/-/blob/19e4133c0461129b0f5fdb286abd7b87511242e2/practices/postgresql-migration-safety.md) if applicable |
| authenticates users, services, or workloads | chosen identity protocol, issuer/trust/key-discovery rules, token/session lifetime and validation, authorization model, revocation, audit evidence, and negative authorization tests | selected protocol, e.g. [OpenID Connect Core](https://openid.net/specs/openid-connect-core-1_0.html); [OWASP authorization](https://cheatsheetseries.owasp.org/cheatsheets/Authorization_Cheat_Sheet.html) |
| holds credentials, signing keys, or certificates | secret inventory/owner, least-privilege delivery path, no-log/no-source controls, rotation/revocation, expiration, audit, break-glass, and restore test | [OWASP secrets management](https://cheatsheetseries.owasp.org/cheatsheets/Secrets_Management_Cheat_Sheet.html) plus the selected secret-management system |
| emits telemetry | signal schemas, service/resource identity, cardinality/value limits, PII/redaction policy, sampling, export failure behavior, retention, alerts, and response owner | [OpenTelemetry semantic conventions](https://opentelemetry.io/docs/specs/semconv/) and SDK configuration specification |
| deploys a service | declared management plane, immutable artifact/config identity, rollout health gates, rollback, drift detection, access/recovery ownership, and post-deploy verification | deployment platform documentation; e.g. [Kubernetes Deployments](https://kubernetes.io/docs/concepts/workloads/controllers/deployment/) |
| has availability, privacy, safety, financial, or legal obligations | named SLO/error budget, capacity and degradation model, recovery objective and restore exercise, data classification/retention, and controlling regulation | owner-selected organizational and regulatory authority |
| is a CLI, daemon, desktop tool, or automation endpoint | argument grammar/defaults, help/version, stdout/stderr contract, stable machine-output format, exit statuses, TTY behavior, and compatibility guarantees | [Rust CLI Book](https://rust-cli.github.io/book/) and [CLI failure-reporting practice](https://gitlab.maclaine.io/alm/rust-oe/-/blob/19e4133c0461129b0f5fdb286abd7b87511242e2/practices/cli-failure-reporting.md) for failure/output integrity only |
| receives configuration through files, arguments, or environment | source precedence, type parsing, absent/empty/invalid behavior, secret-redaction, reload behavior, and a configuration snapshot/audit path | [`std::env`](https://doc.rust-lang.org/std/env/); selected configuration/runtime documentation |
| reads or writes files, temp space, or starts child processes | path/symlink/traversal policy, ownership/permissions, atomicity/durability, size limits, command allowlist/environment, child lifetime and output handling | [`std::fs`](https://doc.rust-lang.org/std/fs/) and [`std::process`](https://doc.rust-lang.org/std/process/) |
| handles human text or identifiers | encoding, normalization/comparison/search policy, locale/time-zone behavior, and localization behavior; compatibility normalization is never automatic | [Unicode UAX #15](https://www.unicode.org/reports/tr15/) |
| ships containers | base-image identity and refresh policy, build/runtime separation, non-secret build context, runtime privileges/filesystem/network, artifact digest/provenance, and vulnerability updates | [Docker build best practices](https://docs.docker.com/build/building/best-practices/) |
| allocates finite resources or caches data | CPU/memory/storage/FD/connection/queue/body budgets, cache key/scope/invalidation/eviction, overload behavior, and exhaustion verification | declared platform; e.g. [Kubernetes resource management](https://kubernetes.io/docs/concepts/configuration/manage-resources-containers/) |
| stores, logs, exports, or deletes potentially sensitive data | data classification and purpose, minimization/access/audit, retention/deletion/export, backup/restore, logging exclusions, and legal/compliance owner | applicable law and product data policy; [OWASP logging](https://cheatsheetseries.owasp.org/cheatsheets/Logging_Cheat_Sheet.html) and [privacy protection](https://cheatsheetseries.owasp.org/cheatsheets/User_Privacy_Protection_Cheat_Sheet.html) |
| depends on time, random values, or generated identifiers | wall-clock versus monotonic-clock usage, time-zone and clock-skew behavior, deterministic test clock, randomness source/security class, uniqueness scope, and serialized/persisted representation | [`std::time`](https://doc.rust-lang.org/std/time/); selected protocol/cryptographic specification; [Rand Book](https://rust-random.github.io/book/) |

The matrix marks these rows `profile required`: research establishes their closest authority and mandatory decision set, while the owning product must select and verify the actual contract. A generic Rust guide cannot close them honestly.

## 10. Engineering governance and lifecycle

- Material architecture changes record the system boundary, trust/data flows, persistent invariants, alternatives, decision owner, and revalidation trigger. A pull request description is not a durable substitute when later changes depend on the decision.
- Threat modeling is mandatory when a change introduces a trust boundary, untrusted input, identity/authorization path, secret, protocol, privileged operation, persistent sensitive data, or material availability risk. Maintain the model alongside the system; each identified threat has a mitigation, transfer, or explicitly accepted residual risk with an owner and testable control.
- Maintained repositories name owners for release, security response, and high-risk paths. Review enforcement must match the declared host: on GitHub, CODEOWNERS only requests review unless branch/ruleset policy makes it required. Define maintainer handoff and privileged-access revocation before the sole maintainer is unavailable.
- Public libraries declare their versioning contract and public API. Classify additions, deprecations, behavioral changes, and removals before release; publish relevant release/migration notes. Support windows and EOL communication are local lifecycle choices to record when applicable, not requirements imposed by SemVer itself.
- Registry publication is generally permanent. `cargo yank` prevents future new resolutions but does not delete the source, repair existing lockfiles, or revoke exposed secrets. Release preparation includes verification, artifact identity, owner authorization, rollback plan, and post-publication monitoring.
- Incidents close only after the owner records timeline/evidence, corrective actions, verification, and the resulting updates to controls, tests, runbooks, and threat models. Blamelessness does not excuse leaving a systemic cause unowned.
- Every product names its functional and quality requirements, acceptance evidence, change authority, and traceability to verification. Rust-level checks cannot decide acceptable latency, availability, usability, privacy, cost, or legal risk; record the selected product threshold and revalidate it after a relevant change.
- The organization sets contributor-access, development/release-environment, security-education, and measurement policy. The retained SSDF/SAMM overview material helps identify questions; it does not establish conformance to their detailed requirements or maturity levels. Use the full selected standard/model when such an assessment is required.

**Evidence:** [NIST SSDF 1.1](https://csrc.nist.gov/pubs/sp/800/218/final), [OWASP SAMM](https://owasp.org/projects/samm), [OWASP threat modeling](https://cheatsheetseries.owasp.org/cheatsheets/Threat_Modeling_Cheat_Sheet.html), [Rust governance](https://www.rust-lang.org/governance/), [Rust RFC process](https://github.com/rust-lang/rfcs), [SemVer 2.0.0](https://semver.org/), [Cargo publishing](https://doc.rust-lang.org/cargo/reference/publishing.html), [GitHub CODEOWNERS](https://docs.github.com/en/repositories/managing-your-repositorys-settings-and-features/customizing-your-repository/about-code-owners).

## Review questions

For each material change, answer:

1. Which upstream/contract authority applies?
2. What observable behavior and failure mode change?
3. Which targeted verification would fail if the implementation were wrong?
4. Does it alter public API, MSRV, edition, feature set, build-time execution, unsafe/FFI boundary, dependency trust, or operational recovery?
5. If a local rule is stricter, where are its owner, exception, and freshness trigger?

## Freshness

Revalidate toolchain-sensitive sections on Rust/edition/MSRV upgrades; dependency and tool sections on policy/tool version changes; domain/operations sections on architecture or deployment-contract changes. Maintain the evidence catalog rather than preserving unverified summaries.
