//! Traversal stays correct across parallel files and repeated scans.
#![allow(clippy::expect_used, reason = "Fixture failures must fail the test")]

use std::fs;

use jscycles::{CycleDetector, DependencyGraph, ImportExtractor};

#[test]
fn mixed_grammars_and_pool_sizes_keep_graphs_and_observe_source_changes() {
    for threads in [4, 1] {
        let temp = tempfile::tempdir().expect("fixture");
        let root = fs::canonicalize(temp.path()).expect("root");
        let extensions = ["js", "ts", "tsx"];
        for number in 0..96 {
            let prefix = match number % 3 {
                0 => "const object = {value: 1};",
                1 => "interface Value { value: number }",
                _ => "const component = <Pane attr='not-an-import'/>;",
            };
            fs::write(
                root.join(format!(
                    "module{number:03}.{}",
                    extensions.get(number % 3).expect("bounded fixture suffix")
                )),
                format!("{prefix}\nimport './module{:03}';", (number + 1) % 96),
            )
            .expect("mixed source");
        }
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(threads)
            .build()
            .expect("test pool");
        let extractor = ImportExtractor::new(extensions.map(str::to_owned).to_vec(), None);
        for ring in [true, false] {
            if !ring {
                fs::write(root.join("module000.js"), "export const leaf = 1;")
                    .expect("break ring between scans");
            }
            let imports = pool
                .install(|| extractor.extract(&root))
                .expect("complete scan");
            assert_eq!(imports.len(), if ring { 96 } else { 95 });
            let cycles = CycleDetector::detect(&DependencyGraph::from_imports(&imports));
            assert_eq!(cycles.len(), usize::from(ring));
        }
    }
}
