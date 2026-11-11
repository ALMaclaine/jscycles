# CLI failure reporting and output integrity

## Applicability and authority

- **Scope:** CLIs publishing artifacts, pipeline data, or batch-job results; output contract selected per product.
- **Authority:** Rust-OE local engineering guidance; the evidence sections identify upstream contracts and bounded experience.
- **Owner:** accountable: CLI/API maintainer; consulted: artifact/recovery owner. The consuming project names the responsible person/team before enforcement.
- **Reviewed:** 2026-10-20 (UTC). Version assumptions follow the named sources and the supported project toolchain/platform.
- **Adoption:** [Common ownership, exceptions, consumer, and verification contract](https://gitlab.maclaine.io/alm/rust-oe/-/blob/19e4133c0461129b0f5fdb286abd7b87511242e2/docs/practice-adoption.md).

## Decision

A Rust CLI that produces files, pipeline data, or long-running job output must make failure diagnosable and incomplete output unmistakable. Keep expected operational failures in a contextual `Result` chain; report unexpected bugs and uncaught panics separately; test the user-visible failure contract where failure handling is non-trivial.

## Risk

Failure can leave plausible-looking incomplete output, lose cause information, or expose sensitive diagnostics.

## Failure contract

1. **Context:** attach causal context at boundaries that identify the failed input, operation, or output. Preserve the source error rather than replacing it with a generic message.
2. **Classification:** distinguish invalid input, operational failure, cancellation, and program bug in the final report and exit status. Do not present a program invariant violation as user remediation.
3. **Panic path:** define whether unattended/batch use needs a custom report or existing supervisor/crash reporting suffices. Include only approved program/version/location/support details; redact sensitive diagnostics. Panic-report behavior depends on the unwinding/termination configuration; do not assume a panic hook recovers or reports every abort, signal, or process failure.
4. **Output integrity:** publish a new single artifact through a clearly incomplete temporary/partial name when that fits the product contract. Observe write, flush, encoder-finish, and rename failures. Same-filesystem rename is a naming step; crash durability, overwrite policy, multi-file consistency, and races need separate guarantees. Preserve a previously complete destination on failure.
5. **Pipelines:** define broken-pipe, cancellation, and signal behavior. A quiet successful exit for an expected downstream early close can be legitimate; distinguish it from unexpected data loss and document the selected contract.
6. **Concurrent workers:** coordinate failure with the output owner. On worker failure, communicate the failure, allow any required writer finalization, preserve the incomplete marker, and return the designated failure result.

## Verification

Test only the observable contracts that can regress:

- contextual error chain and final classification for a representative expected failure;
- incomplete output remains at its partial name after a writer/worker failure;
- successful output is finalized and has no partial marker;
- where custom panic reporting is selected, a controlled failpoint produces the designated report without sensitive data; test abort/signal paths through their actual supervisor policy separately.

Failpoints must be restricted to test/debug configuration and fixed injection points. They are not a production control plane.

## Enforcement and consumers

Coding agents name the output contract (partial naming, broken-pipe policy, exit classification) before changing output paths; review bots flag direct writes to final artifact names (where the selected contract uses partial naming) and unchecked flush/finish/rename results as review questions, not failing findings.

## Exceptions

A streaming CLI whose only output is stdout has no rename boundary. It still needs an explicit broken-pipe/cancellation policy. A tool that intentionally supports resumable partial artifacts must publish a format-level validity/resume contract instead of relying on a filename convention.

## Freshness

Revalidate when the supported compiler/tool/library, platform, exposure, data representation, or adopted contract changes. Re-run relevant evidence and review any exception with its owner; dated examples and measurements are not continuing guarantees.

## Evidence

- [Result propagation, context and cause preservation](https://gitlab.maclaine.io/alm/rust-oe/-/blob/19e4133c0461129b0f5fdb286abd7b87511242e2/sources/cli-book-errors.md)
- [stdout/stderr, buffering, diagnostics and omitted write handling](https://gitlab.maclaine.io/alm/rust-oe/-/blob/19e4133c0461129b0f5fdb286abd7b87511242e2/sources/cli-book-output.md)
- [workload-specific failpoints, partial names and coordinated writer finalization](https://gitlab.maclaine.io/alm/rust-oe/-/blob/19e4133c0461129b0f5fdb286abd7b87511242e2/sources/deterministic-cli-crashes.md)
- [partial/error handling, buffering and descriptor ownership](https://gitlab.maclaine.io/alm/rust-oe/-/blob/19e4133c0461129b0f5fdb286abd7b87511242e2/sources/std-io.md)
- [mount/platform errors and limits of naming publication](https://gitlab.maclaine.io/alm/rust-oe/-/blob/19e4133c0461129b0f5fdb286abd7b87511242e2/sources/std-fs-rename.md)
- [normal process outcomes and platform limitations](https://gitlab.maclaine.io/alm/rust-oe/-/blob/19e4133c0461129b0f5fdb286abd7b87511242e2/sources/std-process-exit-code.md)
