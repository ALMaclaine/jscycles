---
title: Embedding in Rust
description: Run the full jscycles analysis from your own Rust program, and get structured results instead of printed output.
---

jscycles is also a set of Rust libraries. `Scan`, in the `jscycles-scan`
crate, runs the same analysis as the
command (package discovery, workspaces, tsconfig aliases, package ownership
and resource limits), and returns the results as data. The command is a
thin wrapper around it, so an embedding program and the command cannot give
different answers: a test checks that the library's JSON matches the
command's byte for byte, for every fixture and option.

## Add the dependency

```toml
[dependencies]
jscycles-scan = "0.5"
```

`jscycles-scan` re-exports the types its API uses (`Config`, the results and
the error types), so a scan needs no other jscycles crate. The `jscycles`
crate is the command-line binary (`cargo install jscycles`). The libraries
need Rust 1.93 or newer.

## Scan a repository

```rust
use jscycles_scan::{Config, JscyclesError, Scan};
use std::path::Path;

fn check(root: &Path) -> Result<bool, JscyclesError> {
    // Apply the repository's jscycles.yaml, as the command does.
    let config = Config::try_load_or_default(&root.join("jscycles.yaml"))?;
    let report = Scan::new(root).config(config).run()?;

    for package in &report.results().file_results {
        for cycle in &package.cycles {
            println!("{}: {} files", package.name, cycle.path.len() - 1);
        }
    }
    for cycle in &report.results().package_cycles {
        println!("packages: {}", cycle.packages.join(" -> "));
    }
    Ok(report.has_cycles())
}
```

`Scan::new(root)` discovers packages under `root`, like running `jscycles`
in that directory. `Scan::packages([...])` checks exactly the given package
directories, like `jscycles <paths>...`. A runnable version of this program
is the crate's
[`check` example](https://github.com/ALMaclaine/jscycles/blob/main/crates/jscycles-scan/examples/check.rs):

```sh
cargo run --package jscycles-scan --example check -- path/to/repo
```

## Options

Each builder method matches a command-line option:

| Method | Option |
| --- | --- |
| `.config(config)` | `--config` (the default is `Config::default()`, not a file) |
| `.only(["@acme/*"])` | `--only` |
| `.exclude(["legacy-*"])` | `--exclude` |
| `.extensions(["ts", "tsx"])` | `--extensions` |
| `.tsconfig(TsConfigSource::File(path))` | `--tsconfig` |
| `.tsconfig(TsConfigSource::Disabled)` | `--no-tsconfig` |
| `.skip_type_imports(true)` | `--skip-type-imports` |
| `.filter(CycleFilter::Inner)` | `--inner` |
| `.filter(CycleFilter::Outer)` | `--outer` |
| `.verbose(true)` | `--verbose` |

Nothing is read until `run()`. A `Scan` can be cloned and run again; each run
reads the files afresh.

## Results

`run()` returns a `ScanReport`:

- `results()` holds `file_results`, one entry per package with its file
  cycles, and `package_cycles`. Every cycle starts and ends with the same file
  or package. File paths keep the spelling of the path you scanned.
- With `.verbose(true)`, package cycles are in `package_cycles_with_files`
  instead, and each edge lists the files and import specifiers behind it.
- `has_cycles()` is true exactly when the command would exit `1`.
- `to_json()` returns the command's `--json` output, described in the
  [JSON output reference](../../reference/json-output/). `to_human(ColorMode::Plain)`
  returns its human-readable output; `ColorMode::Ansi` adds terminal colors.
- `ignored_tsconfigs()` lists automatically found `tsconfig.json` files that
  could not be used, with the reason. Their path aliases were not applied,
  which can hide a cycle, so show these to your users; the command prints them
  as warnings.

## Errors

An `Err` is anything the command reports with exit status `2`: a bad
configuration or glob, an unreadable file, a syntax error, a `--tsconfig`
file that cannot be used, paths in different workspaces, an exhausted
[resource limit](../../reference/configuration/), or a selection with no
packages (`DiscoveryError::NoPackages`). There is no partial result: `Ok`
means the whole selected graph was built.

Limits come from `config.limits`. Raise them for a very large repository, or
lower them to bound the work an untrusted checkout can cause.

## Lower-level building blocks

A scan is built from small crates, one per part of the analysis, each
published separately:

| Crate | Holds |
| --- | --- |
| `jscycles-budget` | The shared resource quotas |
| `jscycles-config` | `jscycles.yaml` |
| `jscycles-discovery` | Package discovery |
| `jscycles-workspace`, `jscycles-tsconfig` | Workspace and `tsconfig.json` metadata |
| `jscycles-import` | Import records |
| `jscycles-extract` | Import extraction and resolution (needs the native parser) |
| `jscycles-graph` | File and package graphs |
| `jscycles-cycles` | Cycle detection |
| `jscycles-output` | Human and JSON reports |

Use them when you already have an import graph from somewhere else and only
need cycle detection. `jscycles-graph` and `jscycles-cycles` do not depend on
the native parser:

```toml
[dependencies]
jscycles-cycles = "0.5"
jscycles-graph = "0.5"
jscycles-import = "0.5"
```

```rust
use jscycles_cycles as cycles;
use jscycles_graph::DependencyGraph;
use jscycles_import::{Import, ImportTarget};
use std::path::PathBuf;

let imports = vec![Import::new(
    PathBuf::from("a.ts"),
    ImportTarget::Resolved(PathBuf::from("a.ts")),
    "./a",
)];
let found = cycles::file_cycles(&DependencyGraph::from_imports(&imports));
assert_eq!(found.len(), 1);
```

To analyze real files, use `Scan` rather than combining the lower-level
crates by hand. Deciding which package owns each file, which directories
are boundaries, and how quotas are shared is subtle, and `Scan` is the
version that is tested against the TypeScript compiler.

The full API reference is on [docs.rs](https://docs.rs/jscycles-scan).

## From JavaScript

The npm package runs the command-line binary; it has no JavaScript API. From
Node.js, run `jscycles --json` and read its output, as shown in
[scripting with JSON](../scripting/#from-nodejs).
