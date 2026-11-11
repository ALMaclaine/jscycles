# Maintainer, release and recovery runbook

Accountable maintainer: alm (existing repository owner). Rust-OE STD-037,
STD-042–045/065–072 and CI/DEPS/TOOLS at
`19e4133c0461129b0f5fdb286abd7b87511242e2`.

alm owns input/parser and graph changes, public contracts, dependencies/build
trust, test/fuzz findings, release preparation and security triage. Changes to
those paths need maintainer semantic review as well as automated checks. A local
agent checklist does not establish required human review, and the repository
has no CODEOWNERS file (adding one would not either without branch protection).
The GitHub account/team mapping, protected tag policy and privileged access have
not been verified. A public API check on 2026-11-06 confirmed that canonical
`ALMaclaine/jscycles` has unprotected `main`, no repository/parent rulesets and
no applicable branch rules. The expected CI/review enforcement is absent in
that snapshot; see the [verification record](evidence/rust-oe-linux-verification.json).

## Release preparation and verification

1. Review the full [adherence audit](rust-oe-full-audit.md), known gaps, public
   API/JSON changes and [migration notes](../migration-0.2.md). Classify the release
   before changing version/lockfile. The current changes require 0.2, not a
   replacement 0.1.1 binary. Re-run workspace CI, guidance/Python checks, the exact
   MSRV and affected support configurations. Review fresh dependency/advisory results.
2. Inspect the source package with `cargo package --locked --allow-dirty` during
   preparation; release publication must use a reviewed clean source revision.
   Ensure the binary package contains only intended source/tests/license material.
3. The release workflow accepts matching version tags. Manual dispatch must also
   select that tag; dispatch from a branch must fail without publication. Only
   the release job has contents-write permission. Cargo publication is restricted to crates.io and remains a manual maintainer
   operation requiring the same preparation and version review. No registry
   publication token is configured by this project work.
4. Build each release target natively. Check its executable format/CPU and runner
   against the selected target, exact reported version against Cargo.toml, usable
   help, and the known fixture's cycle witness with typed JSON/exit agreement.
   Require the binary's hash to remain unchanged throughout those checks, then
   retain its schema-2 build-info with binary identity/version, rustc, target,
   runner, source, workflow/run/attempt, actor and Cargo.lock digest. Collection
   rejects missing/unexpected assets, mixed identities, changed hashes and
   unverified binaries. It independently reads each downloaded binary's
   format/CPU and checks the recorded native runner and version.
   `scripts/test_release_record.py` tests failure/repair for all five formats/CPUs;
   synthetic headers do not execute another platform's binary locally.
5. Publish only after required CI and identity collection succeed. The release
   attaches binaries, SHA256SUMS and release-metadata.json. Download the published
   assets, compare each digest and run the supported binary's help/known-cycle
   scan. Record the release URL, source SHA, check-run URL, publication event,
   verification and approving maintainer in the release notes/incident record.
   Local tests do not perform or attest this publication step.

6. npm: after the GitHub release, the `npm` job stages one package per binary
   (`@jscycles/linux-x64`, `linux-arm64`, `darwin-x64`, `darwin-arm64`,
   `win32-x64`) and the `jscycles` launcher package with
   `scripts/npm_packages.py`, then publishes the platform packages first. A
   version already on the registry is skipped, so re-running the job after a
   partial failure completes the set. Publishing uses npm trusted publishing
   (GitHub OIDC; no token is stored) and npm attaches provenance.

   One-time setup, before the first tagged release that publishes to npm:
   npm only accepts a trusted publisher for a package that already exists, so
   the first version of each of the six packages is published by hand.
   1. Create the `jscycles` organization on npmjs.com (free plan).
   2. Download the release assets and `SHA256SUMS` into one directory, then
      run `npm login` and
      `python3 scripts/npm_packages.py publish --artifacts DIR --sums DIR/SHA256SUMS`
      from a checkout of the release tag.
   3. For each of the six packages, open its Settings on npmjs.com, add a
      trusted publisher (GitHub Actions, repository `ALMaclaine/jscycles`,
      workflow `release.yml`), and set publishing access to require two-factor
      authentication and disallow tokens.

Website: `.github/workflows/site.yml` builds `site/` on pull requests and
deploys it to GitHub Pages on every push to `main` that changes `site/`,
`docs/` or `README.md`; it is not tied to releases. One-time setup: in the
repository's Settings → Pages, set Source to "GitHub Actions". A custom
domain needs `site` changed (and `base` removed) in `site/astro.config.mjs`.

