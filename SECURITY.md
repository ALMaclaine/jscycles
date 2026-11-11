# Security policy

## Reporting a vulnerability

Please report vulnerabilities privately through GitHub:
[open a private security advisory](https://github.com/ALMaclaine/jscycles/security/advisories/new).
Do not open a public issue, and do not post exploit details, credentials or
private source code in public.

Include the jscycles version (`jscycles --version`), your platform, and a
small project or input that shows the problem. You will get an acknowledgement
when the report is triaged. The project is maintained by one person on a
best-effort basis, so there is no guaranteed response time; the
[maintainer runbook](docs/maintainers/maintainer-runbook.md) describes how
reports are handled through to a fixed release.

## Supported versions

Security fixes are made in the latest released minor version (currently
0.2.x). Older versions do not receive backports; upgrade to the latest
release.

## Scope

jscycles reads a local checkout, which may be untrusted (for example a pull
request in CI). By design it:

- runs no project code, scripts or package-manager hooks;
- makes no network connections and writes nothing except its report on stdout
  and diagnostics on stderr;
- enforces limits on files, bytes, imports, graph size, cycles, work and output
  ([analysis limits](docs/analysis-limits.md)), so it fails with exit status 2
  rather than exhausting memory or time without bound;
- does not follow directory symlinks below the directories it scans. File
  symlinks are followed and read as source even when they point outside the
  checkout, so their target's import specifiers can appear in reports.

Reports that break one of these properties are in scope: for example, input
that makes jscycles read outside the scanned tree through a directory link,
crash, hang or use unbounded memory despite the limits, or execute code.
Reports and diagnostics contain file names and paths from the scanned project;
treat them as you would the source. The
[threat model](docs/maintainers/architecture-and-threat-model.md) describes
the trust boundaries in more detail.
