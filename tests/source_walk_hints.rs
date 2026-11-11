//! Positive traversal hints preserve identity, fallback, reload and quota behavior.
#![allow(clippy::expect_used, reason = "Fixture failures must fail the test")]

use std::fs;

use jscycles::budget::{AnalysisBudget, AnalysisLimits, Resource};
use jscycles::imports::ExtractionMode;
use jscycles::{CycleDetector, DependencyGraph, Import, ImportExtractor, ImportTarget};

fn extractor() -> ImportExtractor {
    ImportExtractor::new(vec!["ts".to_owned(), "tsx".to_owned()], None)
}

fn has_cycles(imports: &[Import]) -> bool {
    !CycleDetector::detect(&DependencyGraph::from_imports(imports)).is_empty()
}

#[test]
fn regular_walk_paths_preserve_canonical_source_and_target_identity() {
    let temp = tempfile::tempdir().expect("fixture");
    let root = fs::canonicalize(temp.path()).expect("root");
    fs::create_dir(root.join("canonical-π")).expect("Unicode directory");
    fs::write(root.join("entry.ts"), "import './canonical-π/other';").expect("entry");
    fs::write(root.join("canonical-π/other.ts"), "import '../entry';").expect("cycle");
    let imports = extractor()
        .extract(&root.join("canonical-π/.."))
        .expect("canonical root spelling");
    assert_eq!(imports.len(), 2);
    assert!(has_cycles(&imports));
    for import in imports {
        assert_eq!(
            import.source,
            fs::canonicalize(&import.source).expect("physical source")
        );
        let ImportTarget::Resolved(target) = import.target else {
            panic!("relative edge resolves");
        };
        assert_eq!(target, fs::canonicalize(&target).expect("physical target"));
    }
}

#[test]
fn index_hints_keep_file_precedence_and_refresh_between_extractions() {
    let temp = tempfile::tempdir().expect("fixture");
    let root = fs::canonicalize(temp.path()).expect("root");
    fs::create_dir(root.join("folder")).expect("folder");
    fs::write(root.join("entry.ts"), "import './folder';").expect("entry");
    fs::write(root.join("folder.ts"), "export const leaf = 1;").expect("higher priority file");
    fs::write(root.join("folder/index.ts"), "import '../entry';").expect("index back edge");
    let selected = extractor();
    for file_present in [true, false] {
        if !file_present {
            fs::remove_file(root.join("folder.ts")).expect("remove preferred target");
        }
        let imports = selected.extract(&root).expect("fresh traversal");
        let import = imports
            .iter()
            .find(|import| import.specifier == "./folder")
            .expect("entry import");
        assert_eq!(
            import.target,
            ImportTarget::Resolved(root.join(if file_present {
                "folder.ts"
            } else {
                "folder/index.ts"
            }))
        );
        assert_eq!(has_cycles(&imports), !file_present);
    }
}

#[test]
fn unselected_first_indices_and_case_variants_keep_filesystem_fallback() {
    let temp = tempfile::tempdir().expect("fixture");
    let root = fs::canonicalize(temp.path()).expect("root");
    for name in ["ignored", "case-folder"] {
        fs::create_dir(root.join(name)).expect("folder");
        fs::write(root.join(name).join("index.ts"), "").expect("first index");
    }
    fs::write(root.join("ignored/index.tsx"), "").expect("later selected candidate");
    fs::write(
        root.join("entry.ts"),
        "import './ignored'; import './CASE-FOLDER';",
    )
    .expect("entry");
    let ignored = root.join("ignored/index.ts");
    let imports = extractor()
        .with_ignore_patterns(vec![glob::Pattern::escape(
            ignored.to_str().expect("UTF-8 fixture"),
        )])
        .extract(&root)
        .expect("selected file list is not negative filesystem evidence");
    assert_eq!(imports.len(), 2);
    for import in imports {
        let expected = match import.specifier.as_str() {
            "./ignored" => ImportTarget::Resolved(ignored.clone()),
            "./CASE-FOLDER" if root.join("CASE-FOLDER/index.ts").is_file() => {
                ImportTarget::Resolved(root.join("case-folder/index.ts"))
            },
            "./CASE-FOLDER" => ImportTarget::Unresolved("./CASE-FOLDER".to_owned()),
            _ => panic!("only fixture imports"),
        };
        assert_eq!(import.target, expected);
    }
}

