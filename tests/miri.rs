//! Pure Rust contracts exercised by the pinned Miri lane, without native parser calls.

use std::io::Write as _;
use std::path::PathBuf;

use jscycles::budget::{AnalysisBudget, AnalysisLimits, BoundedReport, Resource};
use jscycles::cycles::PackageCycleDetector;
use jscycles::graph::PackageDependencyGraph;
use jscycles::{CycleDetector, DependencyGraph};
use std::collections::BTreeSet;

#[test]
fn workspace_package_names_and_paths_agree_when_an_alias_conflicts() {
    let workspace = jscycles::Workspace {
        root: PathBuf::from("workspace"),
        format: jscycles::workspace::WorkspaceFormat::Npm,
        packages: [
            ("@project/a".to_owned(), PathBuf::from("packages/a")),
            ("@project/b".to_owned(), PathBuf::from("packages/b")),
        ]
        .into(),
        aliases: [
            ("@project/a".to_owned(), "@project/b".to_owned()),
            ("short".to_owned(), "@project/a".to_owned()),
        ]
        .into(),
    };
    for (specifier, name) in [
        ("@project/a", "@project/a"),
        ("@project/a/sub", "@project/a"),
        ("short", "@project/a"),
        ("short/sub", "@project/a"),
        ("@project/b", "@project/b"),
    ] {
        assert!(
            workspace.is_workspace_package(specifier),
            "known name or alias"
        );
        assert_eq!(
            workspace.resolve_package_name(specifier),
            name,
            "real workspace names have precedence"
        );
        assert_eq!(
            workspace.get_package_path(specifier),
            workspace.packages.get(name),
            "lookup methods agree"
        );
    }
    assert!(
        !workspace.is_workspace_package("unknown"),
        "unknown name stays external"
    );
}

#[test]
fn elementary_cycles_match_independent_oracle_for_all_three_vertex_graphs() {
    fn visit(edges: &[[bool; 3]; 3], path: &mut Vec<usize>, found: &mut BTreeSet<Vec<String>>) {
        let start = path[0];
        let last = path[path.len() - 1];
        for next in 0..3 {
            if !edges[last][next] {
                continue;
            }
            if next == start {
                let cycle = path
                    .iter()
                    .copied()
                    .chain([start])
                    .map(|id| id.to_string())
                    .collect();
                let _ = found.insert(cycle);
            } else if next > start && !path.contains(&next) {
                path.push(next);
                visit(edges, path, found);
                let _ = path.pop();
            }
        }
    }
    let candidates = [(0, 1), (0, 2), (1, 0), (1, 2), (2, 0), (2, 1)];
    for mask in 0_u32..64 {
        let mut edges = [[false; 3]; 3];
        let mut graph = PackageDependencyGraph::new();
        for id in 0..3 {
            graph.add_node(id.to_string());
        }
        for (bit, (from, to)) in candidates.iter().enumerate() {
            if mask & (1 << bit) != 0 {
                edges[*from][*to] = true;
                graph.add_edge(from.to_string(), to.to_string());
            }
        }
        let mut expected = BTreeSet::new();
        for start in 0..3 {
            visit(&edges, &mut vec![start], &mut expected);
        }
        let found = PackageCycleDetector::try_detect(&graph, &AnalysisBudget::default())
            .expect("bounded small graph");
        let actual: BTreeSet<_> = found.iter().map(|cycle| cycle.packages.clone()).collect();
        assert_eq!(
            found.len(),
            actual.len(),
            "no repeated elementary cycles for graph {mask}"
        );
        assert_eq!(
            actual, expected,
            "complete directed cycles for graph {mask}"
        );
    }
}

#[test]
fn bounded_cycles_keep_only_complete_results() {
    let mut graph = DependencyGraph::new();
    graph.add_edge(PathBuf::from("a"), PathBuf::from("b"));
    graph.add_edge(PathBuf::from("b"), PathBuf::from("a"));
    graph.add_edge(PathBuf::from("b"), PathBuf::from("c"));
    let budget = AnalysisBudget::default();
    let cycles = CycleDetector::try_detect(&graph, &budget).expect("valid graph");
    assert_eq!(cycles.len(), 1, "one strongly connected component");
    assert_eq!(cycles[0].path.len(), 3, "closed two-file cycle");
    assert_eq!(cycles[0].path.first(), cycles[0].path.last(), "closed path");
    let exhausted = AnalysisBudget::new(AnalysisLimits {
        max_nodes: 1,
        ..AnalysisLimits::default()
    })
    .expect("positive limit");
    let failure = CycleDetector::try_detect(&graph, &exhausted).expect_err("cannot retain graph");
    assert_eq!(failure.resource(), Resource::Nodes, "node admission fails");
    assert!(exhausted.check().is_err(), "exhaustion remains sticky");
}

#[test]
fn shared_quota_and_output_fail_atomically() {
    let budget = AnalysisBudget::new(AnalysisLimits {
        max_files: 8,
        max_output_bytes: 4,
        ..AnalysisLimits::default()
    })
    .expect("positive limits");
    let workers: Vec<_> = (0..2)
        .map(|_| {
            let shared = budget.clone();
            std::thread::spawn(move || {
                for _ in 0..4 {
                    shared
                        .consume(Resource::Files, 1)
                        .expect("fits shared quota");
                }
            })
        })
        .collect();
    for worker in workers {
        worker.join().expect("worker completes");
    }
    assert_eq!(budget.used(Resource::Files), 8, "workers share a counter");
    let mut report = BoundedReport::new(budget.clone());
    report.write_all(b"1234").expect("exact output quota");
    assert!(
        report.write_all(b"5").is_err(),
        "reject extra output before publication"
    );
    assert!(report.finish().is_err(), "never publish a partial result");
    assert!(
        budget.consume(Resource::Work, 0).is_err(),
        "all callers see the failure"
    );
}