These are unsigned build identity records. No SLSA level, trusted-builder
attestation, byte-for-byte reproducibility, notarization or independent rebuild
is claimed. Selecting one of those promises requires its own format/verifier,
key policy and consumer-artifact verification pilot.
Header inspection recognizes the distributed ELF64, thin Mach-O64 and PE32+
format/CPU pairs; it does not verify the complete ABI, OS compatibility or
executable validity. Actual execution belongs to each native build job. The
runner/build records remain trusted workflow output, rather than independent
attestations or protection against a concurrently hostile filesystem.

## Failed or harmful release

Before publication, any failed test/digest/inventory gate must stop. Preserve the
failed run artifacts for diagnosis and rebuild a complete matching set; do not
combine attempts. After partial publication, mark the release as affected, retain
evidence/digests, notify consumers through release notes and publish a corrected
version. Avoid silently replacing assets or rewriting a released tag. Reinstall
a previous verified binary to roll back this stateless CLI. Do not claim that
reverting a source commit repairs already downloaded artifacts or reports.

Cargo registry publication, if selected later, is generally permanent. Yanking
prevents new resolutions but does not remove source, repair existing lockfiles,
or revoke a leaked credential. A credential incident requires revocation at its
issuer and review of downstream artifacts, not just removal from Git history.

## Vulnerability and incident handling

Scope includes parser crashes/resource failures, false clean scans that bypass a
consumer gate, private-data disclosure and dependency/build compromise. Preserve
a minimized synthetic reproducer, affected version/target, command/config limits,
source/artifact digest and observed impact. Keep proprietary source, credentials
and exploitable details out of public issues/logs.

After a private report is received, alm assesses impact/affected releases,
coordinates an embargo with the reporter and affected dependency owners, prepares
a tested fix and release/migration guidance, then communicates the update through
release notes and the security advisory when available. No response-time promise
or third-party embargo timeline is invented. Ordinary bug reports use repository
issues after private material is removed. [SECURITY.md](../../SECURITY.md) names GitHub
private vulnerability reporting as the route and the latest minor version as
supported; confirm in the repository settings that private reporting is
enabled before each release.

An incident closes only with a timeline, evidence, impact/affected versions,
correction, named follow-up owners, verification, communication and resulting
updates to tests, threat model and controls. Preserve the reproducer as an
ordinary test whenever practical. Review recurring failures as design/control
problems rather than only assigning blame.

## Handoff and privileged access

A real alternate maintainer/recovery owner has not been identified in the
available repository context. Before sole-maintainer absence, alm must designate
that person, transfer repository/registry/advisory and recovery access through
approved account mechanisms, rehearse access and release recovery, and record
where recovery instructions are stored. On departure or compromise, remove host,
registry, action-secret, deploy-key and runner access; rotate affected tokens and
verify an independent remaining owner can recover. A written checklist is not a
successful handoff or restore exercise. This remains OPEN until real principals
and a rehearsal are recorded.

## Evidence and development storage

Fuzz and coverage artifacts contain synthetic source/paths. Keep full local logs
under ignored target/tooling, fuzz build output under ignored fuzz/target, stable
non-sensitive seeds in version control, and minimized accepted regressions in
tests. No external code/duplicate-triage service is selected. Manual CI coverage
retention is seven days; production outputs are owned by the invoking user.

The initial audit measured approximately 5.5 GiB in target and 0.86 GiB in fuzz/target;
the subsequent closure pass measured 7.2 GiB and 1.0 GiB respectively before its
final release build. Multiple compilers, coverage, release, control pilots and
sanitizer builds multiply storage. Keep project-local outputs; do not set a
machine-global Cargo cache directory or run cargo clean as an automatic fix.
After retaining reports, alm can remove obsolete target/tooling/control-pilot,
coverage or fuzz build directories explicitly when disk pressure justifies the
rebuild cost. CI caches are acceleration, not provenance/reproducibility evidence.

The [profile/workspace breakdown](evidence/rust-oe-closure-storage.json) records allocated
blocks for normal debug/release, LLVM coverage, control/fuzz tooling, packages and
the separate fuzz workspace. Debug output mixes the selected compilers; its
fingerprints do not provide an exact per-toolchain or per-package disk bill.
Before extending campaigns or adding compiler/target/profile variation, alm
checks these categories with `du -sk` and reviews growth. The project-local
advisory review threshold is **12 GiB**, selected after the approximately 8.4 GiB
measurement. Crossing it requests an explicit storage/rebuild-cost decision; it
does not delete files, impose a machine-wide quota or fail a semantic CI check.
Revalidate the threshold on a scope/cache-layout change.