#[test]
fn retained_index_hints_are_bounded_and_outer_mode_skips_them() {
    let temp = tempfile::tempdir().expect("fixture");
    let root = fs::canonicalize(temp.path()).expect("root");
    let mut path_bytes = 0_u64;
    for number in 0..64 {
        let folder = root.join(format!(
            "source-group-long-enough-to-make-retained-index-paths-dominate-records-{number:02}"
        ));
        fs::create_dir(&folder).expect("folder");
        let path = folder.join("index.ts");
        fs::write(&path, "").expect("empty source");
        path_bytes += u64::try_from(path.as_os_str().len()).expect("finite path length");
    }
    for mode in [ExtractionMode::All, ExtractionMode::WorkspaceOnly] {
        let budget = AnalysisBudget::new(AnalysisLimits {
            max_graph_bytes: path_bytes.checked_mul(3).expect("finite payload quota") / 2,
            ..AnalysisLimits::default()
        })
        .expect("finite budget");
        let result = extractor()
            .with_mode(mode)
            .with_budget(budget.clone())
            .extract(&root);
        if mode == ExtractionMode::All {
            assert!(
                result.is_err(),
                "retained directory hint paths must be admitted"
            );
            assert_eq!(
                budget
                    .check()
                    .expect_err("sticky payload failure")
                    .resource(),
                Resource::GraphBytes
            );
        } else {
            assert!(
                result
                    .expect("outer mode needs no file-resolution hints")
                    .is_empty()
            );
            budget.check().expect("no unused hint reservations");
        }
    }
}

#[cfg(unix)]
#[test]
fn file_links_keep_physical_origin_and_directory_links_stay_pruned() {
    let temp = tempfile::tempdir().expect("fixture");
    let base = fs::canonicalize(temp.path()).expect("base");
    let root = base.join("selected");
    let outside = base.join("outside");
    fs::create_dir_all(root.join("folder")).expect("selected folder");
    fs::create_dir(&outside).expect("outside");
    fs::write(root.join("entry.ts"), "import './folder';").expect("entry");
    fs::write(outside.join("actual.ts"), "import './peer';").expect("physical importer");
    fs::write(outside.join("peer.ts"), "").expect("physical peer");
    std::os::unix::fs::symlink("../../outside/actual.ts", root.join("folder/index.ts"))
        .expect("file link");
    std::os::unix::fs::symlink("../outside", root.join("pruned-link")).expect("directory link");
    std::os::unix::fs::symlink("selected", base.join("selected-link")).expect("root link");
    for operand in [root, base.join("selected-link")] {
        let imports = extractor().extract(&operand).expect("link-aware traversal");
        assert_eq!(imports.len(), 2, "do not descend into directory links");
        for import in imports {
            assert_eq!(
                import.target,
                ImportTarget::Resolved(outside.join(if import.specifier == "./folder" {
                    "actual.ts"
                } else {
                    assert_eq!(import.source, outside.join("actual.ts"));
                    "peer.ts"
                })),
                "linked indices retain canonical target and physical importer origin"
            );
        }
    }
}

#[cfg(unix)]
#[test]
fn special_walk_entries_and_broken_links_fail_before_parsing() {
    let temp = tempfile::tempdir().expect("fixture");
    let fifo = temp.path().join("input.ts");
    assert!(
        std::process::Command::new("mkfifo")
            .arg(&fifo)
            .status()
            .expect("mkfifo")
            .success()
    );
    let error = extractor().extract(temp.path()).expect_err("FIFO rejected");
    assert!(error.to_string().contains("regular"));
    fs::remove_file(fifo).expect("remove FIFO");
    std::os::unix::fs::symlink("missing.ts", temp.path().join("broken.ts"))
        .expect("dangling file link");
    assert!(extractor().extract(temp.path()).is_err());
}
