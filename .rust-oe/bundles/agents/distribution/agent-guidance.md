# Working with adopted Rust-OE guidance

Use the consuming repository's adopted guidance index to locate relevant practices and immutable source revisions. Read the complete practice, including scope, authority, evidence, exceptions, and freshness. Cite its rule ID and adopted revision with the implementation decision. Record project-specific decisions and exceptions outside managed content.

## Task triggers

| Change | Review and evidence |
| --- | --- |
| Public APIs or traits | Explain ownership, lifetimes, trait laws, and compatibility; read [API semantics](https://gitlab.maclaine.io/alm/rust-oe/-/blob/19e4133c0461129b0f5fdb286abd7b87511242e2/practices/rust-api-semantic-boundaries.md). |
| Unsafe operations or foreign interfaces | State trusted preconditions, ownership, aliasing, callback/unwind behavior, and the abstraction proof; read [unsafe boundaries](https://gitlab.maclaine.io/alm/rust-oe/-/blob/19e4133c0461129b0f5fdb286abd7b87511242e2/practices/unsafe-abstraction-boundaries.md) and [FFI](https://gitlab.maclaine.io/alm/rust-oe/-/blob/19e4133c0461129b0f5fdb286abd7b87511242e2/practices/ffi-security-boundaries.md). |
| Async resources or queues | State capacity, overload, cancellation, completion, and shutdown ownership; read [task actors](https://gitlab.maclaine.io/alm/rust-oe/-/blob/19e4133c0461129b0f5fdb286abd7b87511242e2/practices/tokio-task-actors.md). Measure a representative workload for performance claims. |
| External input, subprocesses, files, or output | State input/work bounds, parsing, path/process trust, error classes, completion, publication, and recovery; read [input boundaries](https://gitlab.maclaine.io/alm/rust-oe/-/blob/19e4133c0461129b0f5fdb286abd7b87511242e2/practices/untrusted-input-boundaries.md) and [CLI output](https://gitlab.maclaine.io/alm/rust-oe/-/blob/19e4133c0461129b0f5fdb286abd7b87511242e2/practices/cli-failure-reporting.md). |
| Dependencies, tools, CI, or releases | Name actual scope, owner, supported configurations, failure handling, and validation; read [CI selection](https://gitlab.maclaine.io/alm/rust-oe/-/blob/19e4133c0461129b0f5fdb286abd7b87511242e2/practices/cargo-ci-control-selection.md) and [tool adoption](https://gitlab.maclaine.io/alm/rust-oe/-/blob/19e4133c0461129b0f5fdb286abd7b87511242e2/practices/development-tool-adoption.md). |
| Database migrations or telemetry | Read [migration safety](https://gitlab.maclaine.io/alm/rust-oe/-/blob/19e4133c0461129b0f5fdb286abd7b87511242e2/practices/postgresql-migration-safety.md) or [observability](https://gitlab.maclaine.io/alm/rust-oe/-/blob/19e4133c0461129b0f5fdb286abd7b87511242e2/practices/performance-observability.md); select real deployment history, budgets, workloads, and recovery procedures. |

These are review questions, not keyword-based findings. A supported documented alternative can resolve a question. Report checks actually run and their limits. Guidance adoption does not silently activate a new CI gate, trust entry, architecture, or numeric performance threshold. See [adoption and exceptions](../docs/practice-adoption.md).
