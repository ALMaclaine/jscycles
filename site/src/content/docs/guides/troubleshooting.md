---
title: Troubleshooting
description: Common surprises and how to resolve them.
---

## "No packages found."

jscycles analyzes directories whose `package.json` has a `name`. Run it from
the repository root or pass the package directory, and check `scan.include`.

## An alias is not resolved

Look for a `warning: ignored tsconfig` line on stderr; it names the file and
the problem. Otherwise check that the nearest tsconfig at or above the package
defines the alias, or pass `--tsconfig`.

## A cycle only goes through types

Use `--skip-type-imports`, or set `skip_type_imports: true` for the packages
concerned.

## A cycle goes through a barrel file

`export * from './x'` in an `index.ts` imports every re-exported module, so
importing from the barrel inside the same package often closes a loop. Import
the module directly.

## A scan fails with a limit error

Very large repositories can exceed the default resource limits; raise the one
named in the error under `limits` ([analysis
limits](../../reference/analysis-limits/)).

Known gaps are listed in [analysis
limits](../../reference/analysis-limits/#known-analysis-limits).

Still stuck? [Open an issue](https://github.com/ALMaclaine/jscycles/issues)
with a small project that reproduces it.
