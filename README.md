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

**Documentation: [jscycles.dev](https://jscycles.dev/)**

jscycles is a single native binary. It parses every file with tree-sitter,
resolves imports the way TypeScript does (path aliases, `extends`, declaration
files, `.js` specifiers that point at `.ts` sources), and understands npm,
yarn, pnpm and bun workspaces. On the public repositories measured below it
runs 5 to 25 times faster than madge and dpdm.

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

With npm (or pnpm, yarn, bun), as a development dependency:

```sh
npm install --save-dev jscycles
npx jscycles
```

The package runs a prebuilt native binary; npm picks the one for your
platform from optional dependencies, and nothing runs at install time. Builds
exist for Linux (x86-64, ARM64; static, so any distribution including
Alpine), macOS (Intel, Apple Silicon) and Windows (x86-64). If optional
dependencies are disabled (`--omit=optional`), the binary is missing and
`jscycles` exits with status 2 saying so.

The same binaries are attached to each
[GitHub release](https://github.com/ALMaclaine/jscycles/releases), together
with a `SHA256SUMS` file.

With a Rust toolchain (1.93 or newer):

```sh
cargo install jscycles --locked
```

From a checkout: `cargo install --path . --locked`.

## Quick start

Run it at the root of a repository:

```sh
jscycles                      # every package under the current directory
jscycles packages/core        # one directory
jscycles --only "@myorg/*"    # packages whose name matches a glob
jscycles --inner              # file cycles only
jscycles --outer              # package cycles only
jscycles --json > cycles.json # machine-readable report
```

The exit status is `0` when no cycle is found, `1` when cycles are found, and
`2` on any error (bad configuration, unreadable files, syntax errors). A
failed scan never reports clean.

## What it finds

**File cycles.** Within each package, jscycles follows relative imports and
tsconfig path aliases between files. For each group of files that import each
other in a loop (a strongly connected component), it reports one
representative cycle. Fix it and run again to see what remains. madge and
dpdm list every overlapping loop instead, so their counts are much larger.

**Package cycles.** In a workspace, importing another member by name
(`import { x } from '@myorg/utils'`), or through a tsconfig alias that points
into a member, is a dependency between packages. Every elementary cycle
between packages is reported. Dependencies declared in `package.json` but
never imported are not edges.

**What counts as an import:**

- `import … from`, `import '…'`, `export … from`, `export * from`
- `import('…')` and `require('…')` with a string literal (computed specifiers
  cannot be resolved and are skipped)
- `import x = require('…')`
- type-only imports: `import type`, `export type` and `import()` in types.
  The type checker sees these, but they are erased at runtime; pass
  `--skip-type-imports` to leave them out (see
  [type-only imports](docs/configuration.md#type-only-imports)).

Imports of packages that are not workspace members (anything from
`node_modules`) and imports that resolve to no file are ignored.

**Which files belong to which package.** Every `package.json` with a `name`
is a package, including a workspace root. Each file belongs to its nearest
package, so nested packages are analyzed separately and each cycle is
reported once. `node_modules`, `dist`, `build`, `.next` and `.git`
directories are skipped. By default `.ts`, `.tsx`, `.mts`, `.cts`, `.js`,
`.jsx`, `.mjs` and `.cjs` files are analyzed.

**Syntax errors stop the scan.** A file jscycles cannot parse would leave a
hole in the graph, so the scan fails and names the file, line and column. Use
[`ignore`](docs/configuration.md#defaults-and-packages) for intentionally
broken fixtures.

## Use in CI

jscycles fails the build when it finds cycles (status 1) or cannot complete
the scan (status 2). For GitHub Actions:

```yaml
- name: Check for circular imports
  run: |
    version=v0.2.0
    base=https://github.com/ALMaclaine/jscycles/releases/download/$version
    curl -sSfLO "$base/jscycles-linux-x86_64"
    curl -sSfLO "$base/SHA256SUMS"
    sha256sum --check --ignore-missing SHA256SUMS
    chmod +x jscycles-linux-x86_64
    ./jscycles-linux-x86_64
```

To adopt it in a codebase that already has cycles, start with `--only` or
`--exclude` for the packages that are clean, or with `--outer` for package
cycles only, and widen the check as cycles are fixed.

With [pre-commit](https://pre-commit.com/), using a binary on `PATH`:

```yaml
repos:
  - repo: local
    hooks:
      - id: jscycles
        name: jscycles
        entry: jscycles --quiet
        language: system
        pass_filenames: false
        types_or: [javascript, jsx, ts, tsx]
```

## Configuration

Options can live in a `jscycles.yaml` at the repository root:

```yaml
scan:
  include: ["packages/*", "apps/*"]   # where to look for packages
  exclude: ["**/node_modules", "**/fixtures"]

defaults:
  skip_type_imports: true

packages:
  "@myorg/legacy-*":
    extensions: [js]
    ignore: ["**/*.test.js"]
```

The [configuration reference](docs/configuration.md) lists every key, the
precedence between the command line and the file, and the resource
[limits](docs/analysis-limits.md).

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

## TypeScript and workspaces

- **Path aliases.** The nearest `tsconfig.json` at or above each package
  supplies `baseUrl` and `paths`. `extends` is followed through relative
  files, packages in `node_modules` (their `exports`, `tsconfig` field or
  `tsconfig.json`) and arrays, as TypeScript resolves them. A tsconfig that
  cannot be loaded is skipped with a warning naming it. `--tsconfig FILE` uses
  one file everywhere and makes problems with it errors; `--no-tsconfig`
  disables aliases.
- **Resolution.** Extensionless specifiers try `.ts`, `.tsx`, `.d.ts`, `.js`,
  `.jsx`, then a directory's `index` file. `.js`, `.jsx`, `.mjs` and `.cjs`
  specifiers find their TypeScript sources. Directory specifiers (`./dir/`,
  `.`, `..`) resolve only to the index file.
- **Workspaces.** Members come from `package.json` `workspaces` (npm, yarn,
  bun), `pnpm-workspace.yaml`, or tsconfig project `references`. Member names
  must be unique.

## Output

The human report lists packages with cycles. `-v` also lists clean packages
and, for each package cycle, the files that create each edge. Colors are used
only when stdout is a terminal.

`--json` prints:

```json
{
  "has_cycles": true,
  "has_file_cycles": true,
  "has_package_cycles": true,
  "file_cycles": [
    {
      "name": "@myorg/core",
      "path": "./packages/core",
      "cycles": [["src/a.ts", "src/b.ts", "src/a.ts"]]
    }
  ],
  "package_cycles": [
    {
      "packages": ["@myorg/core", "@myorg/utils", "@myorg/core"],
      "edges": [
        { "from_package": "@myorg/core", "to_package": "@myorg/utils", "files": [] },
        { "from_package": "@myorg/utils", "to_package": "@myorg/core", "files": [] }
      ]
    }
  ]
}
```

Packages are sorted by name. Each cycle starts and ends at its
lexicographically smallest file, and file paths are relative to the package.
Edge `files` is filled only with `-v`: an absolute `path` and, when the import
named a subpath (`@myorg/utils/fmt`), a `subpath`. The same input always gives
the same output.

**Stability.** Within a minor version (0.2.x), existing JSON fields keep their
names, types and meaning. New fields may be added, so ignore keys you do not
recognize. Changes to existing fields come only with a new minor version
before 1.0 and are listed in the [changelog](CHANGELOG.md). Always check the
exit status before trusting a report.

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

The tools count cycles differently (see [what it finds](#what-it-finds)), so
their cycle counts are not comparable. Reproduce with
`python3 scripts/compare_tools.py`; results depend on hardware and file cache.

## Comparison with other tools

- **[madge](https://github.com/pahen/madge)** draws dependency graphs as
  images, reads webpack configuration, and lists every cycle. jscycles does
  none of these. It does report package cycles in monorepos, follows tsconfig
  `extends` chains and workspace aliases, and fails rather than reporting
  clean when a file cannot be parsed.
- **[dpdm](https://github.com/acrazing/dpdm)** prints dependency trees from
  given entry files and can skip type dependencies (`-T`).
- **[eslint-plugin-import `no-cycle`](https://github.com/import-js/eslint-plugin-import/blob/main/docs/rules/no-cycle.md)**
  reports cycles inline in the editor, at a higher cost per lint run.
- **[Nx module boundaries](https://nx.dev/features/enforce-module-boundaries)**
  enforces dependency rules between projects in Nx workspaces.

Results differ between tools because their resolution rules differ. Compare
them on your own repository before switching.

## Troubleshooting

**"No packages found."** jscycles analyzes directories whose `package.json`
has a `name`. Run it from the repository root or pass the package directory,
and check `scan.include`.

**An alias is not resolved.** Look for a `warning: ignored tsconfig` line on
stderr; it names the file and the problem. Otherwise check that the nearest
tsconfig at or above the package defines the alias, or pass `--tsconfig`.

**A cycle only goes through types.** Use `--skip-type-imports`, or set
`skip_type_imports: true` for the packages concerned.

**A cycle goes through a barrel file.** `export * from './x'` in an `index.ts`
imports every re-exported module, so importing from the barrel inside the
same package often closes a loop. Import the module directly.

**A scan fails with a limit error.** Very large repositories can exceed the
default resource limits; raise the one named in the error under `limits`
([analysis limits](docs/analysis-limits.md)).

Known gaps are listed in
[analysis limits](docs/analysis-limits.md#known-analysis-limits).

## Library

jscycles is also a Rust library ([docs.rs](https://docs.rs/jscycles)): package
discovery, import extraction, resolution and cycle detection are public. The
command-line tool in [`src/main.rs`](src/main.rs) shows how to combine them.
In particular, pass the boundaries from
`PackageDiscovery::discover_with_boundaries` to
`ImportExtractor::with_nested_packages` and `with_owners`, so each file is
analyzed by its nearest package. Library changes for 0.2 are in the
[upgrade notes](docs/migration-0.2.md).

## Contributing

The most useful contribution is a bug report with a small project that
reproduces it; see [CONTRIBUTING.md](CONTRIBUTING.md) for building, testing
and the differential test harness. Please report security issues privately,
as described in [SECURITY.md](SECURITY.md).

## License

Licensed under either of [Apache License 2.0](LICENSE-APACHE) or
[MIT license](LICENSE-MIT) at your option. Contributions are accepted under
the same terms.
