//! Cycle detection using Tarjan's strongly connected components algorithm.
//!
//! Reports representative file cycles and elementary workspace-package cycles.

use std::convert::Infallible;
use std::path::PathBuf;

use crate::analysis::{self, Indexed, PackageAnalysis};
use crate::budget::{AnalysisBudget, LimitError, Meter, Resource, Unbounded, units};
use crate::graph::{DependencyGraph, EdgeEvidence, PackageDependencyGraph};
use crate::imports::{Import, ImportTarget};

/// A circular dependency cycle.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cycle {
    /// Files in the cycle, starting and ending with the same file.
    /// e.g., `[a.ts, b.ts, c.ts, a.ts]`
    pub path: Vec<PathBuf>,
}

impl Cycle {
    /// Create a new cycle from a path.
    #[inline]
    #[must_use]
    pub const fn new(path: Vec<PathBuf>) -> Self {
        Self { path }
    }

    /// Get the length of the cycle (excluding the repeated first element).
    #[inline]
    #[must_use]
    pub const fn len(&self) -> usize {
        if self.path.is_empty() {
            0
        } else {
            self.path.len().saturating_sub(1)
        }
    }

    /// Check if the cycle is empty.
    #[inline]
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.path.is_empty()
    }
}

/// Cycle detection engine using Tarjan's SCC algorithm.
pub struct CycleDetector;

impl CycleDetector {
    /// Return one deterministic representative per cyclic SCC without recursive traversal.
    /// This compatibility API has no resource quotas; use a try_ method for bounded input.
    #[must_use]
    pub fn detect(graph: &DependencyGraph) -> Vec<Cycle> {
        infallible(Self::run_graph(graph, &Unbounded))
    }

    fn run_graph<M: Meter>(graph: &DependencyGraph, meter: &M) -> Result<Vec<Cycle>, M::Error> {
        let indexed = Indexed::files(graph, meter)?;
        let paths = analysis::representative(&indexed, meter)?;
        Ok(indexed
            .materialize(paths)
            .into_iter()
            .map(Cycle::new)
            .collect())
    }

    /// Analyze a caller-owned graph with shared admission/work/output quotas.
    /// Graph construction by the caller is outside this method's budget.
    /// # Errors
    /// Returns quota exhaustion with no partial result.
    pub fn try_detect(
        graph: &DependencyGraph,
        budget: &AnalysisBudget,
    ) -> Result<Vec<Cycle>, LimitError> {
        Self::run_graph(graph, budget)
    }

    /// Build and analyze a file graph, admitting nodes/edges before allocation.
    /// Inputs remain borrowed; returned paths are owned.
    /// # Errors
    /// Returns quota exhaustion with no partial result.
    pub fn try_detect_imports(
        imports: &[Import],
        budget: &AnalysisBudget,
    ) -> Result<Vec<Cycle>, LimitError> {
        let indexed = Indexed::imports(imports, budget)?;
        let paths = analysis::representative(&indexed, budget)?;
        Ok(indexed
            .materialize(paths)
            .into_iter()
            .map(Cycle::new)
            .collect())
    }
}

/// A circular dependency cycle between packages.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageCycle {
    /// Package names in the cycle, starting and ending with the same package.
    /// e.g., `["@myorg/a", "@myorg/b", "@myorg/c", "@myorg/a"]`
    pub packages: Vec<String>,
}

impl PackageCycle {
    /// Create a new package cycle from a list of packages.
    #[inline]
    #[must_use]
    pub const fn new(packages: Vec<String>) -> Self {
        Self { packages }
    }

    /// Get the length of the cycle (excluding the repeated first element).
    #[inline]
    #[must_use]
    pub const fn len(&self) -> usize {
        if self.packages.is_empty() {
            0
        } else {
            self.packages.len().saturating_sub(1)
        }
    }

    /// Check if the cycle is empty.
    #[inline]
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.packages.is_empty()
    }
}

/// An edge in a package cycle with file-level evidence.
#[derive(Debug, Clone)]
pub struct PackageCycleEdge {
    /// Source package name.
    pub from_package: String,

    /// Target package name.
    pub to_package: String,

    /// Files in the source package that import the target package.
    /// Each entry is (`source_file`, `optional_subpath`).
    pub files: Vec<(PathBuf, Option<String>)>,
}

impl PackageCycleEdge {
    /// Create a new package cycle edge.
    #[inline]
    #[must_use]
    pub const fn new(from_package: String, to_package: String) -> Self {
        Self {
            from_package,
            to_package,
            files: Vec::new(),
        }
    }

    /// Create a package cycle edge with file evidence.
    #[inline]
    #[must_use]
    pub fn with_evidence(
        from_package: String,
        to_package: String,
        evidence: &EdgeEvidence,
    ) -> Self {
        Self {
            from_package,
            to_package,
            files: evidence.files().to_vec(),
        }
    }
}

/// A circular dependency cycle between packages with file-level detail.
///
/// This extends `PackageCycle` to include which specific files create
/// each edge in the cycle.
#[derive(Debug, Clone)]
pub struct PackageCycleWithFiles {
    /// The package names in the cycle (same as `PackageCycle.packages`).
    pub packages: Vec<String>,

    /// Edges in the cycle with file evidence.
    /// Each edge shows which files in `from_package` import `to_package`.
    pub edges: Vec<PackageCycleEdge>,
}

impl PackageCycleWithFiles {
    /// Create a new package cycle with files from a basic cycle and graph.
    #[must_use]
    pub fn from_cycle(cycle: &PackageCycle, graph: &PackageDependencyGraph) -> Self {
        let mut edges = Vec::new();

        // Iterate through consecutive pairs in the cycle
        // packages = [A, B, C, A] means edges: A->B, B->C, C->A
        for window in cycle.packages.windows(2) {
            let (Some(from), Some(to)) = (window.first(), window.get(1)) else {
                continue;
            };

            let edge = graph.edge_evidence(from, to).map_or_else(
                || PackageCycleEdge::new(from.clone(), to.clone()),
                |evidence| PackageCycleEdge::with_evidence(from.clone(), to.clone(), evidence),
            );
            edges.push(edge);
        }

        Self {
            packages: cycle.packages.clone(),
            edges,
        }
    }

    /// Get the length of the cycle (excluding the repeated first element).
    #[inline]
    #[must_use]
    pub const fn len(&self) -> usize {
        if self.packages.is_empty() {
            0
        } else {
            self.packages.len().saturating_sub(1)
        }
    }

    /// Check if the cycle is empty.
    #[inline]
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.packages.is_empty()
    }
}

/// Enumerate elementary package cycles with iterative Tarjan/Johnson traversal.
pub struct PackageCycleDetector;

impl PackageCycleDetector {
    /// Enumerate all elementary cycles. This compatibility API has no resource quotas.
    #[must_use]
    pub fn detect(graph: &PackageDependencyGraph) -> Vec<PackageCycle> {
        infallible(Self::run_graph(graph, &Unbounded))
    }

    fn run_graph<M: Meter>(
        graph: &PackageDependencyGraph,
        meter: &M,
    ) -> Result<Vec<PackageCycle>, M::Error> {
        let indexed = Indexed::packages(graph, meter)?;
        let paths = analysis::elementary(&indexed, meter)?;
        Ok(indexed
            .materialize(paths)
            .into_iter()
            .map(PackageCycle::new)
            .collect())
    }

