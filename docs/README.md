# Documentation

The user documentation is published at [jscycles.dev](https://jscycles.dev/),
which also renders the pages below.

For users:

- [Configuration](configuration.md): every `jscycles.yaml` key and how options
  combine with the command line.
- [Package cycles](package-cycles.md): how workspaces are found and what
  creates a dependency between packages.
- [Analysis limits](analysis-limits.md): resource limits, their defaults, and
  known gaps in the analysis.
- [Upgrading to 0.2](migration-0.2.md): every behavior and library change since
  0.1.
- [Product contract](product-contract.md): the precise rules jscycles
  implements, used as the reference for tests.

For maintainers, in [maintainers/](maintainers/):

- [Architecture and threat model](maintainers/architecture-and-threat-model.md)
- [Maintainer and release runbook](maintainers/maintainer-runbook.md)
- [Build and dependency trust](maintainers/build-trust.md)
- [Engineering tools and results](maintainers/quality-engineering.md) and
  [verification scope](maintainers/verification.md)
- Rust-OE adoption: [adoption record](rust-oe-adoption.md),
  [full audit](maintainers/rust-oe-full-audit.md) and later reviews; machine
  records are in [maintainers/evidence/](maintainers/evidence/).
- [Check contract](../tooling/checks.md) for CI and local checks.
