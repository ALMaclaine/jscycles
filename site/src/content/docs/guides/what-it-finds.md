---
title: What it finds
description: File cycles, package cycles, and what jscycles counts as an import.
---

## File cycles

Within each package, jscycles follows relative imports and tsconfig path
aliases between files. For each group of files that import each other in a
loop (a strongly connected component), it reports one representative cycle.
Fix it and run again to see what remains. madge and dpdm list every
overlapping loop instead, so their counts are much larger.

## Package cycles

In a workspace, importing another member by name
(`import { x } from '@myorg/utils'`), or through a tsconfig alias that points
into a member, is a dependency between packages. Every elementary cycle
between packages is reported. Dependencies declared in `package.json` but
never imported are not edges. The [package cycles
reference](../../reference/package-cycles/) has the details.

## What counts as an import

- `import … from`, `import '…'`, `export … from`, `export * from`
- `import('…')` and `require('…')` with a string literal (computed specifiers
  cannot be resolved and are skipped)
- `import x = require('…')`
- type-only imports: `import type`, `export type` and `import()` in types.
  The type checker sees these, but they are erased at runtime; pass
  `--skip-type-imports` to leave them out (see
  [type-only imports](../../reference/configuration/#type-only-imports)).

Imports of packages that are not workspace members (anything from
`node_modules`) and imports that resolve to no file are ignored.

## Which files belong to which package

Every `package.json` with a `name` is a package, including a workspace root.
Each file belongs to its nearest package, so nested packages are analyzed
separately and each cycle is reported once. `node_modules`, `dist`, `build`,
`.next` and `.git` directories are skipped. By default `.ts`, `.tsx`, `.mts`,
`.cts`, `.js`, `.jsx`, `.mjs` and `.cjs` files are analyzed.

## Syntax errors stop the scan

A file jscycles cannot parse would leave a hole in the graph, so the scan
fails and names the file, line and column. Use
[`ignore`](../../reference/configuration/#defaults-and-packages) for
intentionally broken fixtures.
