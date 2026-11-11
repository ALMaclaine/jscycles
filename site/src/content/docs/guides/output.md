---
title: Output and JSON
description: The human report, the --json format, and its stability promise.
---

The human report lists packages with cycles. `-v` also lists clean packages
and, for each package cycle, the files that create each edge. Colors are used
only when stdout is a terminal.

## JSON

`--json` prints:

```json
{
  "has_cycles": true,
  "has_file_cycles": true,
  "has_package_cycles": true,
  "file_cycles": [
    {
      "name": "@myorg/core",
      "path": "./packages/core",
      "cycles": [["src/a.ts", "src/b.ts", "src/a.ts"]]
    }
  ],
  "package_cycles": [
    {
      "packages": ["@myorg/core", "@myorg/utils", "@myorg/core"],
      "edges": [
        { "from_package": "@myorg/core", "to_package": "@myorg/utils", "files": [] },
        { "from_package": "@myorg/utils", "to_package": "@myorg/core", "files": [] }
      ]
    }
  ]
}
```

Packages are sorted by name. Each cycle starts and ends at its
lexicographically smallest file, and file paths are relative to the package.
Edge `files` is filled only with `-v`: an absolute `path` and, when the import
named a subpath (`@myorg/utils/fmt`), a `subpath`. The same input always gives
the same output.

## Stability

Within a minor version (0.2.x), existing JSON fields keep their names, types
and meaning. New fields may be added, so ignore keys you do not recognize.
Changes to existing fields come only with a new minor version before 1.0 and
are listed in the
[changelog](https://github.com/ALMaclaine/jscycles/blob/main/CHANGELOG.md).
Always check the exit status before trusting a report.
