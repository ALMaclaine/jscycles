# Optional project tools

The baseline remains `cargo xtask ci`. Optional tools have explicit commands, pinned versions in [tools.json](tools.json), and manual GitLab jobs. A nonzero tool exit remains a failed job; these jobs are advisory through GitLab's `allow_failure`, rather than shell commands that hide errors. Change that policy only after selecting a real scope, owner, runtime budget, and failure response.

[Shared CI runtime](https://gitlab.maclaine.io/alm/rust-oe/-/blob/19e4133c0461129b0f5fdb286abd7b87511242e2/tooling/ci.md) records the shared core, analysis and dated-nightly component/image pins, verification, caches, reports and rollback. CI uses preinstalled tools; local wrappers and explicit installations remain available.

Run from the repository root. `cargo xtask tools` lists pins. Install only what you need:

```sh
cargo xtask install-tools vet mutants
cargo xtask install-tools miri
cargo xtask install-tools coverage crap
cargo xtask install-tools fuzz semver
```

With no tool names, `install-tools` retains its baseline cargo-deny behavior. Cargo installations use exact versions and the package's published lockfile; installation/network/compiler failures propagate. Miri and fuzzing use the separately pinned `nightly-2026-10-19`; the default compiler stays Rust 1.98.1. Coverage installs that compiler's matching LLVM tools. Tool checks reject missing or mismatched versions with the named installation command.

## Commands and project configuration

| Command | Contract | Inputs and escape hatch |
| --- | --- | --- |
| `cargo xtask vet-init` | Explicitly initialize the local Vet store with the selected binary | Generates exemptions; this is inventory, not audit completion. Review generated files and decide criteria/imports/exemptions before committing them. Never regenerate exemptions just to make CI green. |
| `cargo xtask vet` | Check selected audit policy with locked imports and no automatic exemption minimization | Requires `supply-chain/config.toml` and `audits.toml`, or an explicit `--store-path`. No store, trust import, audit attestation, or blanket exemption is shipped. The manual job appears after a project supplies its store. |
| `cargo xtask mutants --file crates/rust-oe-cli/src/main.rs` | Isolated-copy mutation testing with one worker and a 60-second per-mutant timeout | Requires explicit `--file`, `--package`, or `--workspace` selection. The manual job accepts an explicit scope through job-local `RUST_CI_ARGUMENTS`, for example `["--file", "src/domain.rs"]`, with a 15-minute job timeout. `--in-place` is rejected. Surviving mutations require triage; they are not automatically product defects. |
| `cargo xtask miri` | Interpret selected tests, with default features, on the dated nightly | The manual CI lane uses `--package rust-oe-cli domain_` for pure validation/rendering/hash tests; Git/process/filesystem tests run natively. Extra Cargo selection arguments can narrow packages/tests. The interpreter checks executed paths and supports only selected APIs. Native I/O/FFI/platform/concurrency tests remain separate evidence. |
| `cargo xtask coverage` | Produce LCOV for workspace tests with all features | Writes `target/tooling/coverage/lcov.info`. This measures selected exercised lines, not every branch, configuration, doctest, or correctness property. No percentage threshold is adopted. |
| `cargo xtask crap` | Consume LCOV and produce an advisory complexity/coverage report | Run coverage first or pass `--lcov FILE`; missing input fails. Writes `target/tooling/crap.json`. No `--fail-above` or regression gate is enabled by default. External duplicate triage remains off. |
| `cargo xtask fuzz TARGET` | Run an owned harness for at most 30 seconds, with a five-second per-input timeout | Requires `fuzz/Cargo.toml`, a reviewed separate `fuzz/Cargo.lock`, and a job-local `RUST_CI_ARGUMENTS` array such as `["owned_target"]` for the manual job. An optional second argument replays a corpus/artifact. Lockfile changes fail the wrapper. An empty demonstration harness would provide no project assurance, so none is shipped. |
| `cargo xtask semver --baseline-rev REV` | Compare a public library against an explicit revision or `--baseline-version VERSION` | The binary-only starter has no public library API. Adopters supply one and declare the compatibility contract; the manual job accepts job-local `RUST_CI_ARGUMENTS` such as `["--baseline-rev", "FULL_BASELINE"]` and fetches complete Git history. Behavioral compatibility remains review. |
| `cargo xtask profile` | Run the bounded Hotpath example and emit a local JSON report | Writes `target/tooling/hotpath/report.json`. The example preserves a known numeric result. Its timings establish instrumentation operation only, not application latency, overhead budgets, or a performance regression gate. |

Optional command arguments are forwarded as argument tokens, not shell text. Use the supported wrappers for the documented baseline; invoking an upstream tool directly selects a different configuration and needs corresponding evidence. Unknown baseline-task arguments must not silently change what a caller believes was checked.

## Hotpath

In the xtask package, the optional `profiling` feature enables exactly Hotpath 0.28.6 with default features disabled and timing instrumentation selected. The CLI does not depend on Hotpath. Both normal and instrumented builds are checked; the existing all-features baseline also compiles the instrumented path.

```sh
cargo run --package xtask --example profile --no-default-features --locked
cargo xtask profile
```

The shipped example is a bounded instrumentation smoke workload. Replace or complement it with a named representative application workload before using profiles to justify optimizations or timing gates. Specify sampling, warm-up, variance, metric, and owner for any performance budget. Reports remain local; Cloud, agent initialization, MCP, Prometheus endpoints, and allocation/CPU instrumentation are not enabled by this feature. The tool initializer is not run.

## Artifacts, ownership, and limits

The repository maintainer owns these wrappers and their pins. A consuming project names its actual tool/policy owner and workload/test scope. GitLab retains optional reports for one week under `target/tooling`; crash corpora or mutation output can contain inputs or code and require the consuming project's data/retention decision. Tool jobs retain a 15-minute bound with a 12-minute helper deadline; semver retains 45 minutes with a 40-minute helper deadline. Packaged tools are preinstalled, and remaining time depends on the selected project workload. A custom campaign may need a narrower scope or an explicitly reviewed budget.

Vet trust choices and exemptions belong to the consuming project. There is no pre-filled audit claim. Fuzz harnesses and semver release baselines are similarly project inputs. Unsupported or missing configuration fails explicitly, rather than reporting a successful check with zero useful work. A manually selected mutation scope that finds no applicable mutants is not evidence of test strength.

Upgrade tool/nightly/configuration pins through a review that runs a passing example, a meaningful rejected example, and recovery; record runtime and artifacts. Keep ordinary tests alongside specialized tools. Disable an optional lane explicitly when its prerequisites do not apply, with an owner and alternative verification; do not suppress failures inside the command.

## Other catalog entries

Machete/geiger remain advisory dependency/unsafe-inventory candidates. All-features enumeration needs a bounded library matrix; udeps needs a dedicated nightly maintenance use case. Chef requires a measured container-build bottleneck. Release-plz/git-cliff require an owned registry-release workflow and commit policy. Just overlaps the existing `xtask` facade, and prek is an opt-in local convenience. None is silently installed by `install-tools`; these dispositions remain visible rather than becoming empty gates.

## Primary references

- [Vet setup](https://mozilla.github.io/cargo-vet/setup.html), [configuration](https://mozilla.github.io/cargo-vet/config.html), [commands](https://mozilla.github.io/cargo-vet/commands.html), and [CI contract](https://mozilla.github.io/cargo-vet/configuring-ci.html).
- [Mutants](https://github.com/sourcefrog/cargo-mutants), [Miri](https://github.com/rust-lang/miri), and [cargo-fuzz](https://github.com/rust-fuzz/cargo-fuzz).
- [LLVM coverage](https://github.com/taiki-e/cargo-llvm-cov), [CRAP](https://github.com/minikin/cargo-crap), and [semver-checks](https://github.com/obi1kenobi/cargo-semver-checks).
- [Hotpath manual setup](https://hotpath.rs/) and the selected package's feature/API definitions.
