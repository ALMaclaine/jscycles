---
title: Use in CI
description: Fail the build on circular imports with GitHub Actions or pre-commit.
---

jscycles fails the build when it finds cycles (status 1) or cannot complete
the scan (status 2).

## GitHub Actions

If the project already installs jscycles from npm:

```yaml
- run: npx jscycles
```

Or download a verified release binary, with no Node.js needed:

```yaml
- name: Check for circular imports
  run: |
    version=v0.2.0
    base=https://github.com/ALMaclaine/jscycles/releases/download/$version
    curl -sSfLO "$base/jscycles-linux-x86_64"
    curl -sSfLO "$base/SHA256SUMS"
    sha256sum --check --ignore-missing SHA256SUMS
    chmod +x jscycles-linux-x86_64
    ./jscycles-linux-x86_64
```

## Adopting it in a codebase with cycles

Start with `--only` or `--exclude` for the packages that are clean, or with
`--outer` for package cycles only, and widen the check as cycles are fixed.

## pre-commit

With [pre-commit](https://pre-commit.com/), using a binary on `PATH`:

```yaml
repos:
  - repo: local
    hooks:
      - id: jscycles
        name: jscycles
        entry: jscycles --quiet
        language: system
        pass_filenames: false
        types_or: [javascript, jsx, ts, tsx]
```

## Machine-readable results

`--json` prints a stable report for other tools; see
[output and JSON](../output/). Always check the exit status before trusting a
report.