    /// Analyze a caller-owned package graph with shared quotas.
    /// # Errors
    /// Returns quota exhaustion; construction of the caller's graph is outside this budget.
    pub fn try_detect(
        graph: &PackageDependencyGraph,
        budget: &AnalysisBudget,
    ) -> Result<Vec<PackageCycle>, LimitError> {
        Self::run_graph(graph, budget)
    }

    /// Build and analyze package edges directly from borrowed import groups.
    /// No unbounded intermediate package graph or import-vector copy is made.
    /// # Errors
    /// Returns quota exhaustion with no partial result.
    pub fn try_detect_imports<'a>(
        groups: impl IntoIterator<Item = (&'a str, &'a [Import])>,
        budget: &AnalysisBudget,
    ) -> Result<Vec<PackageCycle>, LimitError> {
        let indexed = PackageAnalysis::build(groups, false, budget)?.graph;
        let paths = analysis::elementary(&indexed, budget)?;
        Ok(indexed
            .materialize(paths)
            .into_iter()
            .map(PackageCycle::new)
            .collect())
    }

    /// Enumerate cycles and expand file evidence within the same quotas.
    /// # Errors
    /// Returns quota exhaustion, including expansion cost, with no partial result.
    pub fn try_detect_imports_with_files<'a>(
        groups: impl IntoIterator<Item = (&'a str, &'a [Import])>,
        budget: &AnalysisBudget,
    ) -> Result<Vec<PackageCycleWithFiles>, LimitError> {
        let indexed = PackageAnalysis::build(groups, true, budget)?;
        let paths = analysis::elementary(&indexed.graph, budget)?;
        let mut output = Vec::new();
        for path in paths {
            let mut edges = Vec::new();
            for pair in path.windows(2) {
                let (Some(from), Some(to)) = (pair.first(), pair.get(1)) else {
                    continue;
                };
                let from_name = indexed.graph.label(*from);
                let to_name = indexed.graph.label(*to);
                budget.consume(Resource::Work, 1)?;
                for name in [from_name, to_name] {
                    budget.consume(Resource::CycleBytes, units(name.len()))?;
                    budget.consume(Resource::CycleBytes, units(size_of::<String>()))?;
                }
                let mut files = Vec::new();
                // Several imports of one package from one file are one piece
                // of evidence; keep the first occurrence's order.
                let mut seen = std::collections::HashSet::new();
                if let Some(evidence) = indexed.evidence.get(&(*from, *to)) {
                    for import in evidence {
                        if let ImportTarget::WorkspacePackage { subpath, .. } = &import.target {
                            budget.consume(Resource::Work, 1)?;
                            if !seen.insert((&import.source, subpath)) {
                                continue;
                            }
                            budget.consume(
                                Resource::CycleBytes,
                                units(import.source.as_os_str().as_encoded_bytes().len()),
                            )?;
                            budget.consume(
                                Resource::CycleBytes,
                                units(size_of::<(PathBuf, Option<String>)>()),
                            )?;
                            budget.consume(
                                Resource::CycleBytes,
                                units(subpath.as_ref().map_or(0, String::len)),
                            )?;
                            files.push((import.source.clone(), subpath.clone()));
                        }
                    }
                }
                edges.push(PackageCycleEdge {
                    from_package: from_name.clone(),
                    to_package: to_name.clone(),
                    files,
                });
            }
            let packages = indexed.graph.materialize_path(path);
            output.push(PackageCycleWithFiles { packages, edges });
        }
        Ok(output)
    }

    /// Compatibility API for full evidence without quotas.
    #[must_use]
    pub fn detect_with_files(graph: &PackageDependencyGraph) -> Vec<PackageCycleWithFiles> {
        Self::detect(graph)
            .iter()
            .map(|cycle| PackageCycleWithFiles::from_cycle(cycle, graph))
            .collect()
    }
}

