//! Scan-wide admission and bounded graph-search contracts.
#![allow(clippy::expect_used, reason = "Test setup failures must fail the test")]

use std::collections::BTreeSet;
use std::fs;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use std::thread;

use jscycles::budget::{AnalysisBudget, AnalysisLimits, BoundedReport, Resource};
use jscycles::cycles::PackageCycleDetector;
use jscycles::graph::PackageDependencyGraph;
use jscycles::imports::{Import, ImportTarget};
use jscycles::{CycleDetector, DependencyGraph};

#[test]
fn admissions_are_exact_checked_and_sticky() {
    for resource in [
        Resource::Entries,
        Resource::Files,
        Resource::Bytes,
        Resource::Imports,
        Resource::Nodes,
        Resource::Edges,
        Resource::Cycles,
        Resource::Work,
        Resource::GraphBytes,
        Resource::CycleBytes,
        Resource::OutputBytes,
        Resource::FileBytes,
    ] {
        let budget = AnalysisBudget::default();
        let limit = budget.limits().limit(resource);
        budget
            .consume(resource, limit)
            .expect("exact limit accepted");
        let excess = if resource == Resource::FileBytes {
            limit + 1
        } else {
            1
        };
        let error = budget
            .consume(resource, excess)
            .expect_err("first excess rejected");
        assert_eq!(error.resource(), resource, "resource identified");
        assert_eq!(error.limit(), limit, "the error carries the selected limit");
        assert_eq!(
            budget.used(resource),
            limit,
            "failed admission not retained"
        );
        assert_eq!(
            budget.consume(Resource::Work, 0),
            Err(error),
            "failure remains sticky"
        );
    }
    let budget = AnalysisBudget::new(AnalysisLimits {
        max_work: u64::MAX,
        ..AnalysisLimits::default()
    })
    .expect("valid limits");
    assert_eq!(
        budget.limits().max_work,
        u64::MAX,
        "custom limits survive construction"
    );
    budget
        .consume(Resource::Work, u64::MAX)
        .expect("maximum accepted");
    assert!(
        budget.consume(Resource::Work, 1).is_err(),
        "overflow rejects instead of wrapping"
    );
    assert!(
        AnalysisBudget::new(AnalysisLimits {
            max_nodes: 0,
            ..AnalysisLimits::default()
        })
        .is_err(),
        "zero rejected"
    );
}

#[test]
#[expect(
    clippy::needless_collect,
    reason = "Spawn every worker before joining so this test exercises concurrent admissions"
)]
fn workers_share_admissions_without_overcommit() {
    let budget = AnalysisBudget::new(AnalysisLimits {
        max_files: 10_000,
        ..AnalysisLimits::default()
    })
    .expect("valid limits");
    let handles: Vec<_> = (0..8)
        .map(|_| {
            let worker = budget.clone();
            thread::spawn(move || {
                (0..2_000)
                    .filter(|_| worker.consume(Resource::Files, 1).is_ok())
                    .count()
            })
        })
        .collect();
    let accepted: usize = handles
        .into_iter()
        .map(|worker| worker.join().expect("worker completed"))
        .sum();
    assert!(accepted <= 10_000, "concurrent callers cannot overcommit");
    assert_eq!(
        budget.used(Resource::Files),
        10_000,
        "shared rather than per-worker quota"
    );
    assert!(budget.check().is_err(), "all workers observe exhaustion");
}

#[test]
fn file_maximum_is_not_a_cumulative_byte_counter_and_reports_are_staged() {
    let budget = AnalysisBudget::new(AnalysisLimits {
        max_file_bytes: 4,
        max_output_bytes: 4,
        ..AnalysisLimits::default()
    })
    .expect("valid limits");
    for _ in 0..10 {
        budget
            .consume(Resource::FileBytes, 4)
            .expect("each file fits");
    }
    assert_eq!(
        budget.used(Resource::FileBytes),
        4,
        "maximum tracks one file"
    );
    let mut report = BoundedReport::new(budget);
    report.write_all(b"1234").expect("exact encoded limit");
    assert!(report.write_all(b"5").is_err(), "extra output rejected");
    assert!(
        report.flush().is_err(),
        "flush observes the sticky output failure"
    );
    assert!(report.finish().is_err(), "no partial report accepted");
}

