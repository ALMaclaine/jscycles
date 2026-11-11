---
title: TypeScript and workspaces
description: How jscycles reads tsconfig files, resolves specifiers and finds workspace members.
---

## Path aliases

The nearest `tsconfig.json` at or above each package supplies `baseUrl` and
`paths`. `extends` is followed through relative files, packages in
`node_modules` (their `exports`, `tsconfig` field or `tsconfig.json`) and
arrays, as TypeScript resolves them. A tsconfig that cannot be loaded is
skipped with a warning naming it. `--tsconfig FILE` uses one file everywhere
and makes problems with it errors; `--no-tsconfig` disables aliases.

## Resolution

Extensionless specifiers try `.ts`, `.tsx`, `.d.ts`, `.js`, `.jsx`, then a
directory's `index` file. `.js`, `.jsx`, `.mjs` and `.cjs` specifiers find
their TypeScript sources. Directory specifiers (`./dir/`, `.`, `..`) resolve
only to the index file.

## Workspaces

Members come from `package.json` `workspaces` (npm, yarn, bun),
`pnpm-workspace.yaml`, or tsconfig project `references`. Member names must be
unique. See [package cycles](../../reference/package-cycles/) for how
workspaces are found and what creates a dependency between packages.