fn infallible<T>(result: Result<T, Infallible>) -> T {
    match result {
        Ok(value) => value,
        Err(never) => match never {},
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn test_simple_cycle() {
        let mut graph = DependencyGraph::new();
        graph.add_edge(PathBuf::from("a.ts"), PathBuf::from("b.ts"));
        graph.add_edge(PathBuf::from("b.ts"), PathBuf::from("a.ts"));

        let cycles = CycleDetector::detect(&graph);
        assert_eq!(cycles.len(), 1, "expected exactly one cycle");
        assert_eq!(
            cycles.first().map(Cycle::len),
            Some(2),
            "cycle should have 2 nodes"
        );
    }

    #[test]
    fn test_no_cycle() {
        let mut graph = DependencyGraph::new();
        graph.add_edge(PathBuf::from("a.ts"), PathBuf::from("b.ts"));
        graph.add_edge(PathBuf::from("b.ts"), PathBuf::from("c.ts"));

        let cycles = CycleDetector::detect(&graph);
        assert!(cycles.is_empty(), "expected no cycles");
    }

    #[test]
    fn test_self_loop() {
        let mut graph = DependencyGraph::new();
        graph.add_edge(PathBuf::from("a.ts"), PathBuf::from("a.ts"));

        let cycles = CycleDetector::detect(&graph);
        assert_eq!(cycles.len(), 1, "expected one self-loop cycle");
    }

    #[test]
    fn test_cycle_normalization() {
        let cycle = [
            PathBuf::from("c.ts"),
            PathBuf::from("a.ts"),
            PathBuf::from("b.ts"),
            PathBuf::from("c.ts"),
        ];
        let mut graph = DependencyGraph::new();
        for edge in cycle.windows(2) {
            if let (Some(from), Some(to)) = (edge.first(), edge.get(1)) {
                graph.add_edge(from.clone(), to.clone());
            }
        }
        let normalized = CycleDetector::detect(&graph).remove(0);
        assert_eq!(
            normalized
                .path
                .first()
                .map(|p| p.to_string_lossy().to_string()),
            Some("a.ts".to_owned()),
            "cycle should start from lexicographically smallest"
        );
    }

    #[test]
    fn test_package_simple_cycle() {
        let mut graph = PackageDependencyGraph::new();
        graph.add_edge("@myorg/a".to_owned(), "@myorg/b".to_owned());
        graph.add_edge("@myorg/b".to_owned(), "@myorg/a".to_owned());

        let cycles = PackageCycleDetector::detect(&graph);
        assert_eq!(cycles.len(), 1, "expected exactly one package cycle");
        assert_eq!(
            cycles.first().map(PackageCycle::len),
            Some(2),
            "package cycle should have 2 packages"
        );
    }

    #[test]
    fn test_package_no_cycle() {
        let mut graph = PackageDependencyGraph::new();
        graph.add_edge("@myorg/a".to_owned(), "@myorg/b".to_owned());
        graph.add_edge("@myorg/b".to_owned(), "@myorg/c".to_owned());

        let cycles = PackageCycleDetector::detect(&graph);
        assert!(cycles.is_empty(), "expected no package cycles");
    }

    #[test]
    fn test_package_three_way_cycle() {
        let mut graph = PackageDependencyGraph::new();
        graph.add_edge("@myorg/a".to_owned(), "@myorg/b".to_owned());
        graph.add_edge("@myorg/b".to_owned(), "@myorg/c".to_owned());
        graph.add_edge("@myorg/c".to_owned(), "@myorg/a".to_owned());

        let cycles = PackageCycleDetector::detect(&graph);
        assert_eq!(cycles.len(), 1, "expected exactly one package cycle");
        assert_eq!(
            cycles.first().map(PackageCycle::len),
            Some(3),
            "package cycle should have 3 packages"
        );
    }

    // =========================================================================
    // Additional cycle detection tests for comprehensive coverage
    // =========================================================================

    #[test]
    fn test_cycle_len_and_is_empty() {
        let empty_cycle = Cycle::new(vec![]);
        assert_eq!(empty_cycle.len(), 0, "empty cycle has length 0");
        assert!(empty_cycle.is_empty(), "empty cycle is empty");

        let single_node = Cycle::new(vec![PathBuf::from("a.ts")]);
        assert_eq!(single_node.len(), 0, "single node cycle has length 0");
        assert!(!single_node.is_empty(), "single node cycle is not empty");

        let two_node_cycle = Cycle::new(vec![
            PathBuf::from("a.ts"),
            PathBuf::from("b.ts"),
            PathBuf::from("a.ts"),
        ]);
        assert_eq!(two_node_cycle.len(), 2, "two node cycle has length 2");
        assert!(!two_node_cycle.is_empty(), "two node cycle is not empty");
    }

    #[test]
    fn test_package_cycle_len_and_is_empty() {
        let empty_cycle = PackageCycle::new(vec![]);
        assert_eq!(empty_cycle.len(), 0, "empty package cycle has length 0");
        assert!(empty_cycle.is_empty(), "empty package cycle is empty");

        let single_pkg = PackageCycle::new(vec!["@myorg/a".to_owned()]);
        assert_eq!(
            single_pkg.len(),
            0,
            "single package cycle has length 0 (minus repeat)"
        );
        assert!(!single_pkg.is_empty(), "single package cycle is not empty");

        let two_pkg_cycle = PackageCycle::new(vec![
            "@myorg/a".to_owned(),
            "@myorg/b".to_owned(),
            "@myorg/a".to_owned(),
        ]);
        assert_eq!(two_pkg_cycle.len(), 2, "two package cycle has length 2");
    }

    #[test]
    fn test_complex_cycle_with_branches() {
        // Graph: A -> B -> C -> D -> B (cycle in B-C-D)
        //        A -> E (no cycle)
        let mut graph = DependencyGraph::new();
        graph.add_edge(PathBuf::from("a.ts"), PathBuf::from("b.ts"));
        graph.add_edge(PathBuf::from("b.ts"), PathBuf::from("c.ts"));
        graph.add_edge(PathBuf::from("c.ts"), PathBuf::from("d.ts"));
        graph.add_edge(PathBuf::from("d.ts"), PathBuf::from("b.ts")); // creates cycle
        graph.add_edge(PathBuf::from("a.ts"), PathBuf::from("e.ts")); // no cycle

        let cycles = CycleDetector::detect(&graph);
        assert_eq!(cycles.len(), 1, "should detect exactly one cycle");
        assert_eq!(
            cycles.first().map(Cycle::len),
            Some(3),
            "cycle B-C-D should have 3 nodes"
        );
    }

    #[test]
    fn test_multiple_independent_cycles() {
        // Two independent cycles: A -> B -> A and C -> D -> C
        let mut graph = DependencyGraph::new();
        graph.add_edge(PathBuf::from("a.ts"), PathBuf::from("b.ts"));
        graph.add_edge(PathBuf::from("b.ts"), PathBuf::from("a.ts"));
        graph.add_edge(PathBuf::from("c.ts"), PathBuf::from("d.ts"));
        graph.add_edge(PathBuf::from("d.ts"), PathBuf::from("c.ts"));

        let cycles = CycleDetector::detect(&graph);
        assert_eq!(cycles.len(), 2, "should detect two independent cycles");
    }

    #[test]
    fn test_nested_cycles() {
        // Nested cycles: A -> B -> C -> A, B -> D -> B
        let mut graph = DependencyGraph::new();
        graph.add_edge(PathBuf::from("a.ts"), PathBuf::from("b.ts"));
        graph.add_edge(PathBuf::from("b.ts"), PathBuf::from("c.ts"));
        graph.add_edge(PathBuf::from("c.ts"), PathBuf::from("a.ts"));
        graph.add_edge(PathBuf::from("b.ts"), PathBuf::from("d.ts"));
        graph.add_edge(PathBuf::from("d.ts"), PathBuf::from("b.ts"));

        let cycles = CycleDetector::detect(&graph);
        // The SCC includes all nodes A, B, C, D as they are all reachable in cycles
        assert!(
            !cycles.is_empty(),
            "should detect cycles in nested structure"
        );
    }

    #[test]
    fn test_long_chain_no_cycle() {
        // A -> B -> C -> D -> E -> F (no cycle)
        let mut graph = DependencyGraph::new();
        graph.add_edge(PathBuf::from("a.ts"), PathBuf::from("b.ts"));
        graph.add_edge(PathBuf::from("b.ts"), PathBuf::from("c.ts"));
        graph.add_edge(PathBuf::from("c.ts"), PathBuf::from("d.ts"));
        graph.add_edge(PathBuf::from("d.ts"), PathBuf::from("e.ts"));
        graph.add_edge(PathBuf::from("e.ts"), PathBuf::from("f.ts"));

        let cycles = CycleDetector::detect(&graph);
        assert!(
            cycles.is_empty(),
            "long chain without cycle should produce no cycles"
        );
    }

    #[test]
    fn test_long_chain_with_cycle_at_end() {
        // A -> B -> C -> D -> E -> F -> D (cycle D-E-F)
        let mut graph = DependencyGraph::new();
        graph.add_edge(PathBuf::from("a.ts"), PathBuf::from("b.ts"));
        graph.add_edge(PathBuf::from("b.ts"), PathBuf::from("c.ts"));
        graph.add_edge(PathBuf::from("c.ts"), PathBuf::from("d.ts"));
        graph.add_edge(PathBuf::from("d.ts"), PathBuf::from("e.ts"));
        graph.add_edge(PathBuf::from("e.ts"), PathBuf::from("f.ts"));
        graph.add_edge(PathBuf::from("f.ts"), PathBuf::from("d.ts")); // cycle

        let cycles = CycleDetector::detect(&graph);
        assert_eq!(cycles.len(), 1, "should detect one cycle at end of chain");
        assert_eq!(
            cycles.first().map(Cycle::len),
            Some(3),
            "cycle D-E-F should have 3 nodes"
        );
    }

    #[test]
    fn test_diamond_dependency_no_cycle() {
        // Diamond: A -> B, A -> C, B -> D, C -> D (not a cycle)
        let mut graph = DependencyGraph::new();
        graph.add_edge(PathBuf::from("a.ts"), PathBuf::from("b.ts"));
        graph.add_edge(PathBuf::from("a.ts"), PathBuf::from("c.ts"));
        graph.add_edge(PathBuf::from("b.ts"), PathBuf::from("d.ts"));
        graph.add_edge(PathBuf::from("c.ts"), PathBuf::from("d.ts"));

        let cycles = CycleDetector::detect(&graph);
        assert!(
            cycles.is_empty(),
            "diamond dependency pattern is not a cycle"
        );
    }

    #[test]
    fn test_figure_eight_cycles() {
        // Figure-8: A -> B -> A, B -> C -> B
        let mut graph = DependencyGraph::new();
        graph.add_edge(PathBuf::from("a.ts"), PathBuf::from("b.ts"));
        graph.add_edge(PathBuf::from("b.ts"), PathBuf::from("a.ts"));
        graph.add_edge(PathBuf::from("b.ts"), PathBuf::from("c.ts"));
        graph.add_edge(PathBuf::from("c.ts"), PathBuf::from("b.ts"));

        let cycles = CycleDetector::detect(&graph);
        // All three are in one SCC
        assert!(!cycles.is_empty(), "figure-8 pattern should detect cycles");
    }

    #[test]
    fn test_cycle_normalization_lexical_order() {
        // The cycle should be normalized to start from lexicographically smallest node
        let mut graph = DependencyGraph::new();
        graph.add_edge(PathBuf::from("z.ts"), PathBuf::from("m.ts"));
        graph.add_edge(PathBuf::from("m.ts"), PathBuf::from("a.ts"));
        graph.add_edge(PathBuf::from("a.ts"), PathBuf::from("z.ts"));

        let cycles = CycleDetector::detect(&graph);
        assert_eq!(cycles.len(), 1, "should detect one cycle");

        let cycle = cycles.first().expect("test should have cycle");
        assert_eq!(
            cycle.path.first().map(|p| p.to_string_lossy().to_string()),
            Some("a.ts".to_owned()),
            "cycle should start with lexicographically smallest node"
        );
    }

    #[test]
    fn test_package_cycle_five_way() {
        // A -> B -> C -> D -> E -> A (5-way cycle)
        let mut graph = PackageDependencyGraph::new();
        graph.add_edge("@pkg/a".to_owned(), "@pkg/b".to_owned());
        graph.add_edge("@pkg/b".to_owned(), "@pkg/c".to_owned());
        graph.add_edge("@pkg/c".to_owned(), "@pkg/d".to_owned());
        graph.add_edge("@pkg/d".to_owned(), "@pkg/e".to_owned());
        graph.add_edge("@pkg/e".to_owned(), "@pkg/a".to_owned());

        let cycles = PackageCycleDetector::detect(&graph);
        assert_eq!(cycles.len(), 1, "should detect one 5-way cycle");
        assert_eq!(
            cycles.first().map(PackageCycle::len),
            Some(5),
            "cycle should have 5 packages"
        );
    }

    #[test]
    fn test_package_cycle_normalization() {
        // Z -> A -> M -> Z should normalize to A -> M -> Z -> A
        let mut graph = PackageDependencyGraph::new();
        graph.add_edge("@pkg/z".to_owned(), "@pkg/a".to_owned());
        graph.add_edge("@pkg/a".to_owned(), "@pkg/m".to_owned());
        graph.add_edge("@pkg/m".to_owned(), "@pkg/z".to_owned());

        let cycles = PackageCycleDetector::detect(&graph);
        assert_eq!(cycles.len(), 1, "should detect one cycle");

        let cycle = cycles.first().expect("test should have cycle");
        assert_eq!(
            cycle.packages.first(),
            Some(&"@pkg/a".to_owned()),
            "cycle should start with lexicographically smallest package"
        );
    }

    #[test]
    fn test_package_multiple_cycles() {
        // Independent cycles: A <-> B, C <-> D
        let mut graph = PackageDependencyGraph::new();
        graph.add_edge("@pkg/a".to_owned(), "@pkg/b".to_owned());
        graph.add_edge("@pkg/b".to_owned(), "@pkg/a".to_owned());
        graph.add_edge("@pkg/c".to_owned(), "@pkg/d".to_owned());
        graph.add_edge("@pkg/d".to_owned(), "@pkg/c".to_owned());

        let cycles = PackageCycleDetector::detect(&graph);
        assert_eq!(cycles.len(), 2, "should detect two independent cycles");
    }

    #[test]
    fn test_package_complex_interconnected() {
        // Complex: A -> B -> C -> A, A -> D -> E -> A
        let mut graph = PackageDependencyGraph::new();
        graph.add_edge("@pkg/a".to_owned(), "@pkg/b".to_owned());
        graph.add_edge("@pkg/b".to_owned(), "@pkg/c".to_owned());
        graph.add_edge("@pkg/c".to_owned(), "@pkg/a".to_owned());
        graph.add_edge("@pkg/a".to_owned(), "@pkg/d".to_owned());
        graph.add_edge("@pkg/d".to_owned(), "@pkg/e".to_owned());
        graph.add_edge("@pkg/e".to_owned(), "@pkg/a".to_owned());

        let cycles = PackageCycleDetector::detect(&graph);
        // Should find BOTH cycles, not just one
        assert_eq!(
            cycles.len(),
            2,
            "should detect both cycles in interconnected graph"
        );
    }

    #[test]
    fn test_package_graph_with_leaf_nodes() {
        // A -> B -> C -> A (cycle), A -> L1, B -> L2, C -> L3 (leaves)
        let mut graph = PackageDependencyGraph::new();
        graph.add_edge("@pkg/a".to_owned(), "@pkg/b".to_owned());
        graph.add_edge("@pkg/b".to_owned(), "@pkg/c".to_owned());
        graph.add_edge("@pkg/c".to_owned(), "@pkg/a".to_owned());
        graph.add_edge("@pkg/a".to_owned(), "@pkg/leaf1".to_owned());
        graph.add_edge("@pkg/b".to_owned(), "@pkg/leaf2".to_owned());
        graph.add_edge("@pkg/c".to_owned(), "@pkg/leaf3".to_owned());

        let cycles = PackageCycleDetector::detect(&graph);
        assert_eq!(
            cycles.len(),
            1,
            "should detect one cycle (leaves don't affect)"
        );
        assert_eq!(
            cycles.first().map(PackageCycle::len),
            Some(3),
            "cycle A-B-C should have 3 packages"
        );
    }

    #[test]
    fn test_empty_graph_no_cycles() {
        let graph = DependencyGraph::new();
        let cycles = CycleDetector::detect(&graph);
        assert!(cycles.is_empty(), "empty graph should have no cycles");

        let pkg_graph = PackageDependencyGraph::new();
        let pkg_cycles = PackageCycleDetector::detect(&pkg_graph);
        assert!(
            pkg_cycles.is_empty(),
            "empty package graph should have no cycles"
        );
    }

    #[test]
    fn test_single_node_no_self_loop() {
        let mut graph = DependencyGraph::new();
        graph.add_node(PathBuf::from("lonely.ts"));

        let cycles = CycleDetector::detect(&graph);
        assert!(cycles.is_empty(), "single isolated node is not a cycle");
    }

    #[test]
    fn test_cycle_determinism() {
        // Run detection multiple times to ensure deterministic output
        let mut graph = DependencyGraph::new();
        graph.add_edge(PathBuf::from("c.ts"), PathBuf::from("a.ts"));
        graph.add_edge(PathBuf::from("a.ts"), PathBuf::from("b.ts"));
        graph.add_edge(PathBuf::from("b.ts"), PathBuf::from("c.ts"));

        let cycles1 = CycleDetector::detect(&graph);
        let cycles2 = CycleDetector::detect(&graph);
        let cycles3 = CycleDetector::detect(&graph);

        assert_eq!(cycles1, cycles2, "cycle detection should be deterministic");
        assert_eq!(cycles2, cycles3, "cycle detection should be deterministic");
    }

    #[test]
    fn test_package_cycle_determinism() {
        let mut graph = PackageDependencyGraph::new();
        graph.add_edge("@pkg/c".to_owned(), "@pkg/a".to_owned());
        graph.add_edge("@pkg/a".to_owned(), "@pkg/b".to_owned());
        graph.add_edge("@pkg/b".to_owned(), "@pkg/c".to_owned());

        let cycles1 = PackageCycleDetector::detect(&graph);
        let cycles2 = PackageCycleDetector::detect(&graph);

        assert_eq!(
            cycles1, cycles2,
            "package cycle detection should be deterministic"
        );
    }

    #[test]
    fn test_cycle_clone_and_equality() {
        let cycle1 = Cycle::new(vec![
            PathBuf::from("a.ts"),
            PathBuf::from("b.ts"),
            PathBuf::from("a.ts"),
        ]);
        let cycle2 = cycle1.clone();

        assert_eq!(cycle1, cycle2, "cloned cycle should be equal");
        assert_eq!(cycle1.path, cycle2.path, "paths should be equal");
    }

    #[test]
    fn test_package_cycle_clone_and_equality() {
        let cycle1 = PackageCycle::new(vec![
            "@pkg/a".to_owned(),
            "@pkg/b".to_owned(),
            "@pkg/a".to_owned(),
        ]);
        let cycle2 = cycle1.clone();

        assert_eq!(cycle1, cycle2, "cloned package cycle should be equal");
        assert_eq!(cycle1.packages, cycle2.packages, "packages should be equal");
    }

    // =========================================================================
    // Tests for finding ALL elementary cycles (Johnson's algorithm)
    // =========================================================================

    #[test]
    fn test_package_finds_all_two_cycles() {
        // Multiple 2-cycles: A <-> B, C <-> D, E <-> F
        let mut graph = PackageDependencyGraph::new();
        graph.add_edge("@pkg/a".to_owned(), "@pkg/b".to_owned());
        graph.add_edge("@pkg/b".to_owned(), "@pkg/a".to_owned());
        graph.add_edge("@pkg/c".to_owned(), "@pkg/d".to_owned());
        graph.add_edge("@pkg/d".to_owned(), "@pkg/c".to_owned());
        graph.add_edge("@pkg/e".to_owned(), "@pkg/f".to_owned());
        graph.add_edge("@pkg/f".to_owned(), "@pkg/e".to_owned());

        let cycles = PackageCycleDetector::detect(&graph);
        assert_eq!(cycles.len(), 3, "should find all three 2-cycles");

        // Verify each cycle has length 2 (3 nodes including repeated start)
        for cycle in &cycles {
            assert_eq!(cycle.len(), 2, "each cycle should have 2 distinct packages");
        }
    }

    #[test]
    fn test_package_finds_both_cycles_in_shared_scc() {
        // Two distinct cycles sharing node A: A->B->C->A and A->D->E->A
        let mut graph = PackageDependencyGraph::new();
        graph.add_edge("@pkg/a".to_owned(), "@pkg/b".to_owned());
        graph.add_edge("@pkg/b".to_owned(), "@pkg/c".to_owned());
        graph.add_edge("@pkg/c".to_owned(), "@pkg/a".to_owned());
        graph.add_edge("@pkg/a".to_owned(), "@pkg/d".to_owned());
        graph.add_edge("@pkg/d".to_owned(), "@pkg/e".to_owned());
        graph.add_edge("@pkg/e".to_owned(), "@pkg/a".to_owned());

        let cycles = PackageCycleDetector::detect(&graph);
        assert_eq!(cycles.len(), 2, "should find both 3-cycles in same SCC");

        // Verify both cycles have 3 distinct packages
        for cycle in &cycles {
            assert_eq!(cycle.len(), 3, "each cycle should have 3 distinct packages");
        }
    }

    #[test]
    fn test_package_finds_mixed_cycle_sizes() {
        // 2-cycle: A <-> B, 3-cycle: C->D->E->C
        let mut graph = PackageDependencyGraph::new();
        // 2-cycle
        graph.add_edge("@pkg/a".to_owned(), "@pkg/b".to_owned());
        graph.add_edge("@pkg/b".to_owned(), "@pkg/a".to_owned());
        // 3-cycle
        graph.add_edge("@pkg/c".to_owned(), "@pkg/d".to_owned());
        graph.add_edge("@pkg/d".to_owned(), "@pkg/e".to_owned());
        graph.add_edge("@pkg/e".to_owned(), "@pkg/c".to_owned());

        let cycles = PackageCycleDetector::detect(&graph);
        assert_eq!(
            cycles.len(),
            2,
            "should find both cycles of different sizes"
        );

        let two_cycle_count = cycles.iter().filter(|c| c.len() == 2).count();
        let three_cycle_count = cycles.iter().filter(|c| c.len() == 3).count();

        assert_eq!(two_cycle_count, 1, "should have one 2-cycle");
        assert_eq!(three_cycle_count, 1, "should have one 3-cycle");
    }

    #[test]
    fn test_package_finds_overlapping_cycles() {
        // Overlapping cycles: A->B->C->A and A->B->D->A (share A->B edge)
        let mut graph = PackageDependencyGraph::new();
        graph.add_edge("@pkg/a".to_owned(), "@pkg/b".to_owned());
        graph.add_edge("@pkg/b".to_owned(), "@pkg/c".to_owned());
        graph.add_edge("@pkg/c".to_owned(), "@pkg/a".to_owned());
        graph.add_edge("@pkg/b".to_owned(), "@pkg/d".to_owned());
        graph.add_edge("@pkg/d".to_owned(), "@pkg/a".to_owned());

        let cycles = PackageCycleDetector::detect(&graph);
        assert_eq!(cycles.len(), 2, "should find both overlapping cycles");
    }

    #[test]
    fn test_package_finds_two_cycle_with_longer_cycles() {
        // 2-cycle A<->B plus 3-cycle involving A: A->C->D->A
        let mut graph = PackageDependencyGraph::new();
        graph.add_edge("@pkg/a".to_owned(), "@pkg/b".to_owned());
        graph.add_edge("@pkg/b".to_owned(), "@pkg/a".to_owned());
        graph.add_edge("@pkg/a".to_owned(), "@pkg/c".to_owned());
        graph.add_edge("@pkg/c".to_owned(), "@pkg/d".to_owned());
        graph.add_edge("@pkg/d".to_owned(), "@pkg/a".to_owned());

        let cycles = PackageCycleDetector::detect(&graph);
        assert_eq!(cycles.len(), 2, "should find 2-cycle and 3-cycle");

        let has_two_cycle = cycles.iter().any(|c| c.len() == 2);
        let has_three_cycle = cycles.iter().any(|c| c.len() == 3);

        assert!(has_two_cycle, "should have a 2-cycle (A<->B)");
        assert!(has_three_cycle, "should have a 3-cycle (A->C->D->A)");
    }

    #[test]
    fn test_package_cycles_are_normalized() {
        // Cycle C->A->B->C should be normalized to start from A
        let mut graph = PackageDependencyGraph::new();
        graph.add_edge("@pkg/c".to_owned(), "@pkg/a".to_owned());
        graph.add_edge("@pkg/a".to_owned(), "@pkg/b".to_owned());
        graph.add_edge("@pkg/b".to_owned(), "@pkg/c".to_owned());

        let cycles = PackageCycleDetector::detect(&graph);
        assert_eq!(cycles.len(), 1, "should find one cycle");

        let cycle = cycles.first().expect("test should have one cycle");
        assert_eq!(
            cycle.packages.first(),
            Some(&"@pkg/a".to_owned()),
            "cycle should start from lexicographically smallest package"
        );
    }

    #[test]
    fn test_package_no_duplicate_cycles() {
        // Complex graph that could produce duplicates if not handled properly
        let mut graph = PackageDependencyGraph::new();
        graph.add_edge("@pkg/a".to_owned(), "@pkg/b".to_owned());
        graph.add_edge("@pkg/b".to_owned(), "@pkg/a".to_owned());
        graph.add_edge("@pkg/a".to_owned(), "@pkg/c".to_owned());
        graph.add_edge("@pkg/c".to_owned(), "@pkg/b".to_owned());

        let cycles = PackageCycleDetector::detect(&graph);

        // Check for duplicates
        let mut seen = HashSet::new();
        for cycle in &cycles {
            let key = cycle.packages.join("->");
            assert!(seen.insert(key.clone()), "duplicate cycle found: {key}");
        }
    }

    #[test]
    fn test_package_large_scc_finds_all_two_cycles() {
        // Create a large interconnected SCC with multiple 2-cycles
        // A <-> B, B <-> C, C <-> D, all in one SCC via A->C->B connection
        let mut graph = PackageDependencyGraph::new();
        // Main chain making it one SCC
        graph.add_edge("@pkg/a".to_owned(), "@pkg/b".to_owned());
        graph.add_edge("@pkg/b".to_owned(), "@pkg/c".to_owned());
        graph.add_edge("@pkg/c".to_owned(), "@pkg/d".to_owned());
        graph.add_edge("@pkg/d".to_owned(), "@pkg/a".to_owned());
        // Add back-edges creating 2-cycles
        graph.add_edge("@pkg/b".to_owned(), "@pkg/a".to_owned());
        graph.add_edge("@pkg/c".to_owned(), "@pkg/b".to_owned());
        graph.add_edge("@pkg/d".to_owned(), "@pkg/c".to_owned());

        let cycles = PackageCycleDetector::detect(&graph);

        // Should find: A<->B, B<->C, C<->D (3 two-cycles) plus longer cycles
        let two_cycle_count = cycles.iter().filter(|c| c.len() == 2).count();
        assert_eq!(
            two_cycle_count, 3,
            "should find all three 2-cycles in large SCC"
        );
    }

    /// Test that mimics a real monorepo structure like the ender workspace.
    ///
    /// Structure:
    /// - shared-utils-test is imported by many packages
    /// - shared-utils-test imports shared-utils-query-client
    /// - shared-utils-query-client imports shared-hooks
    /// - shared-hooks imports shared-utils-test-wrappers
    /// - shared-utils-test-wrappers imports back to shared-ds (creating cycles)
    /// - shared-ds imports shared-utils-test-wrappers (2-cycle)
    /// - Multiple packages form a large interconnected SCC
    #[test]
    fn test_package_realistic_monorepo_structure() {
        let mut graph = PackageDependencyGraph::new();

        // Core utilities chain (mimics @ender/shared-utils-*)
        graph.add_edge(
            "@pkg/utils-test".to_owned(),
            "@pkg/utils-query-client".to_owned(),
        );
        graph.add_edge(
            "@pkg/utils-query-client".to_owned(),
            "@pkg/utils-error".to_owned(),
        );
        graph.add_edge(
            "@pkg/utils-error".to_owned(),
            "@pkg/utils-notifications".to_owned(),
        );
        graph.add_edge("@pkg/utils-notifications".to_owned(), "@pkg/ds".to_owned());

        // Design system (shared-ds) has bidirectional deps with test-wrappers
        graph.add_edge("@pkg/ds".to_owned(), "@pkg/utils-test-wrappers".to_owned());
        graph.add_edge("@pkg/utils-test-wrappers".to_owned(), "@pkg/ds".to_owned());

        // Test wrappers connects back to query-client (creating longer cycle)
        graph.add_edge(
            "@pkg/utils-test-wrappers".to_owned(),
            "@pkg/utils-query-client".to_owned(),
        );

        // Hooks package in the middle
        graph.add_edge(
            "@pkg/utils-query-client".to_owned(),
            "@pkg/hooks".to_owned(),
        );
        graph.add_edge(
            "@pkg/hooks".to_owned(),
            "@pkg/utils-test-wrappers".to_owned(),
        );

        // Feature packages that depend on shared code
        graph.add_edge("@pkg/feature-auth".to_owned(), "@pkg/ds".to_owned());
        graph.add_edge(
            "@pkg/feature-auth".to_owned(),
            "@pkg/utils-error".to_owned(),
        );
        graph.add_edge("@pkg/ds".to_owned(), "@pkg/feature-auth".to_owned()); // back-edge

        // Generated API packages
        graph.add_edge(
            "@pkg/generated-api".to_owned(),
            "@pkg/utils-rest".to_owned(),
        );
        graph.add_edge("@pkg/utils-rest".to_owned(), "@pkg/utils-error".to_owned());
        graph.add_edge(
            "@pkg/feature-auth".to_owned(),
            "@pkg/generated-api".to_owned(),
        );

        // Context packages
        graph.add_edge("@pkg/contexts-user".to_owned(), "@pkg/ds".to_owned());
        graph.add_edge("@pkg/contexts-user".to_owned(), "@pkg/hooks".to_owned());
        graph.add_edge(
            "@pkg/utils-test-wrappers".to_owned(),
            "@pkg/contexts-user".to_owned(),
        );

        let cycles = PackageCycleDetector::detect(&graph);

        // Should find multiple cycles including:
        // 1. @pkg/ds <-> @pkg/utils-test-wrappers (2-cycle)
        // 2. @pkg/ds <-> @pkg/feature-auth (2-cycle)
        // 3. Longer cycles through the utility chain

        let two_cycle_count = cycles.iter().filter(|c| c.len() == 2).count();
        assert!(
            two_cycle_count >= 2,
            "should find at least 2 two-cycles, found {two_cycle_count}"
        );

        let longer_cycle_count = cycles.iter().filter(|c| c.len() > 2).count();
        assert!(
            longer_cycle_count >= 1,
            "should find at least 1 longer cycle, found {longer_cycle_count}"
        );

        // Total should be more than just finding one cycle
        assert!(
            cycles.len() >= 3,
            "should find multiple cycles in interconnected monorepo, found {}",
            cycles.len()
        );
    }

    /// Test with a graph structure that has many overlapping cycles through shared nodes.
    /// This mimics how utility packages often create many cycle paths.
    #[test]
    fn test_package_hub_spoke_with_cross_connections() {
        let mut graph = PackageDependencyGraph::new();

        // Hub package that many things depend on
        let hub = "@pkg/shared-core";

        // Spoke packages
        let spokes = ["@pkg/a", "@pkg/b", "@pkg/c", "@pkg/d", "@pkg/e"];

        // Each spoke depends on hub
        for spoke in &spokes {
            graph.add_edge((*spoke).to_owned(), hub.to_owned());
        }

        // Hub depends back on some spokes (creating 2-cycles)
        graph.add_edge(hub.to_owned(), "@pkg/a".to_owned());
        graph.add_edge(hub.to_owned(), "@pkg/c".to_owned());

        // Cross-connections between spokes
        graph.add_edge("@pkg/a".to_owned(), "@pkg/b".to_owned());
        graph.add_edge("@pkg/b".to_owned(), "@pkg/c".to_owned());
        graph.add_edge("@pkg/c".to_owned(), "@pkg/d".to_owned());
        graph.add_edge("@pkg/d".to_owned(), "@pkg/e".to_owned());
        graph.add_edge("@pkg/e".to_owned(), "@pkg/a".to_owned()); // completes outer ring

        let cycles = PackageCycleDetector::detect(&graph);

        // Should find:
        // - 2 two-cycles: hub<->a, hub<->c
        // - Multiple longer cycles through the ring and hub
        let two_cycle_count = cycles.iter().filter(|c| c.len() == 2).count();
        assert_eq!(two_cycle_count, 2, "should find 2 two-cycles with hub");

        // The ring a->b->c->d->e->a plus paths through hub create many cycles
        assert!(
            cycles.len() > 2,
            "should find more than just 2-cycles, found {}",
            cycles.len()
        );
    }

    /// Test performance and correctness with a moderately large interconnected graph.
    /// 20 packages with dense connections - smaller than ender but still complex.
    #[test]
    fn test_package_medium_scale_interconnected() {
        let mut graph = PackageDependencyGraph::new();

        // Create 20 packages
        let packages: Vec<String> = (0..20).map(|i| format!("@pkg/p{i:02}")).collect();

        // Create a base ring: p00 -> p01 -> p02 -> ... -> p19 -> p00
        // Use iter().zip() to avoid indexing
        for (from, to) in packages.iter().zip(packages.iter().skip(1)) {
            graph.add_edge(from.clone(), to.clone());
        }
        // Close the ring: p19 -> p00
        if let (Some(last), Some(first)) = (packages.last(), packages.first()) {
            graph.add_edge(last.clone(), first.clone());
        }

        // Add some back-edges to create 2-cycles
        graph.add_edge(packages[1].clone(), packages[0].clone()); // p00 <-> p01
        graph.add_edge(packages[5].clone(), packages[4].clone()); // p04 <-> p05
        graph.add_edge(packages[10].clone(), packages[9].clone()); // p09 <-> p10
        graph.add_edge(packages[15].clone(), packages[14].clone()); // p14 <-> p15

        // Add some cross-connections to create additional cycles
        graph.add_edge(packages[0].clone(), packages[10].clone());
        graph.add_edge(packages[10].clone(), packages[0].clone());
        graph.add_edge(packages[5].clone(), packages[15].clone());

        let cycles = PackageCycleDetector::detect(&graph);

        // Should find at least the 2-cycles we explicitly created
        let two_cycle_count = cycles.iter().filter(|c| c.len() == 2).count();
        assert!(
            two_cycle_count >= 5,
            "should find at least 5 two-cycles, found {two_cycle_count}"
        );

        // Should complete in reasonable time (this test itself is the check)
        // If Johnson's algorithm has a bug, this could hang or take very long
    }

    /// Test that verifies we find ALL 2-cycles even when they're part of larger SCCs.
    /// This was the original bug - 2-cycles were missed when in large SCCs.
    #[test]
    fn test_package_two_cycles_not_missed_in_large_scc() {
        let mut graph = PackageDependencyGraph::new();

        // Create a chain that forms one large SCC
        // p0 -> p1 -> p2 -> p3 -> p4 -> p5 -> p0
        graph.add_edge("@pkg/p0".to_owned(), "@pkg/p1".to_owned());
        graph.add_edge("@pkg/p1".to_owned(), "@pkg/p2".to_owned());
        graph.add_edge("@pkg/p2".to_owned(), "@pkg/p3".to_owned());
        graph.add_edge("@pkg/p3".to_owned(), "@pkg/p4".to_owned());
        graph.add_edge("@pkg/p4".to_owned(), "@pkg/p5".to_owned());
        graph.add_edge("@pkg/p5".to_owned(), "@pkg/p0".to_owned()); // close the ring

        // Add 2-cycles within the SCC
        graph.add_edge("@pkg/p1".to_owned(), "@pkg/p0".to_owned()); // p0 <-> p1
        graph.add_edge("@pkg/p3".to_owned(), "@pkg/p2".to_owned()); // p2 <-> p3
        graph.add_edge("@pkg/p5".to_owned(), "@pkg/p4".to_owned()); // p4 <-> p5

        let cycles = PackageCycleDetector::detect(&graph);

        // Must find all 3 two-cycles
        let two_cycles: Vec<_> = cycles.iter().filter(|c| c.len() == 2).collect();
        assert_eq!(
            two_cycles.len(),
            3,
            "must find all 3 two-cycles even in large SCC, found: {:?}",
            two_cycles.iter().map(|c| &c.packages).collect::<Vec<_>>()
        );

        // Verify the specific pairs
        let has_p0_p1 = two_cycles.iter().any(|c| {
            c.packages.contains(&"@pkg/p0".to_owned()) && c.packages.contains(&"@pkg/p1".to_owned())
        });
        let has_p2_p3 = two_cycles.iter().any(|c| {
            c.packages.contains(&"@pkg/p2".to_owned()) && c.packages.contains(&"@pkg/p3".to_owned())
        });
        let has_p4_p5 = two_cycles.iter().any(|c| {
            c.packages.contains(&"@pkg/p4".to_owned()) && c.packages.contains(&"@pkg/p5".to_owned())
        });

        assert!(has_p0_p1, "should find p0 <-> p1 cycle");
        assert!(has_p2_p3, "should find p2 <-> p3 cycle");
        assert!(has_p4_p5, "should find p4 <-> p5 cycle");
    }

    // =========================================================================
    // Tests for PackageCycleWithFiles and file-level detail
    // =========================================================================

    #[test]
    fn test_package_cycle_edge_new() {
        let edge = PackageCycleEdge::new("@pkg/a".to_owned(), "@pkg/b".to_owned());
        assert_eq!(edge.from_package, "@pkg/a", "from_package should match");
        assert_eq!(edge.to_package, "@pkg/b", "to_package should match");
        assert!(edge.files.is_empty(), "files should be empty");
    }

    #[test]
    fn test_package_cycle_edge_with_evidence() {
        let mut evidence = EdgeEvidence::new();
        evidence.add_file(PathBuf::from("src/a.ts"), None);
        evidence.add_file(PathBuf::from("src/b.ts"), Some("utils".to_owned()));

        let edge =
            PackageCycleEdge::with_evidence("@pkg/a".to_owned(), "@pkg/b".to_owned(), &evidence);

        assert_eq!(edge.from_package, "@pkg/a", "from_package should match");
        assert_eq!(edge.to_package, "@pkg/b", "to_package should match");
        assert_eq!(edge.files.len(), 2, "should have 2 files");
    }

    #[test]
    fn test_package_cycle_with_files_len_empty() {
        let cycle = PackageCycleWithFiles {
            packages: Vec::new(),
            edges: Vec::new(),
        };
        assert_eq!(cycle.len(), 0, "empty cycle should have len 0");
        assert!(cycle.is_empty(), "empty cycle should be empty");
    }

    #[test]
    fn test_package_cycle_with_files_len() {
        let cycle = PackageCycleWithFiles {
            packages: vec![
                "@pkg/a".to_owned(),
                "@pkg/b".to_owned(),
                "@pkg/a".to_owned(),
            ],
            edges: vec![
                PackageCycleEdge::new("@pkg/a".to_owned(), "@pkg/b".to_owned()),
                PackageCycleEdge::new("@pkg/b".to_owned(), "@pkg/a".to_owned()),
            ],
        };
        assert_eq!(cycle.len(), 2, "cycle with 2 packages should have len 2");
        assert!(!cycle.is_empty(), "cycle should not be empty");
    }

    #[test]
    fn test_package_cycle_with_files_from_cycle() {
        // Build a graph with edge evidence
        let mut graph = PackageDependencyGraph::new();
        graph.add_edge_with_evidence(
            "@pkg/a".to_owned(),
            "@pkg/b".to_owned(),
            PathBuf::from("a/index.ts"),
            None,
        );
        graph.add_edge_with_evidence(
            "@pkg/b".to_owned(),
            "@pkg/a".to_owned(),
            PathBuf::from("b/index.ts"),
            Some("utils".to_owned()),
        );

        // Create a basic cycle
        let basic_cycle = PackageCycle::new(vec![
            "@pkg/a".to_owned(),
            "@pkg/b".to_owned(),
            "@pkg/a".to_owned(),
        ]);

        // Convert to cycle with files
        let cycle_with_files = PackageCycleWithFiles::from_cycle(&basic_cycle, &graph);

        assert_eq!(cycle_with_files.packages.len(), 3, "should have 3 entries");
        assert_eq!(cycle_with_files.edges.len(), 2, "should have 2 edges");

        // Check first edge: a -> b
        let first_edge = cycle_with_files.edges.first();
        assert_eq!(
            first_edge.map(|e| e.from_package.as_str()),
            Some("@pkg/a"),
            "first edge from should be @pkg/a"
        );
        assert_eq!(
            first_edge.map(|e| e.to_package.as_str()),
            Some("@pkg/b"),
            "first edge to should be @pkg/b"
        );
        assert_eq!(
            first_edge.map(|e| e.files.len()),
            Some(1),
            "first edge should have 1 file"
        );

        // Check second edge: b -> a
        let second_edge = cycle_with_files.edges.get(1);
        assert_eq!(
            second_edge.map(|e| e.from_package.as_str()),
            Some("@pkg/b"),
            "second edge from should be @pkg/b"
        );
        assert_eq!(
            second_edge.map(|e| e.to_package.as_str()),
            Some("@pkg/a"),
            "second edge to should be @pkg/a"
        );
        assert_eq!(
            second_edge.map(|e| e.files.len()),
            Some(1),
            "second edge should have 1 file"
        );

        // Check file details
        let empty_files: Vec<(PathBuf, Option<String>)> = vec![];
        let second_files = second_edge.map_or(&empty_files, |e| &e.files);
        assert_eq!(
            second_files
                .first()
                .and_then(|(_, s)| s.as_ref())
                .map(String::as_str),
            Some("utils"),
            "second edge file should have utils subpath"
        );
    }

    #[test]
    fn test_detect_with_files_simple() {
        let mut graph = PackageDependencyGraph::new();
        graph.add_edge_with_evidence(
            "@pkg/a".to_owned(),
            "@pkg/b".to_owned(),
            PathBuf::from("a/index.ts"),
            None,
        );
        graph.add_edge_with_evidence(
            "@pkg/b".to_owned(),
            "@pkg/a".to_owned(),
            PathBuf::from("b/index.ts"),
            None,
        );

        let cycles = PackageCycleDetector::detect_with_files(&graph);

        assert_eq!(cycles.len(), 1, "should detect 1 cycle");

        let cycle = cycles.first().unwrap();
        assert_eq!(cycle.len(), 2, "cycle should have 2 packages");
        assert_eq!(cycle.edges.len(), 2, "cycle should have 2 edges");

        // Verify edges have file evidence
        for edge in &cycle.edges {
            assert!(
                !edge.files.is_empty(),
                "edge {} -> {} should have files",
                edge.from_package,
                edge.to_package
            );
        }
    }

    #[test]
    fn test_detect_with_files_three_way() {
        let mut graph = PackageDependencyGraph::new();
        graph.add_edge_with_evidence(
            "@pkg/a".to_owned(),
            "@pkg/b".to_owned(),
            PathBuf::from("a/uses-b.ts"),
            None,
        );
        graph.add_edge_with_evidence(
            "@pkg/b".to_owned(),
            "@pkg/c".to_owned(),
            PathBuf::from("b/uses-c.ts"),
            Some("lib".to_owned()),
        );
        graph.add_edge_with_evidence(
            "@pkg/c".to_owned(),
            "@pkg/a".to_owned(),
            PathBuf::from("c/uses-a.ts"),
            None,
        );

        let cycles = PackageCycleDetector::detect_with_files(&graph);

        assert_eq!(cycles.len(), 1, "should detect 1 cycle");

        let cycle = cycles.first().unwrap();
        assert_eq!(cycle.len(), 3, "cycle should have 3 packages");
        assert_eq!(cycle.edges.len(), 3, "cycle should have 3 edges");

        // Find the b -> c edge and check its subpath
        let b_to_c_edge = cycle
            .edges
            .iter()
            .find(|e| e.from_package == "@pkg/b" && e.to_package == "@pkg/c");

        assert!(b_to_c_edge.is_some(), "should find b -> c edge");
        let b_to_c = b_to_c_edge.unwrap();
        assert_eq!(b_to_c.files.len(), 1, "b -> c should have 1 file");
        assert_eq!(
            b_to_c.files.first().and_then(|(_, s)| s.as_ref()),
            Some(&"lib".to_owned()),
            "b -> c file should have lib subpath"
        );
    }

    #[test]
    fn test_detect_with_files_multiple_files_per_edge() {
        let mut graph = PackageDependencyGraph::new();

        // Multiple files create the same edge
        graph.add_edge_with_evidence(
            "@pkg/a".to_owned(),
            "@pkg/b".to_owned(),
            PathBuf::from("a/index.ts"),
            None,
        );
        graph.add_edge_with_evidence(
            "@pkg/a".to_owned(),
            "@pkg/b".to_owned(),
            PathBuf::from("a/utils.ts"),
            Some("helpers".to_owned()),
        );
        graph.add_edge_with_evidence(
            "@pkg/a".to_owned(),
            "@pkg/b".to_owned(),
            PathBuf::from("a/types.ts"),
            Some("types".to_owned()),
        );
        graph.add_edge_with_evidence(
            "@pkg/b".to_owned(),
            "@pkg/a".to_owned(),
            PathBuf::from("b/index.ts"),
            None,
        );

        let cycles = PackageCycleDetector::detect_with_files(&graph);

        assert_eq!(cycles.len(), 1, "should detect 1 cycle");

        let cycle = cycles.first().unwrap();
        let a_to_b_edge = cycle
            .edges
            .iter()
            .find(|e| e.from_package == "@pkg/a" && e.to_package == "@pkg/b");

        assert!(a_to_b_edge.is_some(), "should find a -> b edge");
        assert_eq!(
            a_to_b_edge.map(|e| e.files.len()),
            Some(3),
            "a -> b edge should have 3 files"
        );
    }

    #[test]
    fn test_detect_with_files_no_cycles() {
        let mut graph = PackageDependencyGraph::new();
        graph.add_edge_with_evidence(
            "@pkg/a".to_owned(),
            "@pkg/b".to_owned(),
            PathBuf::from("a/index.ts"),
            None,
        );
        graph.add_edge_with_evidence(
            "@pkg/b".to_owned(),
            "@pkg/c".to_owned(),
            PathBuf::from("b/index.ts"),
            None,
        );

        let cycles = PackageCycleDetector::detect_with_files(&graph);

        assert!(cycles.is_empty(), "should detect no cycles");
    }

    #[test]
    fn test_package_cycle_with_files_clone() {
        let cycle = PackageCycleWithFiles {
            packages: vec![
                "@pkg/a".to_owned(),
                "@pkg/b".to_owned(),
                "@pkg/a".to_owned(),
            ],
            edges: vec![PackageCycleEdge::new(
                "@pkg/a".to_owned(),
                "@pkg/b".to_owned(),
            )],
        };

        let cloned = cycle.clone();
        assert_eq!(cloned.packages, cycle.packages, "packages should match");
        assert_eq!(cloned.edges.len(), cycle.edges.len(), "edges should match");
    }
}
