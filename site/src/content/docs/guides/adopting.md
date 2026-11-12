---
title: Adopting it in an existing codebase
description: Roll out a cycle check on a codebase that already has cycles, without blocking everyone.
---

On a large codebase the first run usually reports dozens of cycles. Fixing
them all before turning on the check is rarely practical. Instead, stop new
cycles first, then reduce the old ones over time.

## Measure

Get the numbers per package:

```sh
jscycles --json > cycles.json || [ $? -eq 1 ]
jq -r '.file_cycles[] | select(.cycles | length > 0) | "\(.cycles | length)\t\(.name)"' cycles.json | sort -rn
```

```text
14	@acme/web
3	@acme/core
1	@acme/billing
```

Each count is the number of separate tangles in the package, not every
overlapping path, so it is a fair measure of the work left.

Then decide what counts. If only runtime problems matter to you, measure with
`--skip-type-imports` (see [type-only imports](../../reference/configuration/#type-only-imports)):
cycles made only of `import type` usually disappear, often a large share.

## Choose a starting point

Pick whichever of these is closest to passing today, and enforce it in CI.

### Package cycles only

Package cycles are usually few, and they matter most for builds:

```sh
jscycles --outer
```

### Only the clean packages

Check every package except those that still have cycles, and remove packages
from the list as they are fixed:

```sh
jscycles --exclude @acme/web --exclude @acme/core
```

Or the reverse, for a small set you want to protect:

```sh
jscycles --only "@acme/shared-*"
```

### Everything except known files

Leave specific files out of the analysis, per package:

```yaml title="jscycles.yaml"
packages:
  "@acme/web":
    ignore:
      - src/legacy/**
      - src/store/index.ts
```

An ignored file is not analyzed at all, so a new cycle through it goes
unnoticed. Keep the list short and fix entries over time.

## Ratchet the count down

To allow the existing cycles but fail on any increase, keep the current count
in the repository and compare it in CI:

```sh title="scripts/check-cycles.sh"
#!/usr/bin/env bash
set -euo pipefail
jscycles --json > cycles.json || [ $? -eq 1 ]
count=$(jq '([.file_cycles[].cycles[]] | length) + (.package_cycles | length)' cycles.json)
baseline=$(cat .cycles-baseline)
echo "cycles: $count (baseline $baseline)"
if [ "$count" -gt "$baseline" ]; then
  echo "New circular imports; run jscycles to see them." >&2
  exit 1
fi
if [ "$count" -lt "$baseline" ]; then
  echo "Fewer cycles than the baseline: update .cycles-baseline to $count." >&2
  exit 1
fi
```

```sh
echo 17 > .cycles-baseline
```

The second check makes whoever fixes a cycle lower the baseline in the same
change, so it never drifts back up. A count can stay the same while one cycle
is fixed and another added; once the count reaches zero, replace the script
with a plain `jscycles`.

## Then

- Add the check to CI: see [use in CI](../ci/).
- Work through the remaining cycles with [fixing cycles](../fixing-cycles/).
- Once a package is clean, take it off the `--exclude` list or `ignore` and
  let the full check protect it.
