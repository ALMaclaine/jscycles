---
title: Is this AI slop?
description: How jscycles was built with an AI coding assistant, and the evidence that it is correct anyway.
---

A fair question. Much of jscycles' code, tests and documentation was written
with an AI coding assistant, working under a human maintainer who decides what
ships. Commits it helped write say so in a `Co-Authored-By` line.

"AI slop" means output nobody checked: code that looks plausible, docs that
describe features that do not exist, tests that test nothing. The answer to it
is not to avoid the tools but to check the result harder than usual. This page
lists what that checking is, so you can judge for yourself. Every claim links
to something in the repository you can run or read.

## Checked against TypeScript itself

The hardest part of finding import cycles is resolving each import to the right
file. jscycles is not trusted to agree with itself here.

- **A differential test against the TypeScript compiler.** On every pull
  request, [`difftest.py`](https://github.com/ALMaclaine/jscycles/blob/main/scripts/differential/difftest.py)
  generates 1,000 random projects (workspaces, nested packages, tsconfig
  aliases, every spelling of a relative path, BOMs, unusual extensions) on
  both Linux and Windows. An independent reference model resolves each import
  with TypeScript 5.9.3's own `resolveModuleName`, computes the cycles by brute
  force, and compares them with `jscycles --json`. Any disagreement is shrunk
  to a minimal project and saved.
- **Exhaustive graph checks.** The cycle algorithms are compared with an
  independent brute-force enumerator on all 4,096 directed graphs with four
  nodes, and on every graph with three nodes under Miri. "Representative cycle
  per tangle" and "every elementary package cycle" are checked against
  reachability computed separately, not against the implementation.
- **Real repositories.** [`real_repos.py`](https://github.com/ALMaclaine/jscycles/blob/main/scripts/differential/real_repos.py)
  records the full output on real repositories and reports every change on
  the next run, so a change in behavior is reviewed rather than discovered.

## Tests that are themselves tested

Coverage measures which lines ran, not whether a test would notice a bug.
jscycles uses mutation testing for that: [cargo-mutants](https://mutants.rs/)
deliberately breaks the code (flips a comparison, deletes a branch, returns a
constant) and checks that some test fails.

- The full campaign on the release tree produced **970 mutants: 800 caught,
  157 that do not compile, 13 timeouts, each reviewed, and none missed**.
- Every mutant that survives is either killed with a new test or proven
  harmless. The few proven harmless are listed in
  [`.cargo/mutants.toml`](https://github.com/ALMaclaine/jscycles/blob/main/.cargo/mutants.toml)
  by exact line, each with the reason it cannot change behavior. Moving the
  code invalidates the entry and forces a new review.

Around this sit about 600 Rust tests, including a
[spelling matrix](https://github.com/ALMaclaine/jscycles/blob/main/tests/spelling_matrix.rs)
that checks every path-taking input means the same thing however it is written
(`x`, `./x`, `x/`, absolute, `..` detours), and refuses to pass when a
spelling matches nothing at all.

## Fuzzing and undefined-behavior checks

- **Fuzzing** with libFuzzer and sanitizers on four targets: the
  configuration parser, source syntax, text adapters, and graph semantics,
  where generated graphs are checked against independent path-enumeration and
  reachability oracles.
- **[Miri](https://github.com/rust-lang/miri)** interprets the graph, text,
  reporting, path and concurrency code to catch undefined behavior that normal
  tests cannot see. Unsafe code is forbidden in the project anyway.

## A written contract, with evidence for each rule

The [product contract](../../reference/product-contract/) states thirteen
requirements, from which files belong to which package to how Unicode is
escaped, and names the tests that establish each one. The project follows a
published Rust engineering standard (Rust-OE), and the code was reviewed
against it several times, in separate independent reviews of the CLI,
resolution and documentation. Those reviews found real defects, including cases that reported
clean when they should not have. Each was fixed with a regression test, and
the [review record](https://github.com/ALMaclaine/jscycles/blob/main/docs/maintainers/rust-oe-current-review.md)
lists them.

## The compiler is not allowed to be lenient

- Clippy's `pedantic`, `nursery` and `cargo` groups are errors, not warnings,
  along with 31 more individual lints. 32 compiler lints are `forbid` or
  `deny`, including `unsafe_code`.
- An exception is a narrow `#[expect(...)]` or `#[allow(...)]` on the one item
  concerned, and one without a written reason is itself an error.
- A complexity gate (CRAP score) fails any function that is both complex and
  poorly tested.

## Never a false clean

The most damaging bug a tool like this can have is passing when it should
fail. jscycles treats that as a design rule, not a hope.

- Status `0` means the whole selected graph was built and has no cycle.
  An unreadable file, a syntax error, a bad configuration, an exhausted
  resource limit, or a scan that found no packages at all exits `2`.
- Every scan runs under finite quotas for files, bytes, graph size and output,
  so a huge or hostile checkout fails clearly instead of exhausting memory.
- Output is deterministic, byte for byte, and is fully built before anything is
  written.

## Documentation checked against the binary

AI-written documentation is where slop usually shows. Here:

- Every example output on this site was captured by running the current
  binary, not written from memory. Doing so caught three mistakes in the
  first drafts of these pages, and several rough edges in the tool itself,
  which were fixed rather than documented.
- A test fails if the README's `--help` text differs from the binary's.
- The reference pages are generated from the repository's own documents at
  build time, and the build fails on any broken link or anchor.
- The [benchmarks](../../guides/comparison/#performance) name the exact commit
  of each repository measured and the script that reproduces them.

## Release and supply chain

- Every GitHub Action is pinned to a full commit hash; dependencies are locked
  and checked with cargo-deny.
- Each release binary is built natively for its platform. Its format, CPU,
  version and a known-cycle scan are verified, and its hash is recorded with
  the exact source, toolchain and workflow run.
- npm packages run no install scripts. Releases are published to npm and
  crates.io from CI, with npm provenance, through trusted publishing, so no
  long-lived registry token is kept.

## What an AI assistant made practical

None of these techniques is new. What is unusual is doing all of them on a
small open-source tool, because most of them are tedious enough that people
reasonably skip them:

- reviewing every one of ~1,000 mutants and writing a line-level justification
  for each survivor;
- running a 1,000-project differential test against the TypeScript compiler on
  two operating systems for every change;
- re-running every documented command after every behavior change and
  updating the page when the output differs;
- writing down, in the product contract, behavior most tools leave implicit,
  such as how a symlinked workspace member or a Windows `\\?\` path is treated;
- recording machine-readable evidence (hashes, timings, counts) for each
  claim in [`docs/maintainers/evidence/`](https://github.com/ALMaclaine/jscycles/tree/main/docs/maintainers/evidence).

An assistant does not get bored, so the checking can be as exhaustive as the
problem deserves. It also makes confident mistakes, which is exactly why every
claim above is backed by something a machine checks, not by the assistant's
say-so.

## Where the limits are

Checking is not proof. Fuzzing and differential testing sample the input space;
they do not cover it. The parser is a native tree-sitter grammar that jscycles
trusts. jscycles is static analysis of the files it reads, not a type checker
or a runtime. The known gaps are listed in
[analysis limits](../../reference/analysis-limits/#known-analysis-limits). If
you find a case where jscycles is wrong,
[please report it](https://github.com/ALMaclaine/jscycles/issues): every bug
becomes a regression test.
