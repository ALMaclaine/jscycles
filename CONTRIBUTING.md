# Contributing to jscycles

Thanks for helping. Bug reports, small reproducing projects, fixes and
documentation improvements are all welcome. By contributing you agree that
your work is licensed under the project's terms (MIT OR Apache-2.0), and to
follow the [code of conduct](CODE_OF_CONDUCT.md).

## Reporting a bug

The most useful report is a tiny project that shows the problem: a few files,
their `package.json`/`tsconfig.json`, the command you ran, what jscycles printed
and what you expected. Include `jscycles --version` and your platform. If
another tool (TypeScript, madge, your bundler) resolves an import differently,
say which and how. Security problems go through [SECURITY.md](SECURITY.md)
instead.

## Building and testing

The repository pins its development toolchain in `rust-toolchain.toml`
(rustup installs it automatically); the published crate supports Rust 1.93.0
and newer.

```sh
cargo build
cargo test
cargo xtask ci          # everything CI runs on each pull request
```

`cargo xtask ci` runs formatting, `cargo check`, strict Clippy, all tests and
doctests, cargo-deny and rustdoc. Install cargo-deny once with
`cargo xtask install-tools`. Run `cargo xtask` for the other tasks (coverage,
Miri, mutation testing).

A few repository checks are Python scripts (Python 3.11 or newer):

```sh
python3 -B scripts/check_adoption.py
python3 -B -m unittest discover -s scripts -p 'test_*.py'
python3 -B scripts/verify-controls.py
```

### Differential testing

Changes to discovery, ownership or import resolution should also pass the
generated differential tests. `scripts/differential/difftest.py` creates
random projects (workspaces, nested packages, aliases, tsconfig chains,
excludes, symlinks, many path spellings) and compares jscycles with a
reference model that resolves imports with TypeScript's own resolver. It needs
Node.js and installs TypeScript under `target/tooling/` on first use.

```sh
cargo build
python3 scripts/differential/difftest.py --cases 2000
python3 scripts/differential/difftest.py --replay SEED --into /tmp/case   # rebuild one case
```

A disagreement is shrunk to a minimal project under
`target/tooling/differential/failures/`. Fix jscycles, or, if the model is
wrong, change the model and cite the rule in
[docs/product-contract.md](docs/product-contract.md) it implements.
[tooling/checks.md](tooling/checks.md) describes all checks and the bar for
behavior changes.

## Making a change

- Keep pull requests focused, and add a test that fails without the change.
  Integration tests live in `tests/`; small fixture projects can be written by
  the test itself (see `tests/adherence.rs`).
- Exit statuses are part of the interface: 0 clean, 1 cycles found, 2 error. A
  scan that could not read or parse its input must never report clean.
- JSON output is stable within a minor version: add fields, do not rename or
  remove them.
- Add an entry under `Unreleased` in [CHANGELOG.md](CHANGELOG.md) for anything
  a user would notice.

## Repository layout

| Path | Contents |
| --- | --- |
| `src/` | Library and CLI (`main.rs`) |
| `tests/` | Integration tests and fixtures |
| `fuzz/` | cargo-fuzz targets |
| `npm/` | The Node launcher of the `jscycles` npm package (`scripts/npm_packages.py` builds the packages) |
| `scripts/` | Repository checks, release tooling, benchmarks, differential tests |
| `xtask/` | `cargo xtask` tasks |
| `docs/` | User documentation |
| `docs/maintainers/` | Architecture, threat model, release runbook, quality records |

The project follows a pinned set of Rust engineering practices ("Rust-OE")
kept in `.rust-oe/` and summarized in [AGENTS.md](AGENTS.md), which is also the
entry point for coding agents. You do not need the Rust-OE tool to contribute;
`scripts/check_adoption.py` only verifies that those files are intact.
