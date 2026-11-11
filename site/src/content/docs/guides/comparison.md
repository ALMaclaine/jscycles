---
title: Comparison with other tools
description: How jscycles differs from madge, dpdm, eslint-plugin-import and Nx, with benchmarks.
---

- **[madge](https://github.com/pahen/madge)** draws dependency graphs as
  images, reads webpack configuration, and lists every cycle. jscycles does
  none of these. It does report package cycles in monorepos, follows tsconfig
  `extends` chains and workspace aliases, and fails rather than reporting
  clean when a file cannot be parsed.
- **[dpdm](https://github.com/acrazing/dpdm)** prints dependency trees from
  given entry files and can skip type dependencies (`-T`).
- **[eslint-plugin-import `no-cycle`](https://github.com/import-js/eslint-plugin-import/blob/main/docs/rules/no-cycle.md)**
  reports cycles inline in the editor, at a higher cost per lint run.
- **[Nx module boundaries](https://nx.dev/features/enforce-module-boundaries)**
  enforces dependency rules between projects in Nx workspaces.

Results differ between tools because their resolution rules differ. Compare
them on your own repository before switching.

## Performance

Median of five runs after one warm-up, on an Apple M3 Pro (12 cores) running
macOS, analyzing each repository's `packages` directory with every source
extension enabled:

| Repository | Source files | jscycles 0.2.0 | madge 8.0.0 | dpdm 3.14.0 |
| --- | ---: | ---: | ---: | ---: |
| [nest](https://github.com/nestjs/nest/tree/ed90b8357b40c6e94a466f17f0e6ade88cf80048) | 1,068 | 0.08 s | 2.04 s | 0.86 s |
| [typeorm](https://github.com/typeorm/typeorm/tree/5a572a40bae2f85a1d5f7b87a0aa88bbbaec5ab3) | 3,609 | 0.28 s | 2.97 s | 1.48 s |
| [excalidraw](https://github.com/excalidraw/excalidraw/tree/701b901a0f1ba3d0b390d9cdf63d112e25abac4b) | 631 | 0.11 s | 2.63 s | 0.87 s |
| [ts-morph](https://github.com/dsherret/ts-morph/tree/f288183ddb496adc6f4c5b6929830b5b73437185) | 1,171 | 0.08 s | 1.64 s | 0.83 s |

The tools count cycles differently (see [what it finds](../what-it-finds/)),
so their cycle counts are not comparable. Reproduce with
`python3 scripts/compare_tools.py` in a checkout; results depend on hardware
and file cache.
