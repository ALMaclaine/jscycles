# Agent instructions

Read [README.md](README.md), [workspace checks](tooling/checks.md), and the
[Rust-OE adoption record](docs/rust-oe-adoption.md) before making changes.
Run `cargo xtask ci` before committing. A failed or unavailable check must be
reported as such. Intentional dependency changes refresh and review Cargo.lock
before locked checks. Install cargo-deny with `cargo xtask install-tools`.
Also run `python3 -B scripts/check_adoption.py` and the Python tests with
`python3 -B -m unittest discover -s scripts -p 'test_*.py'` using Python 3.11+.
The [full audit](docs/maintainers/rust-oe-full-audit.md) owns applicability and remaining
obligations; read its product and threat contracts for relevant changes. Follow
[verification guidance](docs/maintainers/verification.md) for parser campaigns and coverage
triage. Inventory completeness, documentation and green local checks are not
full-adherence evidence.

Cargo.toml owns the lint policy. Prefer narrow, justified `#[expect]` exceptions;
never replace a real input/error case with a silent default to satisfy a lint.
Read task-triggered practices from the adopted index and record rule IDs and
adopted revisions with implementation decisions. Test consumer-visible behavior.

Preserve CLI exit statuses (0 clean, 1 cycles, 2 error), JSON structure, and
deterministic output. Configuration, stdin, files, traversal, and output failures
must not report a clean scan. Keep project decisions outside managed guidance.

<!-- rust-oe:start -->
## Rust-OE guidance

Read the [adopted guidance index](.rust-oe/INDEX.md) and load practices relevant to the task.
Preserve project-owned decisions and exceptions; cite the applicable rule and adopted source revision.
<!-- rust-oe:end -->
