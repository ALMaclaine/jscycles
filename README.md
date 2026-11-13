<h1>
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="https://raw.githubusercontent.com/ALMaclaine/jscycles/main/site/src/assets/wordmark-dark.svg">
    <img src="https://raw.githubusercontent.com/ALMaclaine/jscycles/main/site/src/assets/wordmark-light.svg" alt="jscycles" width="240">
  </picture>
</h1>

[![CI](https://github.com/ALMaclaine/jscycles/actions/workflows/ci.yml/badge.svg)](https://github.com/ALMaclaine/jscycles/actions/workflows/ci.yml)
[![crates.io](https://img.shields.io/crates/v/jscycles.svg)](https://crates.io/crates/jscycles)
[![License: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](#license)

Find circular imports in JavaScript and TypeScript projects, between files
within a package and between the packages of a monorepo.

jscycles is a single native binary. It parses every file with tree-sitter,
resolves imports the way TypeScript does (path aliases, `extends`, declaration
files, `.js` specifiers that point at `.ts` sources), and understands npm,
yarn, pnpm and bun workspaces. On the public repositories measured below it
runs 5 to 25 times faster than madge and dpdm.

**Documentation: [jscycles.dev](https://jscycles.dev/)**

Built with an AI coding assistant and checked hard: see
[Is this AI slop?](https://jscycles.dev/project/is-this-ai-slop/)

```text
$ jscycles packages
✓ No inter-package cycles

✗ @ts-morph/common: 2 cycles
  src/collections/SortedKeyValueArray.ts -> src/utils/index.ts -> … -> src/collections/SortedKeyValueArray.ts
  src/typescript/index.ts -> src/typescript/tsInternal.ts -> src/typescript/index.ts

✗ ts-morph: 3 cycles
  …
  src/Project.ts -> src/ProjectContext.ts -> src/Project.ts

Summary: 3 packages, 2 with file cycles (5 total), 0 inter-package cycles
```

## Install

```sh
npm install --save-dev jscycles   # or pnpm, yarn, bun
npx jscycles
```

The npm package runs a prebuilt native binary for Linux (x86-64, ARM64;
static, so Alpine too), macOS (Intel, Apple Silicon) or Windows (x86-64),
chosen through optional dependencies; nothing runs at install time. The same
binaries are attached to each
[GitHub release](https://github.com/ALMaclaine/jscycles/releases) with a
`SHA256SUMS` file, and `cargo install jscycles --locked` builds it from source
(Rust 1.93 or newer). See [install](https://jscycles.dev/guides/install/).

## Quick start

Run it at the root of a repository:

```sh
jscycles                      # every package under the current directory
jscycles packages/core        # one directory
jscycles --only "@myorg/*"    # packages whose name matches a glob
jscycles --inner              # file cycles only
jscycles --outer              # package cycles only
jscycles --skip-type-imports  # leave out imports that only carry types
jscycles --json > cycles.json # machine-readable report
```

The exit status is `0` when no cycle is found, `1` when cycles are found, and
`2` on any error (bad configuration, unreadable files, syntax errors, resource
limits). A failed scan never reports clean, so in CI a single step is enough:

```yaml
- run: npx jscycles
```

## Documentation

The full documentation is at **[jscycles.dev](https://jscycles.dev/)**:

- [Why circular imports matter](https://jscycles.dev/introduction/) and
  [how jscycles works](https://jscycles.dev/concepts/how-it-works/)
- Guides: [quick start](https://jscycles.dev/guides/quick-start/),
  [fixing cycles](https://jscycles.dev/guides/fixing-cycles/),
  [adopting it on an existing codebase](https://jscycles.dev/guides/adopting/),
  [CI and git hooks](https://jscycles.dev/guides/ci/),
  [monorepos](https://jscycles.dev/guides/monorepos/),
  [TypeScript](https://jscycles.dev/guides/typescript/),
  [scripting with JSON](https://jscycles.dev/guides/scripting/),
  [embedding in Rust](https://jscycles.dev/guides/rust-library/),
  [migrating from madge or dpdm](https://jscycles.dev/guides/migrating/),
  [troubleshooting](https://jscycles.dev/guides/troubleshooting/)
- Reference: [configuration](docs/configuration.md),
  [JSON output](https://jscycles.dev/reference/json-output/),
  [package cycles](docs/package-cycles.md),
  [analysis limits](docs/analysis-limits.md),
  [product contract](docs/product-contract.md),
  [upgrading to 0.2](docs/migration-0.2.md), [changelog](CHANGELOG.md)

## Command line

```text
Usage: jscycles [OPTIONS] [paths]...

Arguments:
  [paths]...  Paths to check (if empty, uses config scan paths)

Options:
      --only <only>              Only check packages matching glob pattern (can be repeated)
      --exclude <exclude>        Exclude packages matching glob pattern (can be repeated)
      --extensions <extensions>  File extensions to analyze [default: ts,tsx,mts,cts,js,jsx,mjs,cjs]
      --tsconfig <tsconfig>      Path to tsconfig.json
      --no-tsconfig              Skip tsconfig.json auto-detection
      --skip-type-imports        Ignore type-only imports (import type, export type, declaration files)
      --json                     Output as JSON
      --stdin                    Read paths from stdin
  -c, --config <config>          Path to config file [default: jscycles.yaml]
  -q, --quiet                    Only output if cycles found
      --inner                    Show only file-level (inner) cycles
      --outer                    Show only package-level (outer) cycles
  -v, --verbose                  Show all packages (including clean) and file-level details
  -h, --help                     Print help
  -V, --version                  Print version
```

## Performance

Median of five runs after one warm-up, on an Apple M3 Pro (12 cores) running
macOS, analyzing each repository's `packages` directory with every source
extension enabled:

| Repository | Source files | jscycles 0.2.0 | madge 8.0.0 | dpdm 3.14.0 |
| --- | ---: | ---: | ---: | ---: |
| [nest](https://github.com/nestjs/nest/tree/ed90b8357b40c6e94a466f17f0e6ade88cf80048) | 1,068 | 0.08 s | 2.04 s | 0.86 s |
| [typeorm](https://github.com/typeorm/typeorm/tree/5a572a40bae2f85a1d5f7b87a0aa88bbbaec5ab3) | 3,609 | 0.28 s | 2.97 s | 1.48 s |
| [excalidraw](https://github.com/excalidraw/excalidraw/tree/701b901a0f1ba3d0b390d9cdf63d112e25abac4b) | 631 | 0.11 s | 2.63 s | 0.87 s |
| [ts-morph](https://github.com/dsherret/ts-morph/tree/f288183ddb496adc6f4c5b6929830b5b73437185) | 1,171 | 0.08 s | 1.64 s | 0.83 s |

The tools count cycles differently (see
[how it works](https://jscycles.dev/concepts/how-it-works/)), so their cycle
counts are not comparable. Reproduce with
`python3 scripts/compare_tools.py`; results depend on hardware and file cache.

## Library

jscycles is also a Rust library. `Scan` runs the same analysis as the command
and returns the results as data:

```rust
let report = jscycles::Scan::new("path/to/repo").run()?;
for cycle in &report.results().package_cycles {
    println!("{}", cycle.packages.join(" -> "));
}
```

Depend on it with `jscycles = { version = "0.4", default-features = false }`
to leave out the command-line binary. See
[embedding in Rust](https://jscycles.dev/guides/rust-library/) and the API
reference on [docs.rs](https://docs.rs/jscycles).

## Contributing

The most useful contribution is a bug report with a small project that
reproduces it; see [CONTRIBUTING.md](CONTRIBUTING.md) for building, testing
and the differential test harness. Please report security issues privately,
as described in [SECURITY.md](SECURITY.md).

## License

Licensed under either of [Apache License 2.0](LICENSE-APACHE) or
[MIT license](LICENSE-MIT) at your option. Contributions are accepted under
the same terms.
