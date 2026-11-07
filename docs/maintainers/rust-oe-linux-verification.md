# Linux continuation and forge verification

Owner: alm. Rust-OE source remains
`19e4133c0461129b0f5fdb286abd7b87511242e2`; TEST-003/006/007,
CI-004/005/007/008 and STD security/recovery practices. This continuation follows
the [combined review](rust-oe-current-review.md). Its
[machine-readable evidence](evidence/rust-oe-linux-verification.json) records the current
state; earlier Mac failures remain historical evidence with their original scope.

## Linux verification

A dedicated local Colima VM supplies four ARM CPUs, 8 GiB RAM and a 30 GiB sparse
disk limit. The verification container is limited to 7 GiB RAM and uses the
official Rust 1.98.1 Debian image at digest
`sha256:93ce27a88655056a51dbdd8f5f2d7ddc071c7b0070fb288a37b5a285fc83971e`.
It receives a source archive, without host home mounts, credentials, the Git
database or Mac build artifacts. Docker's existing active context is preserved.
This is development infrastructure, not a new product/container support promise.

All selected source/test/configuration hashes match the Mac tree exactly.
Linux passes 526 native target tests and one doctest with all features, including
the two additional Linux-only cases: a real `/dev/full` output failure and a
non-UTF-8 configuration-origin path. It also passes 60 Python tests and guidance
integrity.
The full mutation selection contains the same 790 identities across all 16
production modules. Cargo tests, two mutant workers, two compilation jobs per
Cargo command, 60-second test and 180-second build limits, exact exclusions and
timeout review policy remain unchanged. A separate 150-minute supervisor limit
bounds this attempt; an interrupted/incomplete run cannot pass review.

**The complete 790-case run and both independent strict reviews pass. L01 is
closed.** The run finishes in 53 minutes: 644 raw caught cases, 134 uncompilable
and 12 reviewed test timeouts, with zero survivors, unexplained timeouts or build
timeouts. Linux all-target/all-feature workspace Clippy also passes with warnings
denied. No production Rust, test, manifest, lock, runner or mutation-policy edit
was needed in this continuation.

The raw caught count includes one existing reviewed nonadvancing `circuit` loop
that reaches the container's memory limit and receives SIGKILL. It is recorded
separately from the other 643 actual failing-test detections. The memory-event
counter matches that one named resource kill; no compiler or unknown case was
credited for a resource failure. All 134 uncompilable cases have actual Rust
syntax/type diagnostics. All 12 timeouts match existing exact-diff loop reviews.
No exclusion, hang exemption, longer timeout or rewritten outcome was introduced.

The exact workspace mutation responsible for the rejected Mac build timeout now
builds in about 9.4 seconds and fails a test in about 0.2 seconds on Linux. The
fresh complete Linux campaign supplies new passing evidence; the original Mac
build timeout, unsuccessful complete campaign and interrupted attempts remain
failed historical records. Their replays are not substituted for this run.

The owned container and VM are stopped after verification, preserving their data
and copied raw artifacts while releasing RAM. The original Docker context and
unrelated profiles remain unchanged. This is scoped local execution evidence,
not proof of correctness, native-provider soundness or hosted enforcement.

## Verified public forge facts

Read-only public GitHub REST responses on 2026-11-06 establish:

- `EnderHub/jscycles` redirects to canonical `ALMaclaine/jscycles`.
- `main` at `86b1bbfa99ac902bf00ff8ac7c2f4b54ce7ee480` is unprotected.
- The repository/parent ruleset list and applicable `main` rule list are empty.
- Private vulnerability reporting is disabled.

These are observed gaps, replacing the earlier unknown settings. The proposed
`Required checks` workflow is not an enforced merge requirement in this snapshot.
The candidate private reporting form is not a working verified route. GitHub
settings were not changed, and no test report or message was sent to anyone.
Endpoint semantics are documented in GitHub's
[branch-rules API](https://docs.github.com/en/rest/repos/rules),
[branch-protection API](https://docs.github.com/en/rest/branches/branch-protection)
and [repository API](https://docs.github.com/en/rest/repos/repos).

The configured GitHub credential is rejected with HTTP 401; the ordinary Git
credential helper also exceeded its bounded lookup time. The available public
API suffices for the facts above. No credential value is retained. Account/team
privileges, tag/publisher access, private receivers and an independent recovery
rehearsal remain unverified. The canonical owner account does not identify or
verify an alternate recovery person.

## Remaining boundary

L01 is closed by the complete Linux campaign and unchanged-input strict review.
G01 still needs an operational private route, actual receivers and a support-window
decision. G02 needs actual access/recovery evidence. G03 needs enforced checks
and execution of this proposed tree on the remaining supported platforms/current
stable. This local ARM Linux result does not establish x86 Linux, Windows or
other Mac execution, and the uncommitted workflows have not run on GitHub.
C01 remains release-time review, authorization and consumer-artifact verification.

The continuation does not establish full adherence. It closes only the named
items that have actual passing evidence; no operational fact or hosted result is
invented to turn the map green.
