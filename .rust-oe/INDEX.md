# Adopted Rust-OE guidance

This directory is managed by rust-oe. Record project decisions and exceptions outside it.

## agents

Concise task-triggered guidance for coding agents

Source commit: `19e4133c0461129b0f5fdb286abd7b87511242e2`.

- [distribution/agent-guidance.md](bundles/agents/distribution/agent-guidance.md)
- [docs/practice-adoption.md](bundles/agents/docs/practice-adoption.md)

## api

Public API design, ownership, trait laws, and compatibility

Source commit: `19e4133c0461129b0f5fdb286abd7b87511242e2`.

- [practices/rust-api-semantic-boundaries.md](bundles/api/practices/rust-api-semantic-boundaries.md)

## cli

Untrusted input, subprocess, file publication, and CLI failure contracts

Source commit: `19e4133c0461129b0f5fdb286abd7b87511242e2`.

- [practices/cli-failure-reporting.md](bundles/cli/practices/cli-failure-reporting.md)
- [practices/untrusted-input-boundaries.md](bundles/cli/practices/untrusted-input-boundaries.md)

## dependencies

Third-party audit trust and Cargo support/control selection

Source commit: `19e4133c0461129b0f5fdb286abd7b87511242e2`.

- [practices/cargo-ci-control-selection.md](bundles/dependencies/practices/cargo-ci-control-selection.md)
- [practices/dependency-audit-policy.md](bundles/dependencies/practices/dependency-audit-policy.md)

## observability

Representative performance evidence, profiling, and telemetry budgets

Source commit: `19e4133c0461129b0f5fdb286abd7b87511242e2`.

- [practices/performance-observability.md](bundles/observability/practices/performance-observability.md)

## standards

Engineering standards, applicability, ownership, and authority

Source commit: `19e4133c0461129b0f5fdb286abd7b87511242e2`.

- [docs/practice-adoption.md](bundles/standards/docs/practice-adoption.md)
- [docs/rust-engineering-standards.md](bundles/standards/docs/rust-engineering-standards.md)
- [practices/rust-standards-authority-map.md](bundles/standards/practices/rust-standards-authority-map.md)

## tools

Tool adoption guidance and reference tool pins; assets do not change project CI

Source commit: `19e4133c0461129b0f5fdb286abd7b87511242e2`.

- [practices/development-tool-adoption.md](bundles/tools/practices/development-tool-adoption.md)
- [tooling/README.md](bundles/tools/tooling/README.md)
- [tooling/tools.json](bundles/tools/tooling/tools.json)

## verification

Test strength, coverage, mutation, and fuzzing adoption

Source commit: `19e4133c0461129b0f5fdb286abd7b87511242e2`.

- [practices/fuzzing-boundaries.md](bundles/verification/practices/fuzzing-boundaries.md)
- [practices/verification-strength.md](bundles/verification/practices/verification-strength.md)

## Project decisions

- [Project note 1](../docs/rust-oe-adoption.md)
