---
title: TypeScript projects
description: Path aliases, extends chains, module resolution and type-only imports.
---

jscycles resolves imports the way the TypeScript compiler does, so a cycle
through a path alias or a `.js` specifier that points at a `.ts` file is
found like any other.

## Path aliases

Each package uses the nearest `tsconfig.json` at or above its directory, and
applies its `baseUrl` and `paths`:

```json title="tsconfig.json"
{
  "compilerOptions": {
    "baseUrl": ".",
    "paths": {
      "@/*": ["src/*"],
      "@acme/core": ["packages/core/src/index.ts"]
    }
  }
}
```

`extends` is followed as TypeScript follows it: relative files, packages in
`node_modules` (through their `exports`, `tsconfig` field or `tsconfig.json`),
and arrays of either, where later entries win. A package's own `paths`
replace inherited ones.

A tsconfig found this way that cannot be loaded is skipped, with a warning on
stderr naming it:

```text
warning: ignored tsconfig "/repo/packages/web/tsconfig.json": "…: key must be a string at line 1 column 24"; its path aliases were not applied (fix it, pass --tsconfig to require it, or --no-tsconfig)
```

Imports through its aliases are then not followed, so cycles through them can
be missed: treat the warning as something to fix. (When jscycles has to read a
`tsconfig.json` to find the workspace itself, through project `references`, a
file it cannot parse fails the scan instead.)

### Choosing the tsconfig

| Option | Effect |
| --- | --- |
| *(none)* | Nearest `tsconfig.json` at or above each package. |
| `--tsconfig tsconfig.base.json` | This file for every package. Any problem with it is an error. |
| `--no-tsconfig` | No aliases. Only relative imports and workspace package names are followed. |

A `tsconfig.json` nested inside a package, for example in `src/`, is not
consulted. If your aliases live there, pass it with `--tsconfig`.

## How a specifier finds its file

Relative imports and aliases try these files, in order:

| Specifier | Tried, in order |
| --- | --- |
| `./user` | `user.ts`, `user.tsx`, `user.d.ts`, `user.js`, `user.jsx`, then `user/index` with the same extensions |
| `./user.js` | `user.js` if it exists, otherwise `user.ts`, then `user.tsx` (the ESM convention of importing the compiled name) |
| `./user.mjs`, `./user.cjs` | the same, within the `.mts`/`.mjs` or `.cts`/`.cjs` family |
| `./dir/`, `.`, `..` | the directory's `index` file only |

Imports of packages that are not workspace members, such as anything from
`node_modules`, are not followed; neither are imports that resolve to no file.

## Type-only imports

By default every import counts, including `import type`, because a loop of
types is still a loop for the compiler and for readers. When you care only
about cycles that exist at runtime, leave out what TypeScript erases:

```sh
jscycles --skip-type-imports
```

or, for all or some packages:

```yaml title="jscycles.yaml"
defaults:
  skip_type_imports: true
```

This leaves out `import type`, `export type`, `import()` in type positions
(`type T = import('./a').T`) and every import in a `.d.ts` file. `import
{ type A } from './a'` still counts, because it can still load `./a` at
runtime. Details are in
[type-only imports](../../reference/configuration/#type-only-imports).

## File extensions

`.ts`, `.tsx`, `.mts`, `.cts`, `.js`, `.jsx`, `.mjs` and `.cjs` files are
analyzed by default. To check only TypeScript sources:

```sh
jscycles --extensions ts,tsx,mts,cts
```

jscycles does not read `.vue`, `.svelte` or `.astro` files, and does not
support Flow.
