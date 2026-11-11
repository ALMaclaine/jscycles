# Rust standards authority map

## Applicability and authority

- **Scope:** All proposals for Rust-OE guidance and downstream controls.
- **Authority:** Rust-OE local engineering guidance; the evidence sections identify upstream contracts and bounded experience.
- **Owner:** accountable: consuming repository's engineering owner (Rust-OE maintainers own this guidance); consulted: owners of the affected practices and controls. The consuming project names the responsible person/team before enforcement.
- **Reviewed:** 2026-10-20 (UTC). Version assumptions follow the named sources and the supported project toolchain/platform.
- **Adoption:** [Common ownership, exceptions, consumer, and verification contract](../docs/practice-adoption.md).

## Decision

Classify every proposed Rust rule by authority before making it a coding-agent prompt, review-bot check, or CI gate. Use the Rust Reference and Cargo documentation for semantics and tool behavior; use Rust-project guidelines for conventions; use tool documentation for a tool's bounded claim; treat organization and community guidelines as candidates requiring explicit local adoption.

## Risk

A project can turn an educational opinion, a dated repository, a tool limitation, or an inaccessible Reddit claim into a false “standard.” That produces invalid review failures, hides real uncertainty, and makes templates difficult to upgrade.

## Authority ladder

1. **Upstream behavior:** Rust Reference, standard library, Cargo, rustc, and rustdoc documentation. Cite these for `unsafe`, UB, trait/language semantics, Cargo resolution, test targets, and manifest behavior.
2. **Rust-project convention:** Rust API Guidelines, Style Guide, standard-library policies, and Clippy guidance. Use as defaults where applicable; state the scope and escape hatch.
3. **Tool contract:** a tool's maintained documentation, pinned version, and actual output (Miri, cargo-audit/deny/vet/auditable, fuzzing/mutation/coverage/semver tools). Assert only their documented result—never a broader property they cannot establish.
4. **Domain and organization policy:** product/security/safety requirements and external organization guidelines adopted by the owning organization; external guidelines (Microsoft, safety-critical) are candidates until adopted with recorded need, controls, owner, cost, and exceptions. They never override upstream semantics.
5. **Community material, experience, and research:** discovery or bounded-mechanism/experiment evidence until corroborated; popularity, authorship, or an index title never establishes a universal rule. Articles, course designs, proposals, and sampled discussions stay separate from current language contracts and local policy.

## Enforcement

Every permanent rule records source scope, authority, applicability, owner, enforcement, exceptions, and freshness. Reviewers request correction when a citation supports a narrower claim. Automated gates require their own verified detectable condition. Agents resolve a purported rule conflict by checking current upstream behavior and the local decision record; unresolved semantic or owner decisions stay explicit.

## Exceptions

A team may intentionally choose a stricter local policy than an upstream convention—such as forbidding unsafe code in an application crate. It must name the scope, rationale, escape hatch, and review owner; it must not be presented as a Rust-language requirement.

## Freshness

Revalidate compiler/toolchain-sensitive rules when upgrading Rust, MSRV, edition, or the relevant tool. Revalidate domain-policy sources on their maintainer's release cadence. Preserve historical guidance with a date and an explicit non-normative label.

## Evidence

- [introduction: reference scope and limitations](https://gitlab.maclaine.io/alm/rust-oe/-/blob/19e4133c0461129b0f5fdb286abd7b87511242e2/sources/rust-reference.md)
- [about: organizational scope, reasons, and team judgment](https://gitlab.maclaine.io/alm/rust-oe/-/blob/19e4133c0461129b0f5fdb286abd7b87511242e2/sources/microsoft-pragmatic-rust-guidelines.md)
- [local catalog: authority classification; repository synthesis, not an external standard](https://gitlab.maclaine.io/alm/rust-oe/-/blob/19e4133c0461129b0f5fdb286abd7b87511242e2/sources/rust-standards-catalog-2026-10-19.md)
- [Rust-project library conventions, not language requirements](https://gitlab.maclaine.io/alm/rust-oe/-/blob/19e4133c0461129b0f5fdb286abd7b87511242e2/sources/rust-api-guidelines-checklist.md)
- [default style convention and configuration judgment](https://gitlab.maclaine.io/alm/rust-oe/-/blob/19e4133c0461129b0f5fdb286abd7b87511242e2/sources/rust-style-guide.md)
- [standard-library safety-comment policy, adaptable as a convention elsewhere](https://gitlab.maclaine.io/alm/rust-oe/-/blob/19e4133c0461129b0f5fdb286abd7b87511242e2/sources/stdlib-safety-comments.md)
- [lint-group defaults; lint selection remains project policy](https://gitlab.maclaine.io/alm/rust-oe/-/blob/19e4133c0461129b0f5fdb286abd7b87511242e2/sources/clippy-lint-groups.md)
- [tool result bounded to exercised executions, version and configuration](https://gitlab.maclaine.io/alm/rust-oe/-/blob/19e4133c0461129b0f5fdb286abd7b87511242e2/sources/miri.md)
- [domain guideline effort, not general Rust law or certification evidence](https://gitlab.maclaine.io/alm/rust-oe/-/blob/19e4133c0461129b0f5fdb286abd7b87511242e2/sources/safety-critical-rust-guidelines.md)
