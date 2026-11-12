---
title: Migrating from madge or dpdm
description: Map madge, dpdm and eslint-plugin-import options to jscycles, and what to expect from the first run.
---

jscycles replaces the cycle check of madge, dpdm or `import/no-cycle`. It does
not draw graphs, list orphans or print dependency trees, so keep the other tool
if you use those features.

## What changes

- **It scans packages, not entry points.** jscycles analyzes every source file
  of every package it finds (each `package.json` with a `name`). dpdm follows
  imports from the entry files you give it, so jscycles can find cycles in
  files dpdm never reached.
- **It reports one cycle per tangle.** madge and dpdm list every overlapping
  loop, often hundreds; jscycles shows one representative loop for each group
  of files that import each other. The counts are not comparable, and a
  smaller number does not mean fewer problems were found.
- **It resolves like TypeScript, without webpack.** Aliases come from
  `tsconfig.json` `paths`, including `extends` chains. webpack and RequireJS
  configuration is not read.
- **It finds package cycles** between workspace packages, which neither tool
  reports.
- **It fails on what it cannot read.** A syntax error or an unreadable file
  stops the scan with status 2 instead of being skipped.

## From madge

| madge | jscycles |
| --- | --- |
| `madge --circular src/` | `jscycles` (from the package root) |
| `--extensions ts,tsx` | `--extensions ts,tsx` |
| `--ts-config tsconfig.json` | Automatic; or `--tsconfig tsconfig.json` |
| `--exclude '<regexp>'` | `scan.exclude` (directories) or `ignore` (files) globs in `jscycles.yaml` |
| `--json` | `--json` (a [different shape](../../reference/json-output/)) |
| `--webpack-config`, `--require-config` | Not supported; express aliases as tsconfig `paths` |
| `--image`, `--dot`, `--orphans`, `--leaves`, `--depends`, `--summary` | No equivalent |

A typical script change:

```json title="package.json" del={2} ins={3}
"scripts": {
  "check:cycles": "madge --circular --extensions ts,tsx --ts-config tsconfig.json src/"
  "check:cycles": "jscycles --extensions ts,tsx"
}
```

## From dpdm

| dpdm | jscycles |
| --- | --- |
| `dpdm --exit-code circular:1 --no-tree --no-warning src/index.ts` | `jscycles` |
| `-T`, `--transform` | `--skip-type-imports` |
| `--tsconfig tsconfig.json` | Automatic; or `--tsconfig tsconfig.json` |
| `--ext .ts,.tsx` | `--extensions ts,tsx` (no dots) |
| `--exclude '<regexp>'` | `scan.exclude` or `ignore` globs in `jscycles.yaml` |
| `--skip-dynamic-imports circular` | No equivalent: `import('…')` with a literal path always counts |
| `-o report.json` | `--json > report.json` |

## From `import/no-cycle`

The ESLint rule reports cycles in the editor as you type, but it resolves the
module graph again on every lint run, which is often the slowest rule in a
configuration. A common setup keeps the rule off in ESLint and runs jscycles
in CI and a pre-commit hook instead:

```js title="eslint.config.js"
export default [
  {
    rules: { 'import/no-cycle': 'off' },
  },
];
```

## The first run

Expect different results from the tool you are replacing: different counts,
because of how cycles are grouped, and sometimes different cycles, because
resolution rules differ. Before switching the CI check:

1. Run both tools on the same checkout.
2. For a cycle only one tool reports, check whether the imports in it really
   resolve to those files.
3. If the old tool skipped type imports, compare against
   `jscycles --skip-type-imports`.

To roll out the check on a codebase with existing cycles, see
[adopting jscycles](../adopting/).
