# Practice adoption and ownership

## Status and authority

Reviewed 2026-10-20 (UTC). These 17 practices are published Rust-OE guidance, including the 16 drafts that were previously local and untracked. The standards guide selects cross-project recommendations; individual upstream contracts establish language/library behavior. A project-specific gate, tool, architecture, privacy budget, or operational threshold requires an explicit consuming-project decision.

Rust-OE maintainers own guidance changes. Each practice names the adopter's responsible role; the consuming repository records the actual person/team before enforcing it. Publication does not assign an unknown service owner or claim external-standard conformance.

## Adoption record

For each selected rule or rule group, record its ledger ID/version, applicability, actual owner, supported compiler/library/platform, risk, evidence sections, enforcement command or review question, allowed exceptions, validation artifact, and review trigger. Distinguish upstream obligations from stricter local choices. Record a decision to defer with alternative evidence when prerequisites are absent.

Exceptions cannot make undefined behavior safe or change an upstream API contract. They can change a local policy, select another design, narrow supported scope, or supply equivalent verification. State scope, rationale, residual risk, owner, and expiry/review event. Conflicting guidance follows the highest applicable authority and the product's explicit contract; unresolved decisions stay visible.

## Consumers

- Humans use the practice's Decision, Risk, concrete rules, Evidence, Exceptions, and Freshness together.
- Coding agents load only the practices triggered by the task and cite the rule IDs and applicable version in the change record. They document facts and uncertainty before suggesting a control.
- Review bots distinguish detectable syntax/tool results from semantic review questions. Questions about pointer liveness, cancellation, trait laws, database locks, or business correctness cannot become failing findings by keyword matching.
- CI/template owners adopt only a verified command/configuration with supported package/feature/target selection, failure handling, budget, permissions, and rollback. The [control register](https://gitlab.maclaine.io/alm/rust-oe/-/blob/19e4133c0461129b0f5fdb286abd7b87511242e2/docs/control-adoption.md) distinguishes generated-workspace candidates, required template checks, validated optional tool commands, and remaining project prerequisites.

## Practice index

Rule IDs and exact reviewed statements are in the [evidence ledger](https://gitlab.maclaine.io/alm/rust-oe/-/blob/19e4133c0461129b0f5fdb286abd7b87511242e2/docs/guidance-evidence.json). IDs refer to this reviewed revision; source/document hashes make later drift visible. A changed rule requires renewed review, not automatic ledger regeneration.

| ID family | Practice | Activation |
| --- | --- | --- |
| AUTH | [Authority map](https://gitlab.maclaine.io/alm/rust-oe/-/blob/19e4133c0461129b0f5fdb286abd7b87511242e2/practices/rust-standards-authority-map.md) | Proposing or changing guidance |
| API | [API semantics](https://gitlab.maclaine.io/alm/rust-oe/-/blob/19e4133c0461129b0f5fdb286abd7b87511242e2/practices/rust-api-semantic-boundaries.md) | Public/reused Rust interfaces |
| TYPE | [Typed runtime boundaries](https://gitlab.maclaine.io/alm/rust-oe/-/blob/19e4133c0461129b0f5fdb286abd7b87511242e2/practices/typed-runtime-boundaries.md) | Selected batch/columnar engine |
| SAFE | [Unsafe abstractions](https://gitlab.maclaine.io/alm/rust-oe/-/blob/19e4133c0461129b0f5fdb286abd7b87511242e2/practices/unsafe-abstraction-boundaries.md) | Unsafe operations or trusted invariants |
| FFI | [Foreign interfaces](https://gitlab.maclaine.io/alm/rust-oe/-/blob/19e4133c0461129b0f5fdb286abd7b87511242e2/practices/ffi-security-boundaries.md) | Foreign/native/Wasm boundary |
| INPUT | [Untrusted inputs](https://gitlab.maclaine.io/alm/rust-oe/-/blob/19e4133c0461129b0f5fdb286abd7b87511242e2/practices/untrusted-input-boundaries.md) | External-data trust boundary |
| CI | [Cargo CI selection](https://gitlab.maclaine.io/alm/rust-oe/-/blob/19e4133c0461129b0f5fdb286abd7b87511242e2/practices/cargo-ci-control-selection.md) | Declared Cargo support/release contract |
| BUILD | [Build storage](https://gitlab.maclaine.io/alm/rust-oe/-/blob/19e4133c0461129b0f5fdb286abd7b87511242e2/practices/cargo-build-storage.md) | Measured storage/caching cost |
| CLI | [CLI failures/output](https://gitlab.maclaine.io/alm/rust-oe/-/blob/19e4133c0461129b0f5fdb286abd7b87511242e2/practices/cli-failure-reporting.md) | Artifact, pipeline, or batch output |
| DEPS | [Dependency audit policy (cargo-vet / third-party audit coverage)](https://gitlab.maclaine.io/alm/rust-oe/-/blob/19e4133c0461129b0f5fdb286abd7b87511242e2/practices/dependency-audit-policy.md) | Adopting third-party audit criteria (cargo-vet) |
| TOOLS | [Tool adoption](https://gitlab.maclaine.io/alm/rust-oe/-/blob/19e4133c0461129b0f5fdb286abd7b87511242e2/practices/development-tool-adoption.md) | Workflow/template/CI dependency proposal |
| FUZZ | [Fuzzing](https://gitlab.maclaine.io/alm/rust-oe/-/blob/19e4133c0461129b0f5fdb286abd7b87511242e2/practices/fuzzing-boundaries.md) | Large input space with residual risk |
| ASYNC | [Tokio performance](https://gitlab.maclaine.io/alm/rust-oe/-/blob/19e4133c0461129b0f5fdb286abd7b87511242e2/practices/tokio-performance.md) | Measured runtime/workload issue |
| ACTOR | [Task actors](https://gitlab.maclaine.io/alm/rust-oe/-/blob/19e4133c0461129b0f5fdb286abd7b87511242e2/practices/tokio-task-actors.md) | Selected command/resource ownership protocol |
| PERF | [Profiling/observability](https://gitlab.maclaine.io/alm/rust-oe/-/blob/19e4133c0461129b0f5fdb286abd7b87511242e2/practices/performance-observability.md) | Product objective and telemetry policy |
| PG | [PostgreSQL migrations](https://gitlab.maclaine.io/alm/rust-oe/-/blob/19e4133c0461129b0f5fdb286abd7b87511242e2/practices/postgresql-migration-safety.md) | Owned migration history and deployment |
| TEST | [Verification strength](https://gitlab.maclaine.io/alm/rust-oe/-/blob/19e4133c0461129b0f5fdb286abd7b87511242e2/practices/verification-strength.md) | Selected tests and residual-risk tools |

## Freshness

Revalidate on a supported compiler/MSRV/edition, library/tool, platform, architecture, exposure, data-contract, or enforcement change. Read the affected upstream sections and retain new individual source records where needed. A dated benchmark, overview, or successful pilot is evidence for its named scope only.
