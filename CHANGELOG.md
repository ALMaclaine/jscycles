# Changelog

All notable changes to jscycles are listed here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the project uses
[Semantic Versioning](https://semver.org/) (before 1.0, a minor version may
break compatibility). Library and edge-case details for each release are in
the [upgrade notes](docs/migration-0.2.md).

## [Unreleased]

## [0.2.1] - 2026-11-13

No change to analysis or output.

### Changed

- Release publishing to npm and crates.io runs from CI through trusted
  publishing, with no stored registry token. npm packages carry provenance.
- The crate's keywords are `circular-imports` and `dependency-cycles` (the
  0.2.0 keyword was too long for crates.io, so 0.2.0 on crates.io was
  published from a commit that already contained this fix).

## [0.2.0] - 2026-11-12

Many fixes in this release change which cycles are reported. Most reveal real
cycles that 0.1 missed; some remove cycles 0.1 invented. Expect different
results on the same repository, and compare before relying on them in CI.

### Added

- npm distribution: `npm install --save-dev jscycles` installs the prebuilt
  binary for the current platform through optional dependencies, with no
  install script.
- Linux release binaries are statically linked (musl), so they also run on
  Alpine and on distributions with an older glibc.
- `--skip-type-imports` (and `skip_type_imports` in `jscycles.yaml`, globally
  or per package) leaves out imports TypeScript erases: `import type`,
  `export type`, type-position `import()`, and declaration files.
- TypeScript `extends` arrays, and package bases resolved through `exports`,
  the `tsconfig` field or `tsconfig.json`, as TypeScript resolves them.
- A warning on stderr when an automatically found tsconfig cannot be loaded, so
  path aliases no longer go missing silently.
- A JSON Schema for `jscycles.yaml` (`jscycles.schema.json`) for editor
  completion and validation.
- Resource limits under `limits` in `jscycles.yaml`
  ([analysis limits](docs/analysis-limits.md)).
- Syntax errors name the file, line and column.
- Package-cycle evidence (`-v`) shows the full imported specifier, such as
  `(imports @myorg/utils/strings)` instead of `(imports /strings)`.

### Changed

- `.mts`, `.cts`, `.mjs` and `.cjs` files are scanned by default (the default
  extensions are now `ts,tsx,mts,cts,js,jsx,mjs,cjs`).
- Each file belongs to its nearest package. A workspace root that is itself a
  package no longer repeats its members' cycles.
- Resolution follows TypeScript's order, including declaration files and
  `.jsx` specifiers that prefer `.tsx`.
- The parser is now WillBooster's tree-sitter grammar forks, which accept valid
  syntax the upstream grammars rejected. A file with syntax errors stops the
  scan with exit status 2 rather than being analyzed partially.
- Glob spellings such as `./x`, `x/` and absolute paths mean the same thing in
  `scan.include`, `scan.exclude` and `ignore`; on Windows `.\x\` too.
- Exclusions apply below the scanned directory only, so a checkout inside a
  `build/` or `node_modules/` directory (common in CI) is no longer skipped
  and reported clean.
- Configuration, source and filesystem errors exit 2 instead of being ignored.
- A scan that selects no package (no `package.json` with a `name` under the
  scanned paths, or none matching `--only`/`--exclude`) exits 2 instead of
  printing "No packages found." and exiting 0, so a CI job started in the
  wrong directory no longer passes.
- An import that resolves to a file in another workspace member, through a
  relative path (`../../b/src/x`) or a wildcard tsconfig alias, is a
  dependency on that member, as if imported by name. Cycles made of such
  imports were previously not reported. `--outer` now resolves imports to find
  them, so it reads tsconfig files too.
- The human summary uses singular forms ("1 inter-package cycle", "1
  package"), and `--inner` uses the same wording as the default report.
- Minimum supported Rust version is 1.93.0.

### Fixed

- A UTF-8 byte-order mark in `package.json`, a tsconfig, `jscycles.yaml`, a
  source file or `--stdin` no longer fails the scan.
- Directory specifiers (`./dir/`, `.`, `..`) resolve only to the directory's
  index file; a sibling `dir.ts` no longer creates invented cycles.
- File symlinks are analyzed once, by the package that owns their target.
- Workspace globs and include roots spelled with `./` no longer match nothing.
- Windows: a `paths` target ending in `/` (`"@x/*": ["src/*/"]`) resolves the
  directory's index, not a sibling `<dir>.ts`.
- Windows: cycle members outside their package are shown as `C:\...`, not
  with the internal `\\?\` prefix.
- A `jscycles.yaml` parse error is shown on one line with its position and
  reason, instead of a source excerpt with literal `\n` sequences.

## [0.1.1] - 2026-03-26

- Faster workspace scanning and smaller, faster release builds.

## [0.1.0] - 2026-03-25

Initial release.

[Unreleased]: https://github.com/ALMaclaine/jscycles/compare/v0.2.1...HEAD
[0.2.1]: https://github.com/ALMaclaine/jscycles/compare/v0.2.0...v0.2.1
[0.2.0]: https://github.com/ALMaclaine/jscycles/compare/v0.1.1...v0.2.0
[0.1.1]: https://github.com/ALMaclaine/jscycles/compare/v0.1.0...v0.1.1
[0.1.0]: https://github.com/ALMaclaine/jscycles/releases/tag/v0.1.0
