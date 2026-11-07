# jscycles

Fast circular dependency detection for JavaScript/TypeScript projects.

A Rust CLI and library for file-level and workspace-package cycle detection.

## Why jscycles?

Circular dependencies in JavaScript/TypeScript projects cause:
- Runtime errors from undefined imports
- Difficult-to-debug initialization order issues
- Webpack/bundler warnings and failed builds
- Increased cognitive load when understanding code flow

jscycles uses Rust, parallel processing, and Tarjan's strongly connected components algorithm. Its import-resolution and output contracts differ from madge; compare results on your own repository before switching.

## Performance

Performance depends on the repository, import graph, hardware, and cache state.
No retained comparison currently supports a specific speedup over madge.
Build with `cargo build --release --locked`, then run
`python3 scripts/benchmark.py` for a reproducible synthetic workload, warm-up,
semantic result checks, and a local JSON timing report. It is an experiment,
not a performance gate or a representative customer workload.

## Installation

### From source

```bash
cargo install --path . --locked
```

## Quick Start

```bash
# Check current directory for cycles
jscycles

# Check specific directory
jscycles path/to/package

# Output as JSON for CI integration
jscycles --json

# Only check specific packages in a monorepo
jscycles --only "@myorg/core-*"

# Show only inter-package cycles (outer cycles)
jscycles --outer

# Show only file-level cycles (inner cycles)
jscycles --inner
```

## Usage

```
Usage: jscycles [OPTIONS] [paths]...

Arguments:
  [paths]...  Paths to check (if empty, uses config scan paths)

Options:
      --only <only>              Only check packages matching glob pattern (can be repeated)
      --exclude <exclude>        Exclude packages matching glob pattern (can be repeated)
      --extensions <extensions>  File extensions to analyze [default: ts,tsx,js,jsx]
      --tsconfig <tsconfig>      Path to tsconfig.json
      --no-tsconfig              Skip tsconfig.json auto-detection
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

Without paths, the configured `scan.include` roots are scanned (default `.`).
Every named `package.json` found is a package, including a named workspace root.
Each file belongs to its nearest package, so a workspace root analyzes only
files outside its members' directories and each cycle is reported once. Members
removed by `--only`/`--exclude` keep their directories, and `scan.exclude`
directories are skipped by every package, not just by package discovery.
A file that fails to parse stops the scan with exit 2 and names its line and
column; `ignore` intentionally broken fixtures.

## Examples

### Basic usage

```bash
# Check a single package
jscycles packages/core

# Check multiple directories
jscycles packages/core packages/utils packages/ui
```

### Monorepo filtering

```bash
# Only check packages matching a pattern
jscycles --only "@myorg/feature-*"

# Exclude test packages
jscycles --exclude "*-test" --exclude "*-e2e"

# Combine filters
jscycles --only "@myorg/*" --exclude "@myorg/legacy-*"
```

### Inter-package cycle detection

jscycles detects two types of cycles:

- **Inner cycles**: File-level circular dependencies within a package
- **Outer cycles**: Package-level circular dependencies between workspace packages

```bash
# Show both cycle types (default)
jscycles

# Show only file-level cycles
jscycles --inner

# Show only package-level cycles
jscycles --outer
```

Workspace packages are automatically detected from:
- npm/yarn/bun: `package.json` workspaces field
- pnpm: `pnpm-workspace.yaml`
- TypeScript: `tsconfig.json` project references

### CI/CD Integration

```bash
# JSON output for parsing
jscycles --json > cycles.json

# Quiet mode - only output if cycles exist (for CI gates)
jscycles --quiet  # 0: clean, 1: cycles, 2: error; preserve the status in CI

# Check specific packages from a file list
cat changed-packages.txt | jscycles --stdin
```

### Custom file extensions

```bash
# Only TypeScript files
jscycles --extensions ts,tsx

# Include additional extensions
jscycles --extensions ts,tsx,js,jsx,mjs,cjs
```

## Configuration

Create a `jscycles.yaml` in your project root for persistent configuration:

```yaml
# Package discovery settings
scan:
  # Glob patterns for directories containing packages
  include:
    - "packages/*"
    - "apps/*"
    - "libs/*"

  # Directories to skip (defaults shown)
  exclude:
    - "**/node_modules"
    - "**/dist"
    - "**/build"

# Default settings for all packages
defaults:
  # File extensions to analyze
  extensions:
    - ts
    - tsx
    - js
    - jsx

