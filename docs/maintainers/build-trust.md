# Dependency and executable build trust

Owner: alm. Rust-OE STD-014/031–034, SAFE/FFI, DEPS, BUILD and TOOLS at
`19e4133c0461129b0f5fdb286abd7b87511242e2`.

Cargo.lock is part of the change review, including normal/dev/build dependencies
and all supported target conditions. The inventory in build-trust.json is produced
from locked Cargo metadata and registry lock checksums; it identifies packages
with build scripts/procedural macros and their selected features. It is not a
source audit or an inventory of executed code on every target. Some cross-target
packages are not built on the native host. Recreate it after any graph/feature or
toolchain change, and inspect the diff rather than approving a package count.
The current inventory records 134 application-workspace packages with all features (42 with build
scripts/macros) and the independently locked 78-package fuzz workspace (24 with
build scripts/macros), including its bundled libFuzzer C++ build input. Each graph
has its own manifest/lock hashes; the development license exception is hashed too.

The runtime parser trust boundary is the willbooster-tree-sitter runtime and the
JavaScript and TypeScript grammar packages (including TSX). The cursor-based extraction pass
removed ast-grep-core and its bit-set/bit-vec dependencies without changing the
parser runtime or grammar versions. It uses the same safe provider interfaces,
checks the native u32 input range, and retains compiler/target/sanitizer checks.
We previously removed the general
ast-grep-language dependency, which enabled 22 grammar packages, and use a private
safe language selector for the two supported grammar packages. The previous
per-language feature attempt failed real parser tests because that release's
factory remained gated by its umbrella feature; it was discarded. Tests now
exercise the actual supported grammars with the smaller graph. Unused ignore,
glob-match and clap derive dependencies were also removed. This is input-surface
and build-trust reduction, not proof that the retained native code is sound.

The C compiler/cc build machinery and native grammar sources execute during a
build. Serde/thiserror and other selected procedural macros execute in the compiler
process. Their registry source checksums, lock versions and selected features are
review inputs; normal unsafe-code bans do not apply inside those dependencies.
Read a changed build.rs/macro/native source and its upstream release notes before
accepting a relevant update. Do not execute project checkout scripts during a
scan. There is no project build.rs, source replacement, patch override, custom
registry or allowed Git dependency in the application graph.

The explicit cargo-deny 0.19.8 policy rejects known advisories/yanked versions,
unapproved licenses and unknown sources across normal/dev/build inputs. It does
not establish absence of compromise or perform a code audit. Refresh advisory
inputs and triage failures on changes/weekly maintenance; never add an advisory
ignore or trust exemption just to get a green run. The prior crossbeam-epoch and
YAML replacements remain recorded in the adoption history.

The later import-semantics pass encountered a fresh registry yank of
serde-saphyr 2.1.0. Both locks now select 2.1.1, and Cargo.toml requires at least
that patch. No other resolved package version changed. The packaged changelog
and five changed Rust deserializer files were reviewed: empty-stream handling,
allocation changes, duplicate-key bookkeeping and diagnostics. The declared MSRV
remains 1.89, below this project's 1.93 floor; no build script, procedural macro,
native grammar, feature or license exception was added. The refreshed inventory
retained the then-current 133/90 packages and 42/27 executable build inputs. Native CLI
regressions distinguish blank/default configuration from explicit null/invalid
shapes; fresh configuration and native-source sanitizer campaigns revalidate the
updated graph. This targeted review is not a full third-party source audit.

On 2026-11-06 the parser moved from tree-sitter-facade-sg 0.24.5 (tree-sitter
0.24.7) with tree-sitter-javascript 0.23.1 and tree-sitter-typescript 0.23.2 to
WillBooster's forks: willbooster-tree-sitter =18.0.2,
willbooster-tree-sitter-javascript =22.0.1 and willbooster-tree-sitter-typescript
=26.0.1 (MIT, crates.io, imported under the upstream crate names). The upstream
grammars are the latest releases and report ERROR nodes for valid tagged
templates with `\u` escapes and `typeof import()` type arguments, which the strict
syntax policy (contract R04) turns into exit 2; over 5,461 files from six real
repositories upstream rejected 23 valid files and the forks rejected only the
three intentionally broken fixtures. The fork runtime uses a private ABI number
that upstream runtimes reject, so runtime and grammars change together and are
pinned exactly because the forks publish breaking grammar changes as major
versions. The three build scripts were read: the runtime compiles `lib.c` and the
grammars compile their generated `parser.c`/`scanner.c` with `cc`, with no
network, environment-driven code or file writes outside `OUT_DIR`. New runtime
dependencies are regex (already in the graph) and streaming-iterator; the facade,
its wasm-bindgen inputs and tree-sitter 0.24.7 left the graph, and serde_json
moved to 1.0.154. The runtime declares Rust 1.93, so the MSRV rose from 1.92.0 to
1.93.0. A single-vendor fork is a supply-chain concentration: review each pinned
bump's grammar diff and rerun the corpus probe before accepting it.

Cargo-vet is not initialized: no audit criteria/import set/exemptions have been
approved. This conditional practice is not satisfied by an empty store or bulk
exemptions. The selected alternative is explicit source/license/advisory policy,
reviewed lock/build inventory and targeted native testing/fuzzing. Revisit Vet
with alm when a specific accepted audit source/criteria can reduce this risk.

CI actions are pinned in workflow source. Job permissions default to contents-read;
only the tag release publisher has contents-write. No third-party review/telemetry
or signing credential is introduced. Hosted images, downloads/tool installation,
compiler/native environment and host access policy remain separate moving/trusted
inputs. Release metadata records the actual source/workflow/builder/lock/artifact
identities; it does not claim signed provenance or reproducibility.

The separate fuzz workspace has a separate lock and is evaluated by the same
cargo-deny policy from `cargo xtask deny`. Its only additional license exception,
loaded from fuzz/deny.exceptions.toml when running in that workspace, is NCSA for
exactly libfuzzer-sys 0.4.13. The published manifest/README assigns that license
to its bundled libFuzzer sources; the [SPDX NCSA text](https://spdx.org/licenses/NCSA.html)
was reviewed for its notice/disclaimer retention and no-endorsement conditions.
The exception is confined to this nonpublished development harness. Preserve
third-party notices if distributing a harness binary. It neither widens the
application allowlist nor supplies a Vet/advisory exemption. Owner alm revalidates
it on a version/usage/distribution change. The initial graph check also detected
an unversioned local path dependency, now constrained to the current package
version; update that bound when preparing 0.2.

The control replay checks passing, undeclared-license failure, and repaired
behavior on the actual fuzz graph. No native-only harness should bypass dependency
policy merely because it lives outside the primary Cargo workspace.

The text-adapter harness adds serde_json as a direct independent oracle dependency.
It was already present at the same resolved version in the fuzz graph; lock review
confirmed only the local fuzz package's dependency list changed. The updated
inventory retains both manifest/lock identities and the unchanged version-scoped
license exception. No additional package version, native grammar or trust entry
was introduced by that target.

The explicit local profiling feature adds Hotpath 0.28.6 and its locked timing
support graph. Hotpath's build.rs invokes the selected rustc with `--version`
and reads Cargo environment/profile information. The added hotpath-macros
0.28.7 procedural macro transforms measured functions into timing guards;
reviewed code generation preserves normal return/error propagation. The new
inventory also includes time-macros 0.2.32 and conditional Windows build inputs.
Cloud, MCP, Prometheus, allocation and CPU sampling features are disabled.
Reports require an explicit HOTPATH_OUTPUT_PATH and stay local unless a maintainer
chooses to share them. The default product build has no Hotpath dependency.
