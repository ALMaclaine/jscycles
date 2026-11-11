# Configuration

jscycles reads `jscycles.yaml` from the current directory, or the file passed
with `--config`. Every key is optional; without a file the defaults below
apply. Unknown keys, duplicate keys, explicit `null` values and invalid globs
are errors (exit status 2), so a typo never silently does nothing. A leading
UTF-8 byte-order mark is ignored. JSON is valid YAML, so a `jscycles.json`
passed with `--config` works too.

For completion and validation in editors that use the YAML language server
(VS Code's YAML extension, for example), start the file with:

```yaml
# yaml-language-server: $schema=https://raw.githubusercontent.com/ALMaclaine/jscycles/main/jscycles.schema.json
```

```yaml
scan:
  include: ["."]
  exclude: ["**/node_modules", "**/dist", "**/build"]

defaults:
  extensions: [ts, tsx, mts, cts, js, jsx, mjs, cjs]
  skip_type_imports: false

packages:
  "@myorg/legacy-*":
    extensions: [js]
    ignore: ["**/*.test.js"]
  "@myorg/web":
    skip_type_imports: true

limits:
  max_files: 200000
```

## Precedence

For each package, the first source that sets a value wins:

1. Command-line options (`--extensions`, `--skip-type-imports`)
2. The matching entry under `packages`
3. `defaults`
4. Built-in defaults

## `scan`

| Key | Default | Meaning |
| --- | --- | --- |
| `include` | `["."]` | Directories searched for `package.json` files when no paths are given on the command line. `packages/*` selects each directory under `packages`; `.` selects the whole tree. Ignored when paths are passed as arguments. |
| `exclude` | `["**/node_modules", "**/dist", "**/build"]` | Directories skipped entirely, both when finding packages and when reading a package's files. Setting this replaces the defaults; `node_modules`, `dist`, `build`, `.next` and `.git` directories are always skipped anyway. |

Relative patterns match paths relative to the directory being scanned: the
current directory, or each directory passed as an argument. Absolute patterns
match absolute paths. `./x`, `x` and `x/` mean the same thing, and on Windows
so do `.\x` and `x\`. A scanned directory itself is never excluded, only what
lies beneath it.

## `defaults` and `packages`

`packages` maps a glob over package names (as in `package.json` `name`) to
overrides. When several patterns match one package, the first in lexical order
of the patterns applies. Overrides for patterns that match no package are still
validated.

| Key | In | Default | Meaning |
| --- | --- | --- | --- |
| `extensions` | both | `[ts, tsx, mts, cts, js, jsx, mjs, cjs]` | File suffixes to analyze, without the dot. Letters and digits only. An empty list analyzes no files. |
| `skip_type_imports` | both | `false` | Leave out imports that TypeScript erases from JavaScript output (see [type-only imports](#type-only-imports)). |
| `ignore` | `packages` | none | Globs for files to leave out, relative to the package directory, such as `src/legacy/**` or `**/*.test.ts`. |

### Type-only imports

By default every import counts, including `import type`, because a cycle of
type imports is still a cycle for the type checker and for readers. With
`skip_type_imports` (or `--skip-type-imports`), these are left out because
they do not exist at runtime:

- `import type { A } from './a'`, `import type A from`, `import type * as A from`
- `export type { A } from './a'`
- `import type A = require('./a')`
- `import('./a')` in a type position, such as `type T = import('./a').T`,
  `typeof import('./a')`, annotations, generics, and the type side of `as` or
  `satisfies`
- every import in a declaration file (`.d.ts`, `.d.mts`, `.d.cts`)

`import { type A } from './a'` still counts: under `verbatimModuleSyntax` it
remains an import of `./a` with no bindings, which runs the module.

## `limits`

Every scan has finite quotas for files, bytes, imports, graph size, cycles and
output, so a huge or adversarial checkout fails clearly (exit status 2) instead
of exhausting memory. The defaults suit large monorepos. Each key and default
is listed in [analysis limits](analysis-limits.md#analysis-limits).

## TypeScript configuration

There is no YAML key for tsconfig. jscycles finds the nearest `tsconfig.json`
at or above each package directory and applies its `baseUrl` and `paths`,
following `extends` (a path, a package in `node_modules`, or an array) as
TypeScript does. `--tsconfig FILE` selects one file for every package and
makes any problem with it an error; `--no-tsconfig` turns alias resolution off.

A tsconfig found automatically that cannot be loaded is skipped with a warning
on stderr that names it, because its aliases would otherwise go unresolved
without notice.
