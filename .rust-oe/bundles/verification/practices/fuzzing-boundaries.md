# Fuzzing boundaries

## Applicability and authority

- **Scope:** Selected parsers/decoders/transformations with a large input space and material residual risk.
- **Authority:** Rust-OE local engineering guidance; the evidence sections identify upstream contracts and bounded experience.
- **Owner:** accountable: boundary maintainer; consulted: fuzz-triage owner. The consuming project names the responsible person/team before enforcement.
- **Reviewed:** 2026-10-20 (UTC). Version assumptions follow the named sources and the supported project toolchain/platform.
- **Adoption:** [Common ownership, exceptions, consumer, and verification contract](https://gitlab.maclaine.io/alm/rust-oe/-/blob/19e4133c0461129b0f5fdb286abd7b87511242e2/docs/practice-adoption.md).

## Decision

Fuzz Rust components that parse, decode, deserialize, interpret, transform, or otherwise accept a large/untrusted input space when ordinary example/property tests leave material stability or security risk. Start with a narrow deterministic target and explicit invariants; retain a minimized regression input for every accepted crash or semantic defect.

## Risk

Memory safety reduces classes of defects but does not prevent panics, denial of service, resource exhaustion, parser/decoder logic defects, unsafe-code violations, or incorrect acceptance/rejection of malformed input. A long unowned fuzz job produces crashes without timely triage and gives false confidence about paths it never exercises.

## Target contract

1. Choose one boundary and state accepted input, rejected input, resource limits, and invariants: no panic, no hang, bounded memory/work, round-trip equivalence, or a precise error class.
2. Make the harness deterministic and side-effect free. Stub or isolate network, clock, filesystem, credentials, and irreversible operations.
3. Seed the target's corpus with valid representative inputs plus known boundary cases; add seeds where coverage shows unreached paths (`cargo fuzz coverage` replays `fuzz/corpus/<target>` by default). Keeping the corpus and minimized regressions under version control when they are stable and non-sensitive is local policy.
4. Set a local/reproducer command, CI duration budget, and owner for crash triage. cargo-fuzz drives libFuzzer, needs a nightly compiler for its sanitizer flag, and supports only platforms with LLVM sanitizer support; those requirements belong in the documented fuzz lane rather than the default unit-test lane. A CI smoke run bounds time with `-max_total_time` and retains `fuzz/artifacts` crash inputs on failure. The duration budget and triage owner are local policy.
5. On a finding (local policy): minimize the crashing input, classify, fix the real defect or document the intentional behavior, and promote a stable regression to an ordinary test when that is the fastest persistent guard.

## Enforcement

Coding agents propose fuzzing only when they can name the input boundary and invariant. Review bots ask whether a changed parser/protocol/deserializer preserves a relevant fuzz/property test; they do not demand fuzzing for ordinary business logic. CI retains short reproducible smoke fuzzing only when it has demonstrated value; longer campaigns run on owned scheduled/security infrastructure.

## Exceptions

Skip fuzzing where inputs are tightly bounded and conventional tests exhaust the meaningful state space, or where the project cannot yet provide hermetic harnesses and triage ownership. State the alternative evidence. Do not treat fuzz coverage as proof of correctness or a substitute for protocol conformance tests.

## Freshness

Revalidate when the supported compiler/tool/library, platform, exposure, data representation, or adopted contract changes. Re-run relevant evidence and review any exception with its owner; dated examples and measurements are not continuing guarantees.

## Evidence

- [introduction: fuzzing purpose and tool navigation only](https://gitlab.maclaine.io/alm/rust-oe/-/blob/19e4133c0461129b0f5fdb286abd7b87511242e2/sources/rust-fuzz-book.md)
- [cargo-fuzz setup, nightly/sanitizer/platform needs, corpus, coverage, artifacts and CI time bounds](https://gitlab.maclaine.io/alm/rust-oe/-/blob/19e4133c0461129b0f5fdb286abd7b87511242e2/sources/rust-fuzz-book-cargo-fuzz.md)
- [resource/grammar/semantic limits; local harness/budget choices are project policy](https://gitlab.maclaine.io/alm/rust-oe/-/blob/19e4133c0461129b0f5fdb286abd7b87511242e2/sources/owasp-input-validation.md)
