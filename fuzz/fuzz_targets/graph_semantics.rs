#![no_main]
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use jscycles::budget::{AnalysisBudget, AnalysisLimits};
use jscycles::cycles::PackageCycleDetector;
use jscycles::graph::PackageDependencyGraph;
use jscycles::{CycleDetector, DependencyGraph};
use libfuzzer_sys::fuzz_target;

// At most five vertices: even the complete graph has only 84 elementary
// package cycles. This independent path enumerator is bounded to depth five.
fn visit(
    edges: &[[bool; 5]; 5],
    count: usize,
    labels: &[&str; 5],
    path: &mut Vec<usize>,
    cycles: &mut BTreeSet<Vec<String>>,
) {
    let start = path[0];
    let last = path[path.len() - 1];
    for next in 0..count {
        if !edges[last][next] {
            continue;
        }
        if next == start && path.len() > 1 {
            let cycle = path
                .iter()
                .copied()
                .chain([start])
                .map(|node| labels[node].to_owned())
                .collect();
            let _ = cycles.insert(cycle);
        } else if next > start && !path.contains(&next) {
            path.push(next);
            visit(edges, count, labels, path, cycles);
            let _ = path.pop();
        }
    }
}

fuzz_target!(|bytes: &[u8]| {
    if bytes.len() > 16 {
        return;
    }
    let byte = |index| bytes.get(index).copied().unwrap_or(0);
    let count = usize::from(byte(0) % 5) + 1;
    // Lexically sorted UTF-8 labels also test byte-based payload accounting.
    let labels = ["a", "b", "x", "é", "😀"];
    let mut edges = [[false; 5]; 5];
    let mut files = DependencyGraph::new();
    let mut packages = PackageDependencyGraph::new();
    for node in 0..count {
        files.add_node(PathBuf::from(labels[node]));
        packages.add_node(labels[node].to_owned());
        for target in 0..count {
            let bit = node * count + target;
            if byte(1 + bit / 8) & (1 << (bit % 8)) != 0 {
                edges[node][target] = true;
                files.add_edge(PathBuf::from(labels[node]), PathBuf::from(labels[target]));
                packages.add_edge(labels[node].to_owned(), labels[target].to_owned());
            }
        }
    }
    let mut expected = BTreeSet::new();
    for node in 0..count {
        visit(&edges, count, &labels, &mut vec![node], &mut expected);
    }
    let package_cycles = PackageCycleDetector::try_detect(&packages, &AnalysisBudget::default())
        .expect("five-node enumeration fits the default quotas");
    let actual: Vec<_> = package_cycles
        .into_iter()
        .map(|cycle| cycle.packages)
        .collect();
    assert_eq!(
        actual,
        expected.into_iter().collect::<Vec<_>>(),
        "all cycles, exactly once, in canonical order"
    );

    // A different oracle for file SCCs: transitive closure, not DFS/Johnson.
    let mut reachable = edges;
    for node in 0..count {
        reachable[node][node] = true;
    }
    for via in 0..count {
        for from in 0..count {
            for to in 0..count {
                if reachable[from][via] && reachable[via][to] {
                    reachable[from][to] = true;
                }
            }
        }
    }
    let mut seen = [false; 5];
    let mut expected_components = 0;
    for node in 0..count {
        if seen[node] {
            continue;
        }
        let members: Vec<_> = (0..count)
            .filter(|other| reachable[node][*other] && reachable[*other][node])
            .collect();
        for member in &members {
            seen[*member] = true;
        }
        if members.len() > 1 || edges[node][node] {
            expected_components += 1;
        }
    }
    let file_cycles =
        CycleDetector::try_detect(&files, &AnalysisBudget::default()).expect("small SCCs fit");
    assert_eq!(
        file_cycles.len(),
        expected_components,
        "one representative per cyclic SCC"
    );
    for cycle in &file_cycles {
        assert_eq!(cycle.path.first(), cycle.path.last(), "closed witness");
        assert!(cycle.path.len() >= 2, "nonempty witness");
        for pair in cycle.path.windows(2) {
            assert!(files.has_edge(&pair[0], &pair[1]), "real graph edge");
        }
        let start = labels
            .iter()
            .position(|label| Path::new(label) == cycle.path[0])
            .expect("known label");
        let members = (0..count)
            .filter(|other| reachable[start][*other] && reachable[*other][start])
            .count();
        if members > 1 {
            assert!(
                cycle.path.len() > 2,
                "multi-file SCCs need a multi-file witness"
            );
        }
    }

    let limits = AnalysisLimits {
        max_cycles: u64::from(byte(5)) + 1,
        max_work: u64::from(byte(6)) * 128 + 1,
        max_cycle_bytes: u64::from(byte(7)) * 32 + 1,
        max_graph_bytes: u64::from(byte(8)) * 4 + 1,
        ..AnalysisLimits::default()
    };
    let file_budget = AnalysisBudget::new(limits).expect("positive limits");
    match CycleDetector::try_detect(&files, &file_budget) {
        Ok(limited) => assert_eq!(
            limited.iter().map(|cycle| &cycle.path).collect::<Vec<_>>(),
            file_cycles
                .iter()
                .map(|cycle| &cycle.path)
                .collect::<Vec<_>>(),
            "success is complete"
        ),
        Err(_) => assert!(file_budget.check().is_err(), "exhaustion is sticky"),
    }
    let package_budget = AnalysisBudget::new(limits).expect("positive limits");
    match PackageCycleDetector::try_detect(&packages, &package_budget) {
        Ok(limited) => assert_eq!(
            limited
                .into_iter()
                .map(|cycle| cycle.packages)
                .collect::<Vec<_>>(),
            actual,
            "bounded success is complete"
        ),
        Err(_) => assert!(
            package_budget.check().is_err(),
            "package exhaustion is sticky"
        ),
    }
});
