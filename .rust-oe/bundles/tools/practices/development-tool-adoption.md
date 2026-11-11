# Development-tool adoption

## Applicability and authority

- **Scope:** Proposed tools, hooks, workflow dependencies, or template/CI additions.
- **Authority:** Rust-OE local engineering guidance; the evidence sections identify upstream contracts and bounded experience.
- **Owner:** accountable: workflow/template maintainer; consulted: affected project owner. The consuming project names the responsible person/team before enforcement.
- **Reviewed:** 2026-10-20 (UTC). Version assumptions follow the named sources and the supported project toolchain/platform.
- **Adoption:** [Common ownership, exceptions, consumer, and verification contract](https://gitlab.maclaine.io/alm/rust-oe/-/blob/19e4133c0461129b0f5fdb286abd7b87511242e2/docs/practice-adoption.md).

## Decision

Adopt a development tool only after it is mapped to one observable Rust-project outcome: detecting a plausible defect, validating a supported configuration, improving dependency/unsafe visibility, shortening a release path, or removing a measured build bottleneck. Prefer one tool per responsibility. Pilot it in a representative project before placing it in `rust-template` or making it a required CI dependency.

## Risk

Unpiloted tools introduce overlapping gates, privileges, cost, or false-positive failures.

## Evaluation contract

1. **Claim:** state the defect, delay, or manual error it addresses—not merely its feature list.
2. **Consumer:** identify the project developer, CI, release owner, or operator.
3. **Boundary:** identify where it runs and what it can modify/send/upload. Local tooling must not silently become a CI or credential boundary.
4. **Evidence:** run the candidate on a representative project, retain command/output, and compare runtime, false positives, and maintenance cost against the current workflow.
5. **Control:** pin the version and configuration; document input/output artifacts, required permissions, failure policy, and escape hatch.
6. **Cutover:** select one primary tool for an overlapping responsibility and remove/avoid duplicate defaults.

## Candidate directions

- Configuration correctness: evaluate **cargo-all-features** for public libraries with a deliberately bounded feature set; supported feature/target combinations remain an explicit CI contract.
- Unsafe surface: use **cargo-geiger** as advisory evidence only; review unsafe contracts, not counts.
- Dependency hygiene: evaluate **cargo-machete** as advisory output. `--with-metadata` invokes `cargo metadata` and can rewrite `Cargo.lock`; run it in a disposable checkout or verify the lockfile is unchanged. Validate every proposed removal across supported targets/features/build/dev/doc use. Reserve nightly **cargo-udeps** for a non-blocking maintenance lane.
- Command discoverability: evaluate **just** as a thin facade around named verified commands.
- Local hooks: evaluate **prek** as opt-in convenience. Protected server-side checks and required human approvals remain authoritative under the selected forge's verified policy; a local hook is bypassable. GitHub CODEOWNERS routing does not by itself require approval; other forges need their own source.
- Container efficiency: evaluate **cargo-chef** only for a measured Docker-build bottleneck.
- Registry releases: evaluate **release-plz** only for owned Cargo registry releases; it already integrates changelog generation.

## Explicit non-adoptions

- Agent-review panes, VCS replacements, branch-deletion tools, package/environment managers, editor extensions, and alternative build systems are not Rust-project controls. They need a separate demonstrated workflow need.
- No static metric, unused-dependency detector, or all-features enumeration is a gate until its false-positive/failure behavior and CI budget are proven.

## Enforcement

Template changes require a source record, pilot evidence, pinned version/configuration, a project-facing command, and an opt-out/rollback instruction. Do not add a tool merely because it is popular or written in Rust. Review must reject a new workflow dependency that lacks a stated contract and ownership.

## Exceptions

Complementary tools may share a workflow when their distinct responsibilities justify the cost. Local convenience tools remain optional; no catalog membership activates a gate.

## Freshness

Revalidate when the supported compiler/tool/library, platform, exposure, data representation, or adopted contract changes. Re-run relevant evidence and review any exception with its owner; dated examples and measurements are not continuing guarantees.

## Evidence

- [local tool-specific dispositions and primary-source scope](https://gitlab.maclaine.io/alm/rust-oe/-/blob/19e4133c0461129b0f5fdb286abd7b87511242e2/sources/development-tools-triage.md)
- [bounded feature-combination exploration](https://gitlab.maclaine.io/alm/rust-oe/-/blob/19e4133c0461129b0f5fdb286abd7b87511242e2/sources/cargo-all-features.md)
- [advisory unsafe inventory limits](https://gitlab.maclaine.io/alm/rust-oe/-/blob/19e4133c0461129b0f5fdb286abd7b87511242e2/sources/cargo-geiger.md)
- [heuristic unused-dependency findings](https://gitlab.maclaine.io/alm/rust-oe/-/blob/19e4133c0461129b0f5fdb286abd7b87511242e2/sources/cargo-machete.md)
- [nightly and selected-build limits](https://gitlab.maclaine.io/alm/rust-oe/-/blob/19e4133c0461129b0f5fdb286abd7b87511242e2/sources/cargo-udeps.md)
- [thin command facade; not a build-system guarantee](https://gitlab.maclaine.io/alm/rust-oe/-/blob/19e4133c0461129b0f5fdb286abd7b87511242e2/sources/just-command-runner.md)
- [local hooks and execution boundaries](https://gitlab.maclaine.io/alm/rust-oe/-/blob/19e4133c0461129b0f5fdb286abd7b87511242e2/sources/prek.md)
- [Docker dependency layer caching scope](https://gitlab.maclaine.io/alm/rust-oe/-/blob/19e4133c0461129b0f5fdb286abd7b87511242e2/sources/cargo-chef.md)
- [registry-release/changelog responsibility](https://gitlab.maclaine.io/alm/rust-oe/-/blob/19e4133c0461129b0f5fdb286abd7b87511242e2/sources/release-plz.md)
- [GitHub CODEOWNERS routing versus required approval; other forges need their own source](https://gitlab.maclaine.io/alm/rust-oe/-/blob/19e4133c0461129b0f5fdb286abd7b87511242e2/sources/github-codeowners.md)
