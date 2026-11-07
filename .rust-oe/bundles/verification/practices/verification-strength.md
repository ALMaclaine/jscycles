# Verification strength and complexity risk

## Applicability and authority

- **Scope:** Residual-risk evaluation after a selected test baseline; optional complexity/mutation lanes.
- **Authority:** Rust-OE local engineering guidance; the evidence sections identify upstream contracts and bounded experience.
- **Owner:** accountable: test/CI maintainer; consulted: affected component owner. The consuming project names the responsible person/team before enforcement.
- **Reviewed:** 2026-10-20 (UTC). Version assumptions follow the named sources and the supported project toolchain/platform.
- **Adoption:** [Common ownership, exceptions, consumer, and verification contract](https://gitlab.maclaine.io/alm/rust-oe/-/blob/19e4133c0461129b0f5fdb286abd7b87511242e2/docs/practice-adoption.md).

## Decision

Use ordinary tests as the fast correctness gate. Add coverage/complexity and mutation analysis only where they answer a specific residual-risk question: **which complex code lacks exercised paths**, and **whether configured tests detect plausible code changes**. Treat both outputs as prioritization evidence, not automatic proof of a defect or a required test shape.

## Risk

A green test suite can reach code without asserting its externally observable contract. Conversely, high line coverage can hide untested branches and complex functions; a large mutation run can consume CI capacity while reporting results that do not apply to external or flaky tests.

## Adoption sequence

1. Establish a reliable baseline of the selected tests and supported packages/features/targets. A passing `cargo test --locked` is test-execution evidence, not a line/branch coverage measurement or a deterministic runtime guarantee.
2. For an existing project, generate a coverage baseline and review high CRAP scores as a ranked worklist. Choose a threshold or regression boundary only after inspecting current scores and maintenance cost.
3. When a function is both complex and weakly covered, either split unrelated decisions, add consumer-observable contract tests for the uncovered behavior, or document why the score is acceptable.
4. Pilot mutation analysis on an isolated disposable checkout with an explicit source/test scope and time budget. Verify runner behavior for the chosen release: the retained README and limitations chapter disagree about Nextest; the optional template lane pins cargo-mutants 27.1.0 and explicitly selects Cargo tests. In-place mode modifies source, and CI examples do not fully pin their downloaded binaries. Adopt changed-code or broader lanes only after the pilot; apply a data/retention policy to artifacts.
5. Triage every surviving mutant as one of: a missing observable assertion, equivalent/invalid mutation, intentionally untested boundary, or a design simplification candidate. Do not add implementation-pinned tests merely to kill mutants.

## Enforcement

- **Fast CI:** the project's selected compile, format, lint, unit/integration, doctest, and support checks, each with an owned scope and budget.
- **Optional advisory lane:** coverage/complexity and mutation reports for selected modules after a representative pilot. Publication and retention follow the artifact policy.
- **PR mutation lane:** only after a measured changed-code scope meets the CI budget. Pin tool versions and use the repository's approved installation mechanism.
- **Review question:** Does the changed branch/decision have a consumer-observable test, and does new complexity represent a separable policy?

Never enable external duplicate triage without an approved provider, data classification, credential path, retention decision, and explicit opt-in. It can send function bodies outside the repository.

## Exceptions

Mutation analysis is unsuitable for flaky/non-hermetic tests, tests outside the configured Cargo invocation, generated/macro-dominated code, or a project without CI capacity. Record the excluded path, reason, alternative evidence, owner, and review date. Do not gate on a CRAP threshold until a baseline and a justified failure policy exist.

## Consumers and freshness

Coding agents add observable contract tests—not score-driven scaffolding. Review bots flag newly introduced complex, weakly-tested decision paths as a review question, not a failing finding. CI/template owners pin cargo-crap and cargo-mutants and re-evaluate their behavior on upgrade. Re-run the pilot when the coverage producer, test runner (`cargo test`/nextest), Cargo test selection, or test hermeticity changes.

## Evidence

- [selected LCOV workflow and coverage limits](https://gitlab.maclaine.io/alm/rust-oe/-/blob/19e4133c0461129b0f5fdb286abd7b87511242e2/sources/cargo-llvm-cov.md)
- [test target types and selected invocation](https://gitlab.maclaine.io/alm/rust-oe/-/blob/19e4133c0461129b0f5fdb286abd7b87511242e2/sources/cargo-tests.md)
- [metric floor, LCOV mapping, thresholds, exit failures and external triage](https://gitlab.maclaine.io/alm/rust-oe/-/blob/19e4133c0461129b0f5fdb286abd7b87511242e2/sources/minikin-cargo-crap.md)
- [invoked-test scope and runner disagreement](https://gitlab.maclaine.io/alm/rust-oe/-/blob/19e4133c0461129b0f5fdb286abd7b87511242e2/sources/sourcefrog-cargo-mutants.md)
- [in-place mutation, artifacts and installer pinning](https://gitlab.maclaine.io/alm/rust-oe/-/blob/19e4133c0461129b0f5fdb286abd7b87511242e2/sources/mutants-ci.md)
- [hermetic/cfg/generated-code/runner and format limits](https://gitlab.maclaine.io/alm/rust-oe/-/blob/19e4133c0461129b0f5fdb286abd7b87511242e2/sources/mutants-limitations.md)
