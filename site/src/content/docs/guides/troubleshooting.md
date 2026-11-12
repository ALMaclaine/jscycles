---
title: Troubleshooting and FAQ
description: Common surprises, error messages, and answers to frequent questions.
---

## Nothing is found

### "No packages found."

```text
No packages found.
Summary: 0 packages, 0 with file cycles (0 total), 0 inter-package cycles
```

jscycles analyzes directories whose `package.json` has a `name`. This run
checked nothing, yet exits with status `0`, so make sure CI runs it from the
right directory. Run it from the repository root, pass the package directory,
or check [`scan.include`](../../reference/configuration/#scan). A
`package.json` without `name` (common at a workspace root) is not a package,
but its workspace members still are.

### A cycle you expected is not reported

Work through these in order:

1. **Is the file analyzed?** Its extension must be in `--extensions` (all
   JavaScript and TypeScript extensions by default), and it must not be under
   `node_modules`, `dist`, `build`, `.next`, `.git`, a `scan.exclude` pattern
   or an `ignore` pattern.
2. **Does the import resolve?** An alias needs a `tsconfig.json` at or above
   the package that defines it. Look for a `warning: ignored tsconfig` line
   on stderr. A specifier computed at runtime (`require(name)`) cannot be
   followed.
3. **Are both ends in the scan?** Package cycles are found among the selected
   packages only, so `jscycles packages/a` cannot see a loop through
   `packages/b`.
4. **Is it a relative import into another package?** `../../b/src/x` from
   package `a` is not followed across packages. Import other packages by name.
5. **Is a package a symlink?** Directory symlinks are not followed, so a
   workspace member that is a symlinked directory is not a member.
6. **Is it only types?** With `--skip-type-imports`, loops made only of
   `import type` are left out on purpose.

The complete list of known gaps is in
[analysis limits](../../reference/analysis-limits/#known-analysis-limits).

## Too much is found

### A cycle only goes through types

Run with `--skip-type-imports`, or set `skip_type_imports: true` for the
packages concerned. See [type-only imports](../typescript/#type-only-imports).

### A cycle goes through a barrel file

`export * from './x'` in an `index.ts` makes the barrel depend on every module
it re-exports, so a module that imports the barrel from inside the same
package often closes a loop. Import the module directly; see
[fixing cycles](../fixing-cycles/#imports-through-a-barrel-file).

### A cycle Node would never load

`require` is recognized by name. A local function or parameter called
`require` that is called with a string creates an edge too. Rename it or
`ignore` the file.

### Fixing one cycle reveals another

jscycles shows one loop per tangle of files. When that loop is broken, the
rest of the tangle may still contain one, which is then shown. The number of
tangles goes down as you fix them.

## Errors

Every error exits with status `2`, prints a message starting with `Error:`,
and prints no report.

### A syntax error stops the scan

```text
Error: Import extraction error: Failed to parse /repo/packages/web/src/bad.ts: source contains syntax errors or missing tokens (first syntax error at line 1, column 1)
```

A file that cannot be parsed would leave a hole in the graph, so the scan
stops. If the file is deliberately invalid, such as a test fixture, add it to
[`ignore`](../../reference/configuration/#defaults-and-packages). If it is
valid code, please [report it](https://github.com/ALMaclaine/jscycles/issues)
with the line and column shown.

### A configuration error

```text
Error: IO error: configuration jscycles.yaml: Failed to parse config YAML: error: line 1 column 8: unknown field `inclde`, expected one of include, exclude
```

Unknown keys, duplicate keys, `null` values and invalid globs are errors, so a
typo never silently does nothing. Add the schema line from the
[configuration reference](../../reference/configuration/) for completion and
validation in your editor.

### An analysis limit is exceeded

```text
Error: Workspace error: analysis limit exceeded: input files (limit 100000)
```

Every scan has finite quotas, so a huge or hostile checkout fails clearly
instead of exhausting memory. For a large repository you trust, raise the
quota named in the message under `limits`; the keys are listed in
[analysis limits](../../reference/analysis-limits/).

### "no prebuilt binary" or "package is not installed"

The npm launcher could not find the native binary for your platform. Reinstall
without `--omit=optional` or `--no-optional`, or install with Cargo and set
`JSCYCLES_BINARY`. See [install](../install/).

## FAQ

### Does jscycles run my code or my build?

No. It reads files and parses them; it runs no package scripts, installs
nothing, and contacts no network service.

### Do I need `node_modules` installed?

Only if a `tsconfig.json` extends a configuration from a package, such as
`@tsconfig/node20`; that file is read from `node_modules`. Imports of
dependencies are not followed either way.

### Can I allow one specific cycle?

Not individually. You can leave files out with `ignore`, leave packages out
with `--exclude`, or ratchet the total count; see
[adopting jscycles](../adopting/).

### Is the output stable enough to diff?

Yes. The same input always gives the same output, and the JSON fields are
stable within a minor version; see [JSON output](../../reference/json-output/).

### Does it support Vue, Svelte, Astro or Flow?

No. It reads `.ts`, `.tsx`, `.mts`, `.cts`, `.js`, `.jsx`, `.mjs` and `.cjs`
files, not scripts embedded in other file types, and not Flow syntax.

### Why does it report fewer cycles than madge?

madge lists every overlapping loop; jscycles lists one per tangle of files.
Both are describing the same problem. See
[migrating from madge or dpdm](../migrating/).

## Still stuck?

[Open an issue](https://github.com/ALMaclaine/jscycles/issues) with a small
project that reproduces the problem, the command you ran and its output.