# Package-specific overrides (keyed by glob pattern)
packages:
  # Legacy packages might use different extensions
  "@myorg/legacy-*":
    extensions:
      - js
    ignore:
      - "**/*.test.js"

  # Ignore test files in all feature packages
  "@myorg/feature-*":
    ignore:
      - "**/*.test.ts"
      - "**/*.spec.ts"
      - "**/__tests__/**"
      - "**/__mocks__/**"
```

Automatic scans apply `scan.include` as recursive directory roots relative to
the current directory; `.` selects the whole tree. Explicit directory arguments
recurse through that directory and override these automatic includes. An explicit
package.json path selects just that manifest. Other file operands are errors.
`scan.exclude` globs match directory paths relative to the directory being walked,
and package `ignore` globs match file paths relative to the package root (for
example `src/legacy/**` or `**/*.test.ts`). The built-in `node_modules`, `dist`,
`build`, `.next` and `.git` exclusions likewise apply only beneath the scanned
root, so a checkout that itself lives under such a directory is still analyzed.
An explicitly selected directory is always scanned; exclusions prune beneath it.
Include and exclude globs ignore a trailing `/`; absolute globs are compared
with the lexically normalized absolute path, so `..` in operands does not bypass
them. A tsconfig found automatically that fails to load (for example an
`extends` package missing from `node_modules`) is skipped with a stderr warning
naming it, because its path aliases would otherwise silently go unresolved;
pass `--tsconfig` to make such a failure an error.
TypeScript `extends` accepts a single path or an array (applied in order, later
entries overriding earlier ones). A package base in `node_modules` is found the
way TypeScript finds it: through the package's `exports`, its `tsconfig` field,
or its `tsconfig.json`. On Windows, globs may also be spelled with `\`
(`.\src\legacy\`), and absolute globs such as `C:\repo\legacy` match whatever
the drive-letter case. A leading UTF-8 byte-order mark is ignored in every file
and in `--stdin`.
`--outer` filters the same import graph as the normal scan; unused manifest
requirements do not create edges, and undeclared source imports remain visible.

Resource quotas are configured under `limits`; defaults cover entry/file counts,
aggregate input bytes, imports, graph nodes/edges, cycle enumeration, retained
payloads, and staged output. See [analysis limits](docs/analysis-limits.md) for
all defaults, accounting units, library APIs, and failure behavior.

### Configuration precedence

The schema rejects unknown and duplicate YAML keys. A missing default
`jscycles.yaml` uses built-in defaults; an invalid or unreadable file fails.
An explicitly selected `--config` or `--tsconfig` must exist and parse.

1. CLI arguments (highest priority)
2. Package-specific config in `jscycles.yaml`
3. Default config in `jscycles.yaml`
4. Built-in defaults (lowest priority)

## TypeScript Path Aliases

jscycles automatically discovers and uses `tsconfig.json` for path alias resolution:

```json
{
  "compilerOptions": {
    "baseUrl": ".",
    "paths": {
      "@/*": ["src/*"],
      "@components/*": ["src/components/*"],
      "@utils": ["src/utils/index.ts"]
    }
  }
}
```

With this config, imports like `@/services/api` resolve to `src/services/api.ts`.

### tsconfig options

```bash
# Use a specific tsconfig
jscycles --tsconfig tsconfig.build.json

# Disable tsconfig discovery (treat path aliases as external)
jscycles --no-tsconfig
```

### Resolution behavior

1. **Auto-discovery**: Searches upward from each package for `tsconfig.json`
2. **Extension resolution**: Tries `.ts`, `.tsx`, `.js`, `.jsx` in order
3. **Index files**: Resolves `@/utils` to `src/utils/index.ts` if directory
4. **Graceful fallback**: Unresolvable aliases are treated as external modules

## Output Formats

### Human-readable (default)

```
✗ 1 inter-package cycle
  @myorg/core -> @myorg/utils -> @myorg/core

✗ @myorg/core: 2 cycles
  src/components/Button.tsx -> src/hooks/useTheme.ts -> src/components/Button.tsx
  src/services/api.ts -> src/services/auth.ts -> src/services/api.ts

