# Dependency audit policy (cargo-vet / third-party audit coverage)

## Applicability and authority

- **Scope:** Projects evaluating or adopting third-party audit coverage.
- **Authority:** Rust-OE local engineering guidance; the evidence sections identify upstream contracts and bounded experience.
- **Owner:** accountable: dependency-risk owner; consulted: security owner. The consuming project names the responsible person/team before enforcement.
- **Reviewed:** 2026-10-20 (UTC). Version assumptions follow the named sources and the supported project toolchain/platform.
- **Adoption:** [Common ownership, exceptions, consumer, and verification contract](https://gitlab.maclaine.io/alm/rust-oe/-/blob/19e4133c0461129b0f5fdb286abd7b87511242e2/docs/practice-adoption.md).

## Decision

Treat adopted third-party audit coverage as version-controlled trust policy tied to the resolved graph. Consider cargo-vet after selecting criteria, audit sources, exemption ownership, and review cadence. The selected setup, configuration, check and CI contracts now have individual source reviews, and the template provides opt-in commands. A consuming project still selects its audit criteria, imports, exemptions, owner and failure policy.

## Risk

A dependency graph can change without an explicit human assessment. Exemptions can make an audit pass without completed review; unreviewed policy edits can silently expand trust.

## Adoption sequence

1. Identify the dependency-risk owner and choose the minimum audit criteria that match the product's deployment and execution model.
2. Read the selected release's initialization and CI procedure, inspect its generated files, and trial it in an isolated project. Record the verified command, graph/lockfile selection, policy schema, cache/network inputs, and exit behavior before adoption.
3. Review any exemptions created during initialization as an inventory. Assign owner/reason/review cadence and reduce them only through accepted audits, imported evidence, or a documented risk acceptance.
4. If adopted, pin the verified invocation and evaluate manifest, lockfile, policy, and dependency-update changes against the intended graph. Freshness of imported audits and cached inputs remains explicit.
5. Review policy diffs like code: criteria, imported audit sources, audits, exemptions, and any other trust entries the selected release supports each change the trust boundary.

## Enforcement

- A consuming project's adopted gate blocks only according to its verified command/configuration and declared failure policy. No cargo-vet command or trusted audit set is activated by this guidance alone.
- Review bots request a rationale for added/expanded exemptions and reject unowned blanket exemptions under the project's policy.
- Keep audit artifacts in version control; the lockfile and vet policy are one review unit.

## Exceptions

The template exposes an explicitly initialized store and a locked check; the recorded optional-tool pilot bounds the verified configuration. Do not seed a generic template with an audit set, imported trust entry, or exemption. cargo-vet does not replace vulnerability advisories, license review, sandboxing, or ongoing maintainer-risk assessment. A project may defer adoption while it establishes a dependency-owner and policy process; document the gap rather than treating an empty setup as assurance.

## Consumers and freshness

Coding agents must preserve supply-chain files during dependency updates and must not create exemptions to make CI pass. The dependency-risk owner reviews policy changes. Reassess criteria and the pinned cargo-vet version on tool upgrades or when the dependency threat model changes.

## Evidence

- [Cargo Vet: project setup](https://gitlab.maclaine.io/alm/rust-oe/-/blob/19e4133c0461129b0f5fdb286abd7b87511242e2/sources/cargo-vet-setup.md)
- [Cargo Vet: policy and store configuration](https://gitlab.maclaine.io/alm/rust-oe/-/blob/19e4133c0461129b0f5fdb286abd7b87511242e2/sources/cargo-vet-configuration.md)
- [Cargo Vet: selected initialization and check commands](https://gitlab.maclaine.io/alm/rust-oe/-/blob/19e4133c0461129b0f5fdb286abd7b87511242e2/sources/cargo-vet-commands.md)
- [Cargo Vet: continuous-integration invocation](https://gitlab.maclaine.io/alm/rust-oe/-/blob/19e4133c0461129b0f5fdb286abd7b87511242e2/sources/cargo-vet-ci.md)
- [README purpose; detailed initialization and CI claims explicitly excluded](https://gitlab.maclaine.io/alm/rust-oe/-/blob/19e4133c0461129b0f5fdb286abd7b87511242e2/sources/mozilla-cargo-vet.md)
- [sharing, relative audits, and deferred exemptions](https://gitlab.maclaine.io/alm/rust-oe/-/blob/19e4133c0461129b0f5fdb286abd7b87511242e2/sources/cargo-vet-introduction.md)
- [published advisory matching and scan limits](https://gitlab.maclaine.io/alm/rust-oe/-/blob/19e4133c0461129b0f5fdb286abd7b87511242e2/sources/cargo-audit.md)
- [selected policy checks; not policy selection](https://gitlab.maclaine.io/alm/rust-oe/-/blob/19e4133c0461129b0f5fdb286abd7b87511242e2/sources/cargo-deny.md)