fn cli(root: &Path, arguments: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_jscycles"))
        .current_dir(root)
        .args(arguments)
        .output()
        .expect("CLI starts")
}
fn two_cycles(root: &Path) {
    fs::write(root.join("package.json"), r#"{"name":"test"}"#).expect("manifest");
    for (from, to) in [("a", "b"), ("b", "a"), ("c", "d"), ("d", "c")] {
        fs::write(
            root.join(format!("{from}.ts")),
            format!("import '{to_path}';", to_path = format_args!("./{to}")),
        )
        .expect("source");
    }
}

#[test]
fn every_cli_limit_fails_before_stdout_in_json_and_quiet_modes() {
    let temp = tempfile::tempdir().expect("directory");
    two_cycles(temp.path());
    for (setting, diagnostic) in [
        ("max_entries", "directory entries"),
        ("max_files", "input files"),
        ("max_total_bytes", "total input bytes"),
        ("max_imports", "imports"),
        ("max_nodes", "graph nodes"),
        ("max_edges", "graph edges"),
        ("max_cycles", "cycles"),
        ("max_work", "analysis work"),
        ("max_graph_bytes", "retained graph/import bytes"),
        ("max_cycle_bytes", "cycle/evidence bytes"),
        ("max_output_bytes", "report bytes"),
        ("max_file_bytes", "bytes in one input file"),
    ] {
        fs::write(
            temp.path().join("jscycles.yaml"),
            format!("limits:\n  {setting}: 1\n"),
        )
        .expect("limits");
        for args in [["--inner", "--json"], ["--inner", "--quiet"]] {
            let output = cli(temp.path(), &args);
            assert_eq!(
                output.status.code(),
                Some(2),
                "{setting}: {}",
                String::from_utf8_lossy(&output.stderr)
            );
            assert!(
                output.stdout.is_empty(),
                "{setting} must not publish a partial report"
            );
            assert!(
                String::from_utf8_lossy(&output.stderr).contains(diagnostic),
                "{setting}: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }
    }
}

#[test]
fn encoded_report_exact_limit_succeeds_and_next_byte_fails() {
    let temp = tempfile::tempdir().expect("directory");
    two_cycles(temp.path());
    let baseline = cli(temp.path(), &["--inner", "--json"]);
    assert_eq!(baseline.status.code(), Some(1), "cycles found");
    for (limit, status) in [(baseline.stdout.len(), 1), (baseline.stdout.len() - 1, 2)] {
        fs::write(
            temp.path().join("jscycles.yaml"),
            format!("limits:\n  max_output_bytes: {limit}\n"),
        )
        .expect("limits");
        let output = cli(temp.path(), &["--inner", "--json"]);
        assert_eq!(
            output.status.code(),
            Some(status),
            "encoded limit includes newline"
        );
        if status == 2 {
            assert!(output.stdout.is_empty(), "staged output discarded");
        } else {
            assert_eq!(output.stdout, baseline.stdout, "complete report preserved");
        }
    }
}

#[test]
fn optional_metadata_exhaustion_cannot_fall_back_to_clean() {
    let temp = tempfile::tempdir().expect("directory");
    fs::write(temp.path().join("package.json"), r#"{"name":"test"}"#).expect("manifest");
    fs::write(temp.path().join("a.ts"), "export const a = 1;").expect("source");
    fs::write(
        temp.path().join("tsconfig.json"),
        format!(
            "{{\"compilerOptions\":{{}},\"padding\":\"{}\"}}",
            "x".repeat(200)
        ),
    )
    .expect("metadata");
    fs::write(
        temp.path().join("jscycles.yaml"),
        "limits:\n  max_file_bytes: 128\n",
    )
    .expect("limits");
    let output = cli(temp.path(), &["--inner", "--quiet"]);
    assert_eq!(
        output.status.code(),
        Some(2),
        "best-effort metadata cannot hide a quota failure"
    );
    assert!(output.stdout.is_empty(), "no clean report");
}

#[test]
fn excluded_entries_are_still_bounded() {
    let temp = tempfile::tempdir().expect("directory");
    fs::write(
        temp.path().join("jscycles.yaml"),
        "scan:\n  exclude: ['**']\nlimits:\n  max_entries: 1\n",
    )
    .expect("limits");
    // Extraction ignores these files, but the walk must still account for them.
    fs::write(temp.path().join("package.json"), r#"{"name":"test"}"#).expect("manifest");
    let budget = AnalysisBudget::new(AnalysisLimits {
        max_entries: 1,
        ..AnalysisLimits::default()
    })
    .expect("limits");
    let extractor = jscycles::ImportExtractor::new(vec!["ts".to_owned()], None)
        .with_budget(budget.clone())
        .with_ignore_patterns(vec!["*.json".to_owned(), "*.yaml".to_owned()]);
    assert!(
        extractor.extract(temp.path()).is_err(),
        "non-source/ignored entries consume quota"
    );
    assert_eq!(
        budget.check().expect_err("sticky failure").resource(),
        Resource::Entries,
        "entry budget exhausted"
    );
}

#[test]
fn deep_chain_and_ring_fit_a_small_native_stack() {
    thread::Builder::new()
        .stack_size(128 * 1024)
        .spawn(|| {
            const N: usize = 20_000;
            let names: Vec<_> = (0..N).map(|id| format!("n{id:05}")).collect();
            let mut files = DependencyGraph::new();
            let mut packages = PackageDependencyGraph::new();
            for pair in names.windows(2) {
                files.add_edge(PathBuf::from(&pair[0]), PathBuf::from(&pair[1]));
                packages.add_edge(pair[0].clone(), pair[1].clone());
            }
            assert!(CycleDetector::detect(&files).is_empty(), "deep file DAG");
            assert!(
                PackageCycleDetector::detect(&packages).is_empty(),
                "deep package DAG"
            );
            files.add_edge(PathBuf::from(&names[N - 1]), PathBuf::from(&names[0]));
            packages.add_edge(names[N - 1].clone(), names[0].clone());
            assert_eq!(
                CycleDetector::try_detect(&files, &AnalysisBudget::default())
                    .expect("bounded file ring")[0]
                    .len(),
                N,
                "full ring retained"
            );
            assert_eq!(
                PackageCycleDetector::try_detect(&packages, &AnalysisBudget::default())
                    .expect("bounded package ring")[0]
                    .len(),
                N,
                "Johnson traversal/unblocking stay iterative"
            );
        })
        .expect("small stack thread")
        .join()
        .expect("stack-safe traversal");
}

// Independent bounded oracle: enumerate every simple path on four vertices.
fn oracle(edges: &[[bool; 4]; 4]) -> BTreeSet<Vec<String>> {
    #[expect(
        clippy::indexing_slicing,
        reason = "Oracle indices range over exactly four vertices; recursion is limited to simple paths of length four"
    )]
    fn visit(
        edges: &[[bool; 4]; 4],
        start: usize,
        path: &mut Vec<usize>,
        result: &mut BTreeSet<Vec<String>>,
    ) {
        for next in start..4 {
            if !edges[*path.last().expect("nonempty path")][next] {
                continue;
            }
            if next == start && path.len() > 1 {
                let _ = result.insert(
                    path.iter()
                        .chain(std::iter::once(&start))
                        .map(usize::to_string)
                        .collect(),
                );
            } else if !path.contains(&next) {
                path.push(next);
                visit(edges, start, path, result);
                let _ = path.pop();
            }
        }
    }
    let mut result = BTreeSet::new();
    for start in 0..4 {
        visit(edges, start, &mut vec![start], &mut result);
    }
    result
}

#[test]
fn exhaustive_four_vertex_graphs_match_independent_cycle_oracle() {
    let possible: Vec<_> = (0..4)
        .flat_map(|from| {
            (0..4)
                .filter(move |to| *to != from)
                .map(move |to| (from, to))
        })
        .collect();
    for mask in 0..(1u32 << possible.len()) {
        let mut edges = [[false; 4]; 4];
        let mut graph = PackageDependencyGraph::new();
        let mut files = DependencyGraph::new();
        for id in 0..4 {
            graph.add_node(id.to_string());
            files.add_node(PathBuf::from(id.to_string()));
        }
        for (bit, (from, to)) in possible.iter().enumerate() {
            if mask & (1 << bit) != 0 {
                edges[*from][*to] = true;
                graph.add_edge(from.to_string(), to.to_string());
                files.add_edge(
                    PathBuf::from(from.to_string()),
                    PathBuf::from(to.to_string()),
                );
            }
        }
        let expected = oracle(&edges);
        let cycles = PackageCycleDetector::detect(&graph);
        let actual: BTreeSet<_> = cycles.iter().map(|cycle| cycle.packages.clone()).collect();
        assert_eq!(actual, expected, "all elementary cycles for graph {mask}");
        assert_eq!(cycles.len(), actual.len(), "no duplicates for graph {mask}");
        let representatives = CycleDetector::detect(&files);
        // Reachability defines SCC membership independently of Tarjan.
        let mut reach = edges;
        for via in 0..4 {
            for from in 0..4 {
                for to in 0..4 {
                    reach[from][to] |= reach[from][via] && reach[via][to];
                }
            }
        }
        let cyclic_components = (0..4)
            .filter(|node| {
                reach[*node][*node]
                    && !(0..*node).any(|prior| reach[prior][*node] && reach[*node][prior])
            })
            .count();
        assert_eq!(
            representatives.len(),
            cyclic_components,
            "one file cycle per cyclic SCC for {mask}"
        );
        for cycle in representatives {
            assert_eq!(cycle.path.first(), cycle.path.last(), "closed path");
            for pair in cycle.path.windows(2) {
                assert!(
                    files.has_edge(&pair[0], &pair[1]),
                    "every returned edge exists"
                );
            }
        }
    }
}

#[test]
fn dense_cycle_enumeration_stops_at_cycle_or_work_quota() {
    let mut graph = PackageDependencyGraph::new();
    for from in 0..12 {
        for to in 0..12 {
            if from != to {
                graph.add_edge(from.to_string(), to.to_string());
            }
        }
    }
    for (limits, resource) in [
        (
            AnalysisLimits {
                max_cycles: 2,
                ..AnalysisLimits::default()
            },
            Resource::Cycles,
        ),
        (
            AnalysisLimits {
                max_work: 2_000,
                ..AnalysisLimits::default()
            },
            Resource::Work,
        ),
    ] {
        let budget = AnalysisBudget::new(limits).expect("limits");
        assert_eq!(
            PackageCycleDetector::try_detect(&graph, &budget)
                .expect_err("exponential output cut short")
                .resource(),
            resource,
            "specific quota"
        );
        assert!(
            budget.used(Resource::Cycles) > 0,
            "enumeration began before exhausting work"
        );
    }
}

#[test]
fn verbose_evidence_expansion_has_its_own_admission() {
    let imports: Vec<_> = ["b", "a"]
        .into_iter()
        .map(|target| {
            vec![Import {
                source: PathBuf::from("very_long_source_path.ts".repeat(100)),
                specifier: target.to_owned(),
                target: ImportTarget::WorkspacePackage {
                    package_name: target.to_owned(),
                    subpath: None,
                },
            }]
        })
        .collect();
    let budget = AnalysisBudget::new(AnalysisLimits {
        max_cycle_bytes: 500,
        ..AnalysisLimits::default()
    })
    .expect("limits");
    assert_eq!(
        PackageCycleDetector::try_detect_imports_with_files(
            [("a", imports[0].as_slice()), ("b", imports[1].as_slice())],
            &budget
        )
        .expect_err("evidence exceeds quota")
        .resource(),
        Resource::CycleBytes,
        "expanded paths counted"
    );
}

#[test]
fn cached_metadata_snapshots_share_ownership() {
    let temp = tempfile::tempdir().expect("directory");
    fs::write(temp.path().join("tsconfig.json"), "{}").expect("metadata");
    let cache = jscycles::TsConfigCache::new();
    let first = cache.discover_shared(temp.path()).expect("config");
    let second = cache.discover_shared(temp.path()).expect("cached config");
    assert!(
        Arc::ptr_eq(&first, &second),
        "packages reuse immutable snapshots"
    );
}

#[test]
fn import_quota_is_shared_by_parallel_package_workers() {
    let temp = tempfile::tempdir().expect("directory");
    for name in ["one", "two"] {
        let package = temp.path().join(name);
        fs::create_dir(&package).expect("package directory");
        fs::write(
            package.join("package.json"),
            format!("{{\"name\":\"{name}\"}}"),
        )
        .expect("manifest");
        fs::write(package.join("a.ts"), "import 'external';").expect("source");
    }
    fs::write(
        temp.path().join("jscycles.yaml"),
        "limits:\n  max_imports: 1\n",
    )
    .expect("limits");
    let output = cli(temp.path(), &["--inner", "--json"]);
    assert_eq!(
        output.status.code(),
        Some(2),
        "workers share one import admission"
    );
    assert!(output.stdout.is_empty(), "worker error prevents output");
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("limit exceeded: imports"),
        "shared import limit: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn outer_mode_without_workspace_does_not_enumerate_file_cycles() {
    let temp = tempfile::tempdir().expect("directory");
    two_cycles(temp.path());
    fs::write(
        temp.path().join("jscycles.yaml"),
        "limits:\n  max_cycles: 1\n",
    )
    .expect("limits");
    let output = cli(temp.path(), &["--outer", "--json"]);
    assert_eq!(
        output.status.code(),
        Some(0),
        "no package graph means no outer cycles"
    );
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).expect("complete JSON");
    assert_eq!(
        report["has_cycles"], false,
        "status and filtered report agree"
    );
}

/// Imports that stay inside their own package never search the workspace's
/// members for an owner, so the work they cost does not grow with the size of
/// the workspace.
#[test]
fn own_package_imports_cost_the_same_in_small_and_large_workspaces() {
    let work = |members: usize| {
        let temp = tempfile::tempdir().expect("directory");
        let root = temp.path();
        fs::write(
            root.join("package.json"),
            r#"{"name":"root","private":true,"workspaces":["packages/*"]}"#,
        )
        .expect("workspace");
        for member in 0..members {
            let directory = root.join(format!("packages/m{member}"));
            fs::create_dir_all(&directory).expect("member");
            fs::write(
                directory.join("package.json"),
                format!(r#"{{"name":"m{member}"}}"#),
            )
            .expect("manifest");
        }
        let package = root.join("packages/m0");
        for file in 0..20 {
            let next = (file + 1) % 20;
            fs::write(
                package.join(format!("f{file}.ts")),
                format!("import './f{next}';"),
            )
            .expect("source");
        }
        let workspace = jscycles::Workspace::discover(root)
            .expect("workspace")
            .expect("found");
        let budget = AnalysisBudget::new(AnalysisLimits::default()).expect("limits");
        let imports = jscycles::ImportExtractor::new(vec!["ts".to_owned()], None)
            .with_workspace(Some(workspace))
            .with_budget(budget.clone())
            .extract(&package)
            .expect("extraction");
        assert_eq!(
            imports.len(),
            20,
            "every import resolves inside the package"
        );
        budget.used(Resource::Work)
    };
    assert_eq!(work(2), work(200));
}
