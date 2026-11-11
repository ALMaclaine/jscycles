# Cargo CI control selection

## Applicability and authority

- **Scope:** A consuming Cargo project's declared packages, targets, features, release and MSRV promise.
- **Authority:** Rust-OE local engineering guidance; the evidence sections identify upstream contracts and bounded experience.
- **Owner:** accountable: repository/CI maintainer; consulted: CI lane response owners. The consuming project names the responsible person/team before enforcement.
- **Reviewed:** 2026-10-20 (UTC). Version assumptions follow the named sources and the supported project toolchain/platform.
- **Adoption:** [Common ownership, exceptions, consumer, and verification contract](https://gitlab.maclaine.io/alm/rust-oe/-/blob/19e4133c0461129b0f5fdb286abd7b87511242e2/docs/practice-adoption.md).

## Decision

Construct CI from the repository's published support/release contract. Select reproducible inputs for protected checks and name remaining network/environment/runtime variability; pinning and `--locked` alone do not make arbitrary checks deterministic. Add compatibility, fresh-dependency, multi-platform, and MSRV lanes according to the consumer promise, cost, and response owner.

## Risk

A copied “standard Rust CI” matrix either omits a supported surface or wastes capacity on combinations nobody owns. Floating toolchains and dependency resolution turn unrelated ecosystem changes into opaque PR failures; conversely, testing only the lockfile hides breakage for consumers resolving allowed newer dependency versions.

## Required controls

1. Declare the supported Rust toolchain and, for a reusable or published package, `rust-version`. Pin the CI toolchain or make toolchain upgrades an explicit reviewable change.
2. On protected changes, run the project-selected build/test/lint/documentation checks with `--locked` where lockfile reproducibility is required. Keep the commands aligned with the actual packages, features, targets, and test types the project ships. Commit `Cargo.lock` by default, as `cargo new` does; another lockfile policy is a recorded project decision, and lockfile tests never stand in for dependency-range coverage.
3. For a reusable or published package with an MSRV, add an MSRV lane that builds and tests the supported packages/features on that exact toolchain; a check-only lane is a documented cost compromise.
4. Test fresh dependency resolution in a clearly labeled non-blocking or owned scheduled/update-PR lane. Use regular dependency-update PRs where possible so breakage has a review and rollback unit. With resolver `"3"` (the edition-2024 default), `cargo update` prefers versions compatible with `rust-version`; set the resolver's incompatible-rust-versions policy to `allow` (`resolver.incompatible-rust-versions`, or its environment equivalent for the selected Cargo release) for the latest-dependency lane, or state that the lane tests only MSRV-compatible versions.
5. Add target or feature matrix cells only for supported, divergent surfaces. Each cell needs an owner and failure policy. `--all-targets` selects Cargo target kinds—library, binaries, tests, benchmarks, and examples—not OS/architecture target triples.
6. Verify every supported non-host platform with an explicit `--target` and suitable runner/CI image; native execution may require a host or emulator for that target.
7. For a crate with a stable public API, classify compatibility before release. A piloted semver checker can supplement review using an explicit baseline, features, cfgs, and targets; its supported lints do not cover every behavioral or source-compatibility change. Applications may have protocol/schema compatibility needs without a crate-API comparison lane.

## Lint exceptions

Prefer `#[expect(lint, reason = "...")]` for a narrow intentional suppression when the project's MSRV supports lint expectations and actual lint behavior. Retain a justified `#[allow]` where expectation semantics, generated/macro code, older compilers, or scope-wide policy need it. Record the reason and narrow scope; do not enable the whole `restriction` or `nursery` group; enable `pedantic` selectively or with documented allowances. Upgrades can change findings and expectations.

## Enforcement

CI configuration documents: toolchain, lockfile policy, supported targets/features, required versus informational lanes, and response owner. Review bots flag a change to `rust-version`, public API, feature/target support, lockfile policy, or CI toolchain that lacks its corresponding verification update.

## Exceptions

An unpublished application/service binary need not define an MSRV, SemVer baseline, or broad target matrix. A library may defer fresh-dependency testing while it has no owner for update failures; record the gap rather than presenting lockfile-only tests as dependency-range coverage.

## Freshness

Revalidate when the supported compiler/tool/library, platform, exposure, data representation, or adopted contract changes. Re-run relevant evidence and review any exception with its owner; dated examples and measurements are not continuing guarantees.

## Evidence

- [provider examples, latest dependencies, MSRV cost and ownership](https://gitlab.maclaine.io/alm/rust-oe/-/blob/19e4133c0461129b0f5fdb286abd7b87511242e2/sources/cargo-continuous-integration.md)
- [resolver 3 default and rust-version-aware resolution](https://gitlab.maclaine.io/alm/rust-oe/-/blob/19e4133c0461129b0f5fdb286abd7b87511242e2/sources/rust-edition-guide-2024.md)
- [lockfile tracking default and package-specific choice](https://gitlab.maclaine.io/alm/rust-oe/-/blob/19e4133c0461129b0f5fdb286abd7b87511242e2/sources/cargo-faq-lockfiles.md)
- [package/target selection and locked/offline/frozen behavior](https://gitlab.maclaine.io/alm/rust-oe/-/blob/19e4133c0461129b0f5fdb286abd7b87511242e2/sources/cargo-build-command.md)
- [declaration versus verified support](https://gitlab.maclaine.io/alm/rust-oe/-/blob/19e4133c0461129b0f5fdb286abd7b87511242e2/sources/cargo-rust-version.md)
- [additivity, defaults, resolver and matrix cost](https://gitlab.maclaine.io/alm/rust-oe/-/blob/19e4133c0461129b0f5fdb286abd7b87511242e2/sources/cargo-features.md)
- [baselines/features/targets and omitted compatibility classes](https://gitlab.maclaine.io/alm/rust-oe/-/blob/19e4133c0461129b0f5fdb286abd7b87511242e2/sources/cargo-semver-checks.md)
- [opt-in groups, allowances, and restriction warning](https://gitlab.maclaine.io/alm/rust-oe/-/blob/19e4133c0461129b0f5fdb286abd7b87511242e2/sources/clippy-lint-groups.md)
- [allow_attributes: expect semantics and MSRV limitations](https://gitlab.maclaine.io/alm/rust-oe/-/blob/19e4133c0461129b0f5fdb286abd7b87511242e2/sources/clippy-lint-index.md)
- [resolver incompatible-rust-versions policy is a preference, not a filter](https://gitlab.maclaine.io/alm/rust-oe/-/blob/19e4133c0461129b0f5fdb286abd7b87511242e2/sources/cargo-configuration.md)
- [target tiers: build versus execution/test guarantees](https://gitlab.maclaine.io/alm/rust-oe/-/blob/19e4133c0461129b0f5fdb286abd7b87511242e2/sources/rust-platform-support.md)
