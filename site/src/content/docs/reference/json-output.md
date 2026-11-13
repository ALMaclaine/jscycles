---
title: JSON output
description: Every field of the --json report, and what stays stable between versions.
---

`jscycles --json` prints one JSON object to standard output. Read it only when
the exit status is `0` or `1`; with status `2` the scan failed and any output
is invalid.

```sh
jscycles --json > cycles.json
status=$?
[ "$status" -le 1 ] || exit "$status"
```

## Example

For a workspace where `@acme/core` has a file cycle and imports `@acme/utils`,
which imports `@acme/core` back, `jscycles --json -v` prints:

```json
{
  "has_cycles": true,
  "has_file_cycles": true,
  "has_package_cycles": true,
  "file_cycles": [
    {
      "name": "@acme/core",
      "path": "./packages/core",
      "cycles": [
        ["src/index.ts", "src/session.ts", "src/user.ts", "src/index.ts"]
      ]
    },
    { "name": "@acme/utils", "path": "./packages/utils", "cycles": [] },
    { "name": "acme", "path": ".", "cycles": [] }
  ],
  "package_cycles": [
    {
      "packages": ["@acme/core", "@acme/utils", "@acme/core"],
      "edges": [
        {
          "from_package": "@acme/core",
          "to_package": "@acme/utils",
          "files": [
            { "path": "/repo/packages/core/src/session.ts", "subpath": "strings" }
          ]
        },
        {
          "from_package": "@acme/utils",
          "to_package": "@acme/core",
          "files": [{ "path": "/repo/packages/utils/src/strings.ts" }]
        }
      ]
    }
  ]
}
```

Without `-v`, each edge's `files` is an empty array.

## Top level

| Field | Type | Meaning |
| --- | --- | --- |
| `has_cycles` | boolean | `has_file_cycles` or `has_package_cycles`. Matches exit status `1`. |
| `has_file_cycles` | boolean | At least one package has a file cycle. Always `false` with `--outer`. |
| `has_package_cycles` | boolean | At least one cycle between packages. Always `false` with `--inner`. |
| `file_cycles` | array | One entry per analyzed package, clean or not, sorted by name. Empty with `--outer`. |
| `package_cycles` | array | One entry per elementary cycle between packages. Empty with `--inner`. |

## `file_cycles[]`

| Field | Type | Meaning |
| --- | --- | --- |
| `name` | string | The package's `name` from `package.json`. |
| `path` | string | The package directory, as reached from the scanned path (`.` for the scanned directory itself). |
| `cycles` | array of arrays of strings | One representative cycle per group of files that import each other in a loop. Each cycle is a list of file paths relative to the package, starting and ending at its lexicographically smallest file. |

## `package_cycles[]`

| Field | Type | Meaning |
| --- | --- | --- |
| `packages` | array of strings | Package names around the loop, starting and ending at the alphabetically first one. |
| `edges` | array | One entry per step of the loop, in order. |

### `edges[]`

| Field | Type | Meaning |
| --- | --- | --- |
| `from_package` | string | The importing package. |
| `to_package` | string | The imported package. |
| `files` | array | Filled only with `-v`: each file in `from_package` that imports `to_package`. |

### `files[]`

| Field | Type | Meaning |
| --- | --- | --- |
| `path` | string | Absolute path of the importing file. |
| `subpath` | string, optional | Present when the import named a path inside the package, such as `strings` for `@acme/utils/strings`. Omitted otherwise, never `null`. |

## Guarantees

- The same input always produces the same output, byte for byte.
- No source code is included, only package names and file paths.
- On Windows, paths use `\` separators.

## Stability

Within a minor version (for example, all of 0.4.x), existing fields keep their names, types
and meaning. New fields may be added, so ignore keys you do not recognize.
Changes to existing fields come only with a new minor version before 1.0, and
are listed in the [changelog](../changelog/).

To process the report, see [scripting with JSON](../../guides/scripting/).
