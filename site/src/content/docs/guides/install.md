---
title: Install
description: Install jscycles from npm, GitHub releases or crates.io.
---

## npm, pnpm, yarn or bun

Add it as a development dependency:

```sh
npm install --save-dev jscycles
npx jscycles
```

The `jscycles` package runs a prebuilt native binary. npm picks the one for
your platform from optional dependencies, and nothing runs at install time.
Builds exist for:

| Platform | Architectures |
| --- | --- |
| Linux | x86-64, ARM64 (statically linked, so any distribution, including Alpine) |
| macOS | Intel, Apple Silicon |
| Windows | x86-64 |

:::caution
If optional dependencies are disabled (`--omit=optional` or
`--no-optional`), the binary is missing and `jscycles` exits with status 2
saying so. It never reports a clean result without having scanned.
:::

On another platform, install with Cargo (below) and point the npm launcher at
the binary with `JSCYCLES_BINARY=/path/to/jscycles`, or run it directly.

## GitHub releases

The same binaries are attached to each
[GitHub release](https://github.com/ALMaclaine/jscycles/releases), together
with a `SHA256SUMS` file:

```sh
version=v0.2.0
base=https://github.com/ALMaclaine/jscycles/releases/download/$version
curl -sSfLO "$base/jscycles-linux-x86_64"
curl -sSfLO "$base/SHA256SUMS"
sha256sum --check --ignore-missing SHA256SUMS
chmod +x jscycles-linux-x86_64
```

## Cargo

With a Rust toolchain (1.93 or newer):

```sh
cargo install jscycles --locked
```

From a checkout: `cargo install --path . --locked`.

Next: [quick start](../quick-start/).
