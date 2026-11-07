# Performance observability and profiling

## Applicability and authority

- **Scope:** Product-relevant performance diagnosis and selected production instrumentation.
- **Authority:** Rust-OE local engineering guidance; the evidence sections identify upstream contracts and bounded experience.
- **Owner:** accountable: performance/SLO owner; consulted: telemetry/privacy owner. The consuming project names the responsible person/team before enforcement.
- **Reviewed:** 2026-10-20 (UTC). Version assumptions follow the named sources and the supported project toolchain/platform.
- **Adoption:** [Common ownership, exceptions, consumer, and verification contract](https://gitlab.maclaine.io/alm/rust-oe/-/blob/19e4133c0461129b0f5fdb286abd7b87511242e2/docs/practice-adoption.md).

## Decision

Instrument a product-relevant request, job, pipeline, or command boundary first. Use correlated evidence to locate the dominant cost before optimizing: latency and throughput with downstream calls, allocations, queues, lock contention, executor state, and host signals. Treat profiler overhead, metric cardinality, and the observability endpoint as production behavior to measure and operate.

## Risk

A metric or profiler can bias experiments, add overhead/cardinality, or expose sensitive data without explaining causality.

## Measurement model

- **Objective:** define the affected SLO, percentile, throughput, cost, or resource ceiling; record the workload and load shape.
- **Boundary:** attach attribution at the operation a user or system observes—route, queue consumer, scheduled job, or CLI command—not every helper by default.
- **Breakdown:** capture only the dimensions that can change a decision: dependent SQL/HTTP/I/O time and count, allocations/request, queue delay/depth, lock wait/hold time, executor scheduling, and per-thread/host saturation.
- **Comparison:** compare before and after under equivalent inputs, concurrency, hardware allocation, and warm-up. Separate a function's inclusive time from exclusive work before attributing cause.

## Diagnostic sequence

1. Identify the highest-impact operation by traffic-weighted latency, errors, resource cost, or backlog—not a single low-level metric in isolation.
2. Attribute its dependent work. A query that is individually fast can still dominate through calls per request; an outbound HTTP call can dominate a route despite low CPU.
3. Inspect queues and locks as separate systems. Check sustained arrival/service rates, intended bursts, configured capacity, queue age, and overload policy before interpreting a full/growing queue. Lock wait and hold time help identify contention and candidate critical-section changes; they do not alone identify the cause.
4. Correlate application evidence with host CPU, memory, disk, network, and executor signals. High process CPU cannot by itself identify the responsible route or task; application spans/metrics cannot by themselves establish host saturation.
5. Choose the smallest intervention whose mechanism matches the observed bottleneck. Re-run the representative workload and retain the evidence.

## Instrumentation constraints

- Start with high-level boundaries and known suspect paths. Instrumenting every tiny function can add material per-call overhead; measure its share before keeping it.
- Establish instrumentation cost in the target workload. Evaluate `per-call overhead × call rate` and its share of end-to-end work; exclude or sample paths whose measurement cost is material.
- Bound metric label cardinality. Route patterns, operation names, and normalized query identities may be valid labels; request IDs, unbounded user input, full SQL parameters, and high-cardinality URLs are not.
- Expose metrics only through the service's approved observability plane. Protect, scope, and retain them according to the deployment's telemetry policy.
- Keep profiling feature-gated or otherwise explicitly enabled. Validate both instrumented and normal builds for applications using optional instrumentation.
- Verify effective telemetry configuration and privacy/resource limits. Supported `OTEL_CONFIG_FILE` configuration can replace standalone SDK environment settings except explicit substitutions; specification defaults and exporter names do not prove Rust SDK support or safe product budgets.

## Regression controls

A performance gate earns enforcement only when it names a stable workload, warm-up and sampling policy, metric, permitted variance, and consumer impact. Prefer semantic budgets—bounded SQL calls/request, allocation-free known hot path, queue age/capacity, or a meaningful end-to-end percentile—to incidental local timing. A failed budget must report the measured operation and reproduction command.

## Coding-agent and review-bot contract

Agents must request or create a measurement hypothesis before proposing performance work. They must not add a profiler, dashboard, Prometheus labels, or a CI timing gate solely because a service uses Rust or Tokio. Review bots may ask for a bound on new queue/lock instrumentation, cardinality review for metrics, and a workload for a proposed performance budget; they should not reject a change based on function duration without production/load evidence.

## Exceptions

Microbenchmarks and low-level instrumentation are useful for a named mechanism. Performance gates need stable workload/variance evidence; no universal timing or label budget follows.

## Freshness

Revalidate when the supported compiler/tool/library, platform, exposure, data representation, or adopted contract changes. Re-run relevant evidence and review any exception with its owner; dated examples and measurements are not continuing guarantees.

## Evidence

- [equivalent work, realistic inputs, metrics and aggregation limitations](https://gitlab.maclaine.io/alm/rust-oe/-/blob/19e4133c0461129b0f5fdb286abd7b87511242e2/sources/perf-book-benchmarking.md)
- [profiler/platform/debug-information limits](https://gitlab.maclaine.io/alm/rust-oe/-/blob/19e4133c0461129b0f5fdb286abd7b87511242e2/sources/perf-book-profiling.md)
- [bounded dimensions, units and cardinality costs](https://gitlab.maclaine.io/alm/rust-oe/-/blob/19e4133c0461129b0f5fdb286abd7b87511242e2/sources/prometheus-metric-naming.md)
- [optional environment support, limits, and config-file precedence](https://gitlab.maclaine.io/alm/rust-oe/-/blob/19e4133c0461129b0f5fdb286abd7b87511242e2/sources/opentelemetry-sdk-environment.md)
- [boundary attribution and profiling overhead](https://gitlab.maclaine.io/alm/rust-oe/-/blob/19e4133c0461129b0f5fdb286abd7b87511242e2/sources/hotpath-profiling-guide.md)
- [bounded operation/label and export case study](https://gitlab.maclaine.io/alm/rust-oe/-/blob/19e4133c0461129b0f5fdb286abd7b87511242e2/sources/hotpath-grafana-prometheus.md)
- [selected metric/feature support](https://gitlab.maclaine.io/alm/rust-oe/-/blob/19e4133c0461129b0f5fdb286abd7b87511242e2/sources/tokio-runtime-metrics.md)
- [queue saturation, lock wait/hold hypotheses and feature-gated builds](https://gitlab.maclaine.io/alm/rust-oe/-/blob/19e4133c0461129b0f5fdb286abd7b87511242e2/sources/hotpath-async-concurrency.md)
- [feature-gated instrumentation and metric-endpoint deployment limits](https://gitlab.maclaine.io/alm/rust-oe/-/blob/19e4133c0461129b0f5fdb286abd7b87511242e2/sources/hotpath-rs-performance-observability.md)
- [host and neighboring-thread scheduling delay](https://gitlab.maclaine.io/alm/rust-oe/-/blob/19e4133c0461129b0f5fdb286abd7b87511242e2/sources/dial9-tokio-performance.md)
