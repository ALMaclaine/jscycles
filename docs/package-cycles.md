# Package cycles

In a monorepo, a package cycle is a loop of workspace packages that import
each other: `@myorg/core` imports `@myorg/utils`, which imports
`@myorg/core`. Package managers and bundlers usually tolerate these, but they
make build order ambiguous, defeat incremental builds, and often hide the
same runtime initialization problems as file cycles.

jscycles reports package cycles alongside file cycles. `--outer` shows only
package cycles; `--inner` only file cycles.

## Finding the workspace

jscycles looks upward from the scanned directory for the workspace root, in
this order:

1. `pnpm-workspace.yaml` (its `packages` globs)
2. `package.json` with `workspaces`, as an array of globs or
   `{ "packages": [...] }` (npm, yarn, bun)
3. `tsconfig.json` with project `references`

Globs follow the package managers' rules: `*` matches one directory level,
`**` any number, and `!pattern` excludes. Each member needs a `package.json`
with a `name`, and names must be unique; a duplicate is an error rather than
a guess. Without a workspace, there are no package cycles to report.

## What creates an edge

An edge from package A to package B comes from a file in A that imports B:

- by name: `import { x } from '@myorg/b'`, including subpaths such as
  `@myorg/b/format` and `require('@myorg/b')`;
- through a tsconfig path alias whose target lies inside B, such as
  `"@b": ["packages/b/src/index.ts"]` or `"@b/*": ["packages/b/src/*"]`.

These do not create edges:

- `dependencies` in `package.json` that no file imports;
- a package importing itself by name (that import resolves to its own files);
- relative imports that reach into another package's directory
  (`../../b/src/x`). These are not package edges and are not followed across
  packages, so a cycle made only of them is not reported. Import other
  packages by name.

With `--skip-type-imports`, `import type` and `export type` between packages
are left out too.

## Reading the report

```text
✗ 1 inter-package cycle

  @myorg/core -> @myorg/utils -> @myorg/core
```

Every elementary cycle is listed, each starting from its alphabetically first
package. Add `-v` to see which files create each edge (paths are absolute):

```text
  @myorg/core -> @myorg/utils -> @myorg/core
    @myorg/core -> @myorg/utils
      - /repo/packages/core/src/format.ts (imports @myorg/utils/strings)
    @myorg/utils -> @myorg/core
      - /repo/packages/utils/src/index.ts
```

In `--json` output the same evidence is in each edge's `files`, with the
imported subpath when there is one.

## Breaking a package cycle

Common fixes:

- move the shared code both packages need into a third package that imports
  neither;
- invert one dependency, for example by passing a callback or registering an
  implementation instead of importing it;
- merge the packages if they are really one unit.

To introduce the check gradually, run `jscycles --outer` in CI first: package
cycles are usually fewer than file cycles and matter most for build tooling.