Summary: 3 packages, 1 with file cycles (2 total), 1 inter-package cycles
```

Clean packages (`✓ @myorg/ui: no cycles`) and the source files that create each
inter-package edge are listed only with `-v`. On a TTY, cycles are red and clean
packages green.

### JSON output

```bash
jscycles --json
```

```json
{
  "has_cycles": true,
  "has_file_cycles": true,
  "has_package_cycles": true,
  "file_cycles": [
    {
      "name": "@myorg/core",
      "path": "./packages/core",
      "cycles": [
        [
          "src/components/Button.tsx",
          "src/hooks/useTheme.ts",
          "src/components/Button.tsx"
        ],
        [
          "src/services/api.ts",
          "src/services/auth.ts",
          "src/services/api.ts"
        ]
      ]
    },
    {
      "name": "@myorg/ui",
      "path": "./packages/ui",
      "cycles": []
    },
    {
      "name": "@myorg/utils",
      "path": "./packages/utils",
      "cycles": []
    }
  ],
  "package_cycles": [
    {
      "packages": [
        "@myorg/core",
        "@myorg/utils",
        "@myorg/core"
      ],
      "edges": [
        {
          "from_package": "@myorg/core",
          "to_package": "@myorg/utils",
          "files": []
        },
        {
          "from_package": "@myorg/utils",
          "to_package": "@myorg/core",
          "files": []
        }
      ]
    }
  ]
}
```

Packages are sorted by name. Cycle file paths are relative to their package.
Edge `files` evidence is collected only with `-v`; each entry has an absolute
`path` and, when the import named a package subpath, a `subpath` key (omitted
otherwise). Check the exit status before trusting a report.

## How It Works

### Algorithm

jscycles uses **Tarjan's strongly connected components (SCC)** algorithm:

1. **Parse**: Extract literal imports with one tree-sitter cursor walk per file (WillBooster's JavaScript/TypeScript grammar forks, which accept valid syntax the upstream 0.23 grammars reject)
2. **Build graph**: Create a directed graph of file dependencies
3. **Find SCCs**: Run Tarjan's algorithm to find strongly connected components
4. **Extract cycles**: Report one representative file cycle per cyclic SCC; single nodes are checked for self-loops. Package analysis enumerates elementary cycles.
5. **Normalize**: Cycles are normalized to start from the lexicographically smallest file

### Import extraction

Supported import syntaxes:
- `import x from './module'`
- `import { x } from './module'`
- `import * as x from './module'`
- `import './module'` (side-effect imports)
- `export { x } from './module'`
- `export * from './module'`
- Dynamic `import('./module')`
- `require('./module')` (CommonJS)

### What's considered a dependency

| Import | Classification |
|--------|----------------|
| `./relative/path` | Resolved to file path |
| `../parent/path` | Resolved to file path |
| `@/aliased/path` | Resolved via tsconfig paths |
| `lodash` | External (node_modules) |
| `@myorg/utils` | Workspace package (if in workspace) |
| `@scope/package` | External (node_modules) |

File paths participate in file-level (inner) cycle detection. Workspace package imports participate in package-level (outer) cycle detection. External node_modules are ignored.

## Exit Codes

| Code | Meaning |
|------|---------|
| 0 | No circular dependencies found |
| 1 | Circular dependencies detected |
| 2 | Error occurred (invalid config, parse error, etc.) |

## Library Usage

jscycles can be used as a Rust library:

```rust
use std::{collections::HashMap, path::{Path, PathBuf}};
use jscycles::{
    Config, CycleDetector, DependencyGraph, ImportExtractor,
    PackageDiscovery, TsConfig, Workspace,
};
use jscycles::cycles::PackageCycleDetector;
use jscycles::graph::PackageDependencyGraph;

// Load configuration
let config = Config::try_load_or_default(Path::new("jscycles.yaml"))?;

// Discover workspace for inter-package cycles
let workspace = Workspace::discover(&PathBuf::from("."))?;

// Discover packages
let discovery = PackageDiscovery::new(&config, &[], &[])?;
let packages = discovery.discover(&PathBuf::from("."))?;

// Collect imports from all packages
let mut imports_by_package = HashMap::new();
for package in &packages {
    let tsconfig = TsConfig::discover(&package.path);
    let extractor = ImportExtractor::new(
        vec!["ts".into(), "tsx".into()],
        tsconfig,
    ).with_workspace(workspace.clone());

    let imports = extractor.extract(&package.path)?;

    // File-level cycles
    let graph = DependencyGraph::from_imports(&imports);
    let cycles = CycleDetector::detect(&graph);
    println!("{}: {} file cycles", package.name, cycles.len());

    imports_by_package.insert(package.name.clone(), imports);
}

