```text
jscycles [OPTIONS] [paths]...
```

With no paths, jscycles scans the current directory, using
[`scan.include`](../configuration/#scan) from `jscycles.yaml` when it is set.

## Arguments

### `paths`

Directories or `package.json` files to check. A directory is searched for
packages (every `package.json` with a `name`, at any depth); a `package.json`
selects that one package. Files of other kinds are an error, and so is a path
that does not exist.

```sh
jscycles packages/core packages/utils
jscycles apps/web/package.json
```

Package cycles are found among the packages that are selected, so scan the
whole workspace (or use `--only`) to see cycles that pass through several
packages. Passing paths replaces `scan.include` for that run.

## Selecting packages

### `--only <glob>`

Check only packages whose `name` matches the glob. Repeat it to allow several
patterns. `*` matches any run of characters, including `/`, so `@acme/*`
selects every package in the `@acme` scope.

```sh
jscycles --only "@acme/*" --only legacy-api
```

### `--exclude <glob>`

Leave out packages whose `name` matches the glob. Repeatable. Excluded packages
are not analyzed and do not take part in package cycles. Use
[`scan.exclude`](../configuration/#scan) to skip directories instead.

### `--stdin`

Read the paths from standard input, one per line, instead of from the command
line. Empty lines are ignored; between 1 and 10,000 paths and at most 1 MiB are
accepted. It cannot be combined with `paths`.

```sh
git ls-files 'packages/*/package.json' | jscycles --stdin
```

## What to analyze

### `--extensions <list>`

Comma-separated file suffixes to analyze, without the dot. The default is
`ts,tsx,mts,cts,js,jsx,mjs,cjs`. It overrides `extensions` in `jscycles.yaml`,
both the default and per-package settings.

```sh
jscycles --extensions ts,tsx
```

### `--skip-type-imports`

Leave out imports that TypeScript erases from the JavaScript it emits:
`import type`, `export type`, `import()` in type positions, and every import in
a declaration file. Use it when only runtime cycles matter. See
[type-only imports](../configuration/#type-only-imports).

### `--tsconfig <file>`

Use this `tsconfig.json` for every package, instead of the nearest one above
each package. Any problem loading it is an error.

### `--no-tsconfig`

Do not read any `tsconfig.json`, so path aliases are not resolved. Cannot be
combined with `--tsconfig`.

## Configuration

### `-c, --config <file>`

Read settings from this file. The default is `jscycles.yaml` in the current
directory; when that default file does not exist, built-in defaults apply. A
file named explicitly must exist. JSON works too, since JSON is valid YAML.

## Output

### `--json`

Print a machine-readable report instead of the human one. Its shape is
described in [JSON output](../json-output/).

### `-v, --verbose`

List clean packages as well, and for each package cycle the files whose imports
create each edge. With `--json`, fills each edge's `files`.

### `-q, --quiet`

Print nothing when no cycle is found (including with `--json`). Errors are
still printed and still exit with status 2.

### `--inner`

Report only file cycles, within packages.

### `--outer`

Report only package cycles, between workspace packages. This also skips
building file graphs, so it is faster on large repositories. Cannot be combined
with `--inner`.

Colors are used only when standard output is a terminal.

## Exit status

| Status | Meaning |
| --- | --- |
| `0` | The scan completed and found no cycle. |
| `1` | The scan completed and found at least one cycle. |
| `2` | The scan could not complete: bad arguments or configuration, an unreadable file, a syntax error, or a [resource limit](../analysis-limits/). Treat any output as invalid. |

A failed scan never reports clean, so a CI step can rely on the status alone.

## Environment

| Variable | Effect |
| --- | --- |
| `JSCYCLES_BINARY` | Used by the npm package: run this jscycles binary instead of the one installed for the platform. |
| `RAYON_NUM_THREADS` | Number of worker threads. Defaults to the number of CPUs. |

## `--help` output

```text
{{usage}}
```
