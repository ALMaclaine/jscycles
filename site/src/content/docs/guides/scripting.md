---
title: Scripting with JSON
description: Recipes for processing the --json report with jq, Node.js and GitHub Actions.
---

`--json` prints a stable, deterministic report. The fields are described in
the [JSON output reference](../../reference/json-output/); these recipes show
what to do with it.

## Check the exit status first

Status `1` means cycles were found and the report is complete. Status `2`
means the scan failed and the output must not be used. In a shell script that
stops on errors, accept `1` and nothing else:

```sh
jscycles --json > cycles.json || [ $? -eq 1 ]
```

## Recipes with jq

Count all cycles:

```sh
jq '([.file_cycles[].cycles[]] | length) + (.package_cycles | length)' cycles.json
```

Cycles per package, largest first:

```sh
jq -r '.file_cycles[] | select(.cycles | length > 0) | "\(.cycles | length)\t\(.name)"' cycles.json | sort -rn
```

Every file cycle on one line, for grepping or diffing:

```sh
jq -r '.file_cycles[] | .name as $p | .cycles[] | "\($p): \(join(" -> "))"' cycles.json
```

```text
@acme/core: src/index.ts -> src/session.ts -> src/user.ts -> src/index.ts
```

The files behind each package edge (needs `--json -v`):

```sh
jq -r '.package_cycles[].edges[] | "\(.from_package) -> \(.to_package)", (.files[] | "  \(.path)")' cycles.json
```

## Show only new cycles in a pull request

Because the output is deterministic, the one-line form above can be compared
between two runs. On the base branch and on the pull request:

```sh
jq -r '.file_cycles[] | .name as $p | .cycles[] | "\($p): \(join(" -> "))"' cycles.json | sort > cycles.txt
```

Then the lines only in the pull request's list are the new cycles:

```sh
comm -13 base-cycles.txt pr-cycles.txt
```

A fix elsewhere in a tangle can change which representative loop is shown, so
treat this as a hint for reviewers rather than a gate.

## A GitHub Actions job summary

```yaml
- name: Check for circular imports
  run: |
    status=0
    npx jscycles --json > cycles.json || status=$?
    if [ "$status" -eq 1 ]; then
      jq -r '"## Circular imports\n",
        (.package_cycles[] | "- packages: " + (.packages | join(" → "))),
        (.file_cycles[] | .name as $p | .cycles[] | "- `\($p)`: " + join(" → "))' \
        cycles.json >> "$GITHUB_STEP_SUMMARY"
    fi
    exit $status
```

## From Node.js

```js title="scripts/cycles.mjs"
import { execFileSync } from 'node:child_process';

let stdout;
try {
  stdout = execFileSync('jscycles', ['--json'], { encoding: 'utf8' });
} catch (error) {
  if (error.status !== 1) throw error; // status 2: the scan failed
  stdout = error.stdout;
}

const report = JSON.parse(stdout);
for (const { name, cycles } of report.file_cycles) {
  if (cycles.length) console.log(`${name}: ${cycles.length}`);
}
```

Run it with `jscycles` on `PATH`, for example through `npm exec node
scripts/cycles.mjs` when jscycles is a development dependency.
