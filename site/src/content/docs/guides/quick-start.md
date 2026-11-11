---
title: Quick start
description: Run jscycles on a repository and read the result.
---

Run it at the root of a repository:

```sh
jscycles                      # every package under the current directory
jscycles packages/core        # one directory
jscycles --only "@myorg/*"    # packages whose name matches a glob
jscycles --inner              # file cycles only
jscycles --outer              # package cycles only
jscycles --json > cycles.json # machine-readable report
```

The exit status is `0` when no cycle is found, `1` when cycles are found, and
`2` on any error (bad configuration, unreadable files, syntax errors). A
failed scan never reports clean.

## Reading the report

```text
✓ No inter-package cycles

✗ @myorg/core: 1 cycle
  src/a.ts -> src/b.ts -> src/a.ts

Summary: 4 packages, 1 with file cycles (1 total), 0 inter-package cycles
```

For each group of files that import each other in a loop, jscycles reports
one representative cycle. Break it (often by importing a module directly
instead of through a barrel `index.ts`) and run again to see what remains.

`-v` also lists clean packages and, for each package cycle, the files that
create each edge. `-q` prints nothing unless cycles are found.

## Configure it

Options can live in a `jscycles.yaml` at the repository root:

```yaml
scan:
  include: ["packages/*", "apps/*"]   # where to look for packages
  exclude: ["**/node_modules", "**/fixtures"]

defaults:
  skip_type_imports: true

packages:
  "@myorg/legacy-*":
    extensions: [js]
    ignore: ["**/*.test.js"]
```

See the [configuration reference](../../reference/configuration/) for every
key, and [command line](../../reference/command-line/) for every option.