// Package-level cycles
if workspace.is_some() {
    let pkg_graph = PackageDependencyGraph::from_imports(&imports_by_package);
    let pkg_cycles = PackageCycleDetector::detect(&pkg_graph);
    println!("{} inter-package cycles", pkg_cycles.len());
}
```

## Comparison with madge

| Feature | madge | jscycles |
|---------|-------|----------|
| Language | JavaScript | Rust |
| Parallel processing | No | Yes (rayon) |
| tsconfig paths | Yes | Yes |
| JSON output | Yes | Yes |
| Webpack config | Yes | No |
| Image output | Yes | No |
| Monorepo support | Limited | First-class |
| Inter-package cycles | No | Yes |
| Workspace detection | No | npm/pnpm/TypeScript |

## Related Tools

- **[madge](https://github.com/pahen/madge)** - Popular JavaScript dependency graph and circular dependency detector (see [comparison](#comparison-with-madge) above)
- **[NX](https://nx.dev/features/enforce-module-boundaries)** - Monorepo build system with module boundary enforcement and circular dependency detection via ESLint rules
- **[eslint-plugin-import](https://github.com/import-js/eslint-plugin-import)** - ESLint plugin with `no-cycle` rule for detecting circular dependencies inline during development
- **[dpdm](https://github.com/acrazing/dpdm)** - JavaScript/TypeScript circular dependency detector with tree output
- **[circular-dependency-plugin](https://github.com/aackerman/circular-dependency-plugin)** - Webpack plugin that detects cycles during bundling

## Troubleshooting

### "No packages found"

- Ensure you're running from a directory with `package.json` files
- Check your `scan.include` patterns in `jscycles.yaml`
- Use `--only` to explicitly specify package patterns

### Path aliases not resolving

- Ensure `tsconfig.json` exists and has `compilerOptions.paths`
- Check that `baseUrl` is set correctly
- Use `--tsconfig` to specify an explicit path
- Verify the alias pattern matches (e.g., `@/*` vs `@/`)

### False positives

- Add patterns to `ignore` in package config
- Use `--exclude` to skip problematic packages
- Check for `export * from` re-exports that create apparent cycles

## Development

```bash
# Run workspace format, check, clippy, tests, doctests, dependency policy, and docs
cargo xtask ci
```

Development uses Rust 1.98.1; the library and CLI retain MSRV 1.93.0.
CI tests the five release targets, plus the exact MSRV on Linux. Current stable
and fresh dependencies have a weekly advisory maintenance lane. The optional
`profiling` feature produces local timing reports and is disabled by default.

The [engineering tool record](docs/quality-engineering.md) documents strict Clippy,
coverage-backed CRAP, isolated mutations, Miri and Hotpath commands and results.

Read [AGENTS.md](AGENTS.md), [check contracts](tooling/checks.md), and the
[Rust-OE adoption record](docs/rust-oe-adoption.md). The [full audit](docs/rust-oe-full-audit.md) records applicability, implemented evidence and open obligations; the [earlier repair record](docs/rust-oe-adherence.md) is scoped history. The [guidance index](.rust-oe/INDEX.md)
contains the selected practices and immutable source pins. Guidance updates are
reviewed with `rust-oe plan --diff`, applied with `rust-oe sync`, and verified
without network access with `rust-oe check`.

```bash
cargo xtask install-tools          # cargo-deny 0.19.8
cargo xtask install-tools coverage # optional cargo-llvm-cov 0.9.1 and LLVM tools
cargo xtask coverage               # advisory LCOV; test failures remain failures
python3 -B scripts/check_adoption.py # offline guidance integrity; Python 3.11+
python3 -B -m unittest discover -s scripts -p 'test_*.py'
python3 scripts/verify-controls.py --boundaries --output docs/rust-oe-closure-controls.json
```

Inputs are local checkout data. A scan shares finite quotas across discovery,
metadata, parallel source readers, graphs, cycle search, and encoded output.
The configuration bootstrap is capped at 16 MiB; source/metadata files default to
16 MiB each. Stdin accepts up to 1 MiB and 10000 nonempty UTF-8 paths. Graph
traversal uses explicit stacks. See [analysis limits](docs/analysis-limits.md)
for the defaults and their scope; these are not OS memory/time isolation.
Directory, file, and quota errors stop the scan with status 2. Automatic
TypeScript aliases remain best-effort for ordinary metadata errors; shared quota
failures always propagate. Use `--tsconfig` for strict validation.
Reports are staged under the output quota before writing stdout. A subsequent
stdout write/flush failure, including a closed pipe, exits 2 and can leave partial
output; consumers must check the exit status before accepting a report.

Release workflows run the same required checks before building artifacts.
Publishing library API changes requires the compatibility review recorded in
[the adoption record](docs/rust-oe-adoption.md).

## License

MIT OR Apache-2.0
