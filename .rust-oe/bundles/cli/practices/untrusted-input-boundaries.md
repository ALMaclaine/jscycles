# Untrusted input boundaries

## Applicability and authority

- **Scope:** External bytes/text/paths/configuration/process data entering a named trust boundary.
- **Authority:** Rust-OE local engineering guidance; the evidence sections identify upstream contracts and bounded experience.
- **Owner:** accountable: boundary/API maintainer; consulted: product security owner for exposure and budgets. The consuming project names the responsible person/team before enforcement.
- **Reviewed:** 2026-10-20 (UTC). Version assumptions follow the named sources and the supported project toolchain/platform.
- **Adoption:** [Common ownership, exceptions, consumer, and verification contract](https://gitlab.maclaine.io/alm/rust-oe/-/blob/19e4133c0461129b0f5fdb286abd7b87511242e2/docs/practice-adoption.md).

## Decision

Treat bytes, strings, paths, environment values, request fields, file contents, and child-process output from outside the trust boundary as untrusted until one ingress operation parses them into a domain type with explicit semantic and resource limits. Memory safety does not bound allocation, CPU, recursion, decompression, authorization, or side effects.

## Risk

Well-typed or UTF-8-valid input can still cause resource exhaustion, injection, traversal, or unauthorized behavior.

## Boundary contract

1. **Limit before materialization:** cap bytes read, decompressed output, item count, nesting/depth, and work before allocating or traversing attacker-controlled structures. A parser recursion limit is not a substitute for a byte or collection budget.
2. **Establish domain invariants:** use a fallible parser/constructor and protected representation where an invariant needs it. Apply the selected grammar, range, length, normalization/comparison policy, and application semantics. Do not force canonicalization or a wrapper onto every string. Validate again when a new trust boundary or changed state invalidates the earlier guarantee.
3. **Arithmetic:** use checked operations for sizes, offsets, and counts whose overflow invalidates a safety/resource decision. Use saturating or wrapping operations only when that behavior is part of the published semantics. Use `TryFrom` for untrusted narrowing. Do not rely on `overflow-checks` panics: release profiles disable them by default.
4. **External interpreters:** use parameterized/database/structured APIs instead of composing syntax. For `Command`, pass an absolute or approved program and each argument separately; avoid a shell, validate child-specific syntax, and use `--` when the child supports it. Do not pass untrusted input to `cmd.exe` or batch files.
5. **Paths:** model accepted relative path components, reject root/prefix/parent traversal, and do not claim that lexical filtering alone confines filesystem access. If an attacker can race or alter the filesystem, enforce containment through a descriptor/capability-based or isolated filesystem boundary.
6. **Schemas:** reject unknown fields only for a deliberately closed wire contract. Bound string/collection fields independently. Authorization is a separate operation; successful deserialization never establishes authority.
7. **Failure:** return a typed/reviewable boundary error without leaking secrets or presenting a partial output as successful.
8. **Foreign buffers:** bound encoded bytes rather than character/code-unit counts. Pointer validity, provenance, and ownership follow the [FFI boundary contract](https://gitlab.maclaine.io/alm/rust-oe/-/blob/19e4133c0461129b0f5fdb286abd7b87511242e2/practices/ffi-security-boundaries.md#boundary-contract); the ingress contract also defines copying, reentrancy, and output-buffer reuse.

## Verification

- Boundary tests cover the largest accepted and first rejected byte/count/depth/range values, arithmetic overflow, malformed syntax, and representative traversal/option-injection inputs.
- Property/fuzz targets follow the [FUZZ target contract](https://gitlab.maclaine.io/alm/rust-oe/-/blob/19e4133c0461129b0f5fdb286abd7b87511242e2/practices/fuzzing-boundaries.md#target-contract), adding no escape from the modeled boundary.
- Integration tests exercise the actual database/process/filesystem adapter where correctness depends on its grammar or operating-system behavior.

## Enforcement

Coding agents must name the untrusted source, parser/constructor, resource budget, and failure result before adding an external-input path. Review bots flag capacity allocation, multiplication/addition, shell execution, raw path joining, recursive decode, or deserialization of externally supplied data that lacks a nearby bound/validation explanation. These remain review questions; only a separately verified detectable condition (for example a selected Clippy lint) may become a gate under the project's ingress contract.

## Exceptions

Do not require wrappers/canonicalization for every string. Choose open/closed schemas and path/normalization policies for the actual protocol; trusted internal data may rely on already established invariants.

## Freshness

Revalidate when the supported compiler/tool/library, platform, exposure, data representation, or adopted contract changes. Re-run relevant evidence and review any exception with its owner; dated examples and measurements are not continuing guarantees.

## Evidence

- [syntactic/semantic limits, Unicode, early bounds, and validation versus authorization](https://gitlab.maclaine.io/alm/rust-oe/-/blob/19e4133c0461129b0f5fdb286abd7b87511242e2/sources/owasp-input-validation.md)
- [overflow-checks are profile-dependent](https://gitlab.maclaine.io/alm/rust-oe/-/blob/19e4133c0461129b0f5fdb286abd7b87511242e2/sources/cargo-profiles.md)
- [literal arguments, Windows parsers, output/status/error distinctions](https://gitlab.maclaine.io/alm/rust-oe/-/blob/19e4133c0461129b0f5fdb286abd7b87511242e2/sources/std-process-command.md)
- [lexical components do not establish filesystem confinement](https://gitlab.maclaine.io/alm/rust-oe/-/blob/19e4133c0461129b0f5fdb286abd7b87511242e2/sources/std-path-components.md)
- [forms, concatenation, stream-safe changes, and version assumptions](https://gitlab.maclaine.io/alm/rust-oe/-/blob/19e4133c0461129b0f5fdb286abd7b87511242e2/sources/unicode-normalization.md)
- [manual Wasm byte bound and output alias defects](https://gitlab.maclaine.io/alm/rust-oe/-/blob/19e4133c0461129b0f5fdb286abd7b87511242e2/sources/pretzelhammer-rust-in-non-rust-servers.md)
- [reference validity beyond shape checks](https://gitlab.maclaine.io/alm/rust-oe/-/blob/19e4133c0461129b0f5fdb286abd7b87511242e2/sources/rust-reference-undefined-behavior.md)
- [stale checks/TOCTOU; canonicalized paths do not establish race-free confinement](https://gitlab.maclaine.io/alm/rust-oe/-/blob/19e4133c0461129b0f5fdb286abd7b87511242e2/sources/std-fs.md)
