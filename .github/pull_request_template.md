## What changed

Describe the change a user would notice, and anything it could break (exit
statuses, JSON output, which cycles are reported).

## Testing

Commands run and their results, including `cargo xtask ci`. For discovery,
ownership or resolution changes, also a `scripts/differential/difftest.py` run.

- [ ] Tests cover the change
- [ ] `CHANGELOG.md` updated under `Unreleased` (if user-visible)

<!-- Maintainers: for task-triggered Rust-OE review, cite the rule, the adopted
revision, the behavior/test evidence, and any exception recorded in
docs/rust-oe-adoption.md. -->
