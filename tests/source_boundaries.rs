//! Consumer regressions for import, discovery, reporting and CLI boundaries.
#![allow(
    clippy::expect_used,
    reason = "Fixture and subprocess failures must fail tests"
)]

use std::fs;
use std::path::Path;
use std::process::Command;

use jscycles::ImportExtractor;
use jscycles::Workspace;
use jscycles::budget::{AnalysisBudget, AnalysisLimits, Resource};
use jscycles::cycles::{PackageCycle, PackageCycleWithFiles};
use jscycles::imports::{ExtractionMode, ImportTarget};
use jscycles::output::{
    CycleFilter, OutputFormatter as _, Results, UnifiedOutputFormatter as _, UnifiedResults,
};
use jscycles::{Config, PackageDiscovery};
use jscycles::{Cycle, HumanFormatter, JsonFormatter, PackageResult};

#[test]
fn relative_specifiers_cannot_become_workspace_package_edges() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let root = directory.path();
    fs::write(root.join("entry.ts"), "import './local'; import 'actual';").expect("source");
    let workspace = Workspace {
        root: root.to_path_buf(),
        format: jscycles::workspace::WorkspaceFormat::Npm,
        packages: [".", "actual"]
            .map(|name| (name.to_owned(), root.to_path_buf()))
            .into_iter()
            .collect(),
        aliases: std::collections::HashMap::new(),
    };
    let imports = ImportExtractor::new(vec!["ts".to_owned()], None)
        .with_workspace(Some(workspace))
        .with_mode(ExtractionMode::WorkspaceOnly)
        .extract(root)
        .expect("sources");
    assert_eq!(
        imports.len(),
        1,
        "relative imports do not become package edges"
    );
    assert_eq!(
        imports[0].specifier, "actual",
        "bare package import retained"
    );
}

#[cfg(unix)]
#[test]
fn workspace_pattern_prefixes_classify_the_target_of_a_symlink() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let root = directory.path();
    fs::write(root.join("package.json"), r#"{"workspaces":["linked/*"]}"#).expect("workspace");
    fs::create_dir_all(root.join("physical/member")).expect("member directory");
    fs::write(
        root.join("physical/member/package.json"),
        r#"{"name":"member"}"#,
    )
    .expect("member manifest");
    std::os::unix::fs::symlink("physical", root.join("linked")).expect("directory symlink");
    let workspace = Workspace::discover(root)
        .expect("symlink prefix is a directory")
        .expect("workspace");
    assert!(
        workspace.packages.contains_key("member"),
        "classify and walk the selected prefix"
    );
}

#[test]
fn legacy_report_totals_and_human_summaries_describe_the_actual_packages() {
    let mut results = Results::new();
    assert_eq!(
        results.packages_with_cycles(),
        0,
        "empty results have no cyclic packages"
    );
    for (name, count) in [("clean", 0), ("single", 1), ("multiple", 2)] {
        results.packages.push(PackageResult {
            name: name.to_owned(),
            path: name.into(),
            cycles: (0..count)
                .map(|_| Cycle::new(["a", "b", "a"].map(std::path::PathBuf::from).to_vec()))
                .collect(),
        });
    }
    assert_eq!(
        results.packages_with_cycles(),
        2,
        "count cyclic packages rather than cycles"
    );
    assert_eq!(results.total_cycles(), 3, "retain every cycle");
    let mut output = Vec::new();
    HumanFormatter::new(false)
        .format(&results, &mut output)
        .expect("human report");
    let text = String::from_utf8(output).expect("UTF-8 report");
    assert!(
        text.contains("Summary: 3 packages checked, 2 with cycles, 3 total cycles"),
        "legacy summaries retain their totals"
    );
}

#[test]
fn human_package_counts_agree_with_the_json_cycle_arrays() {
    for detailed in [false, true] {
        for count in [1, 2] {
            let mut results = UnifiedResults::new();
            for _ in 0..count {
                let packages = vec!["a".to_owned(), "b".to_owned(), "a".to_owned()];
                if detailed {
                    results
                        .package_cycles_with_files
                        .push(PackageCycleWithFiles {
                            packages,
                            edges: Vec::new(),
                        });
                } else {
                    results.package_cycles.push(PackageCycle::new(packages));
                }
            }
            let mut output = Vec::new();
            HumanFormatter::new(false)
                .format_unified(&results, CycleFilter::Outer, &mut output)
                .expect("human report");
            let word = if count == 1 { "cycle" } else { "cycles" };
            assert!(
                String::from_utf8(output)
                    .expect("UTF-8 report")
                    .contains(&format!("{count} inter-package {word}\n")),
                "count and noun agree for detailed={detailed}"
            );
            let mut json = Vec::new();
            JsonFormatter::new()
                .format_unified(&results, CycleFilter::Outer, &mut json)
                .expect("JSON report");
            let report: serde_json::Value = serde_json::from_slice(&json).expect("JSON");
            assert_eq!(
                report["package_cycles"]
                    .as_array()
                    .expect("cycle array")
                    .len(),
                count,
                "human and machine counts agree"
            );
        }
    }
}

#[test]
fn formatter_write_errors_are_never_successful_reports() {
    struct RejectWrites;
    impl std::io::Write for RejectWrites {
        fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
            Err(std::io::ErrorKind::BrokenPipe.into())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let results = Results {
        packages: vec![PackageResult {
            name: "clean".to_owned(),
            path: ".".into(),
            cycles: Vec::new(),
        }],
    };
    assert!(
        HumanFormatter::new(false)
            .format(&results, &mut RejectWrites)
            .is_err(),
        "even a summary-only write must propagate failure"
    );
    assert!(
        JsonFormatter::new()
            .format(&results, &mut RejectWrites)
            .is_err(),
        "legacy JSON write failure"
    );
    for filter in [CycleFilter::All, CycleFilter::Inner, CycleFilter::Outer] {
        assert!(
            HumanFormatter::new(false)
                .format_unified(&UnifiedResults::new(), filter, &mut RejectWrites)
                .is_err(),
            "human {filter:?} write failure"
        );
        assert!(
            JsonFormatter::new()
                .format_unified(&UnifiedResults::new(), filter, &mut RejectWrites)
                .is_err(),
            "JSON {filter:?} write failure"
        );
    }
}

#[test]
fn automatic_include_patterns_do_not_cross_component_boundaries() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let root = directory.path();
    for (path, name) in [
        ("packages/app/src", "selected"),
        ("packages/app/deeper/src", "outside"),
    ] {
        fs::create_dir_all(root.join(path)).expect("package directory");
        fs::write(
            root.join(path).join("package.json"),
            serde_json::json!({"name":name}).to_string(),
        )
        .expect("manifest");
    }
    let config = Config::parse("scan:\n  include: ['packages/*/src']\n").expect("configuration");
    let packages = PackageDiscovery::new(&config, &[], &[])
        .expect("selectors")
        .discover(root)
        .expect("discovery");
    assert_eq!(
        packages
            .iter()
            .map(|package| package.name.as_str())
            .collect::<Vec<_>>(),
        ["selected"],
        "a single wildcard selects one path component"
    );
}

#[test]
fn discovered_packages_retain_the_selected_overrides_and_ignore_patterns() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let root = directory.path();
    for name in ["app-one", "other"] {
        fs::create_dir_all(root.join(name)).expect("package directory");
        fs::write(
            root.join(name).join("package.json"),
            serde_json::json!({"name":name}).to_string(),
        )
        .expect("manifest");
    }
    let config = Config::parse(
        "packages:\n  'app-*':\n    extensions: [jsx]\n    ignore: ['**/ignored.ts']\n",
    )
    .expect("configuration");
    let packages = PackageDiscovery::new(&config, &[], &[])
        .expect("selectors")
        .discover_explicit(&[root.to_path_buf()])
        .expect("discovery");
    let app = packages
        .iter()
        .find(|package| package.name == "app-one")
        .expect("matching package");
    assert_eq!(
        app.config.extensions.as_deref(),
        Some(["jsx".to_owned()].as_slice()),
        "extensions survive discovery"
    );
    assert_eq!(
        app.config.ignore.as_deref(),
        Some(["**/ignored.ts".to_owned()].as_slice()),
        "ignores survive discovery"
    );
    let other = packages
        .iter()
        .find(|package| package.name == "other")
        .expect("unmatched package");
    assert_ne!(
        other.config.extensions, app.config.extensions,
        "unmatched packages use defaults"
    );
    fs::write(root.join("app-one/ignored.ts"), "import './other';").expect("ignored source");
    let imports = ImportExtractor::new(vec!["ts".to_owned()], None)
        .with_ignore_patterns(app.config.ignore.clone().expect("selected ignores"))
        .extract(&root.join("app-one"))
        .expect("extraction");
    assert!(
        imports.is_empty(),
        "ignored source must not become graph edges"
    );
}

#[test]
fn external_import_targets_are_admitted_as_owned_payloads() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let specifier = "x".repeat(8192);
    fs::write(
        directory.path().join("entry.ts"),
        format!("import '{specifier}';"),
    )
    .expect("source");
    let budget = AnalysisBudget::new(AnalysisLimits {
        max_graph_bytes: 12_000,
        ..AnalysisLimits::default()
    })
    .expect("positive limit");
    let result = ImportExtractor::new(vec!["ts".to_owned()], None)
        .with_budget(budget.clone())
        .extract(directory.path());
    assert!(
        result.is_err(),
        "the decoded specifier and owned external target cannot both fit"
    );
    assert_eq!(
        budget.check().expect_err("exhaustion").resource(),
        Resource::GraphBytes,
        "owned target payload has its own reservation"
    );
}

#[test]
fn relative_modules_are_normalized_before_filesystem_lookup() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let root = directory.path();
    fs::write(root.join("entry.ts"), "import './not-created/../target';").expect("entry");
    fs::write(root.join("target.ts"), "import './entry';").expect("target");
    let imports = ImportExtractor::new(vec!["ts".to_owned()], None)
        .extract(root)
        .expect("valid sources");
    assert!(imports.iter().any(|import| import.specifier=="./not-created/../target" && matches!(&import.target, ImportTarget::Resolved(path) if path.ends_with("target.ts"))), "dot segments resolve lexically even when a removed component does not exist");
}

#[test]
fn no_tsconfig_disables_file_alias_resolution() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let root = directory.path();
    fs::write(root.join("package.json"), r#"{"name":"flags"}"#).expect("manifest");
    fs::write(
        root.join("tsconfig.json"),
        r#"{"compilerOptions":{"paths":{"@local/*":["*"]}}}"#,
    )
    .expect("tsconfig");
    fs::write(root.join("a.ts"), "import '@local/b';").expect("alias source");
    fs::write(root.join("b.ts"), "import './a';").expect("cycle source");
    for (extra, status) in [(vec![], 1), (vec!["--no-tsconfig"], 0)] {
        let result = Command::new(env!("CARGO_BIN_EXE_jscycles"))
            .current_dir(root)
            .args(["--inner", "--json"])
            .args(extra)
            .output()
            .expect("CLI starts");
        assert_eq!(
            result.status.code(),
            Some(status),
            "automatic file aliases respect the flag; stderr={}",
            String::from_utf8_lossy(&result.stderr)
        );
    }
}

#[cfg(unix)]
#[test]
fn workspace_markers_may_be_symlinks_to_regular_files() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let root = directory.path();
    fs::write(
        root.join("workspace-marker.json"),
        r#"{"workspaces":["packages/*"]}"#,
    )
    .expect("marker target");
    std::os::unix::fs::symlink("workspace-marker.json", root.join("package.json"))
        .expect("file symlink");
    fs::create_dir_all(root.join("packages/member")).expect("member");
    fs::write(
        root.join("packages/member/package.json"),
        r#"{"name":"member"}"#,
    )
    .expect("member manifest");
    let workspace = Workspace::discover(root)
        .expect("regular file symlink is allowed")
        .expect("workspace");
    assert!(
        workspace.packages.contains_key("member"),
        "follow the marker identity before classifying its file type"
    );
}

fn index_fixture(root: &Path, direct_first: bool) {
    fs::create_dir_all(root.join("folder")).expect("source directory");
    fs::write(root.join("package.json"), r#"{"name":"cache-boundary"}"#).expect("manifest");
    let entry = if direct_first {
        "import './folder/__index__'; import './folder';"
    } else {
        "import './folder'; import './folder/__index__';"
    };
    fs::write(root.join("entry.ts"), entry).expect("entry source");
    fs::write(
        root.join("folder/index.ts"),
        if direct_first {
            "import '../entry';"
        } else {
            "export {};"
        },
    )
    .expect("directory entry");
    fs::write(
        root.join("folder/__index__.ts"),
        if direct_first {
            "export {};"
        } else {
            "import '../entry';"
        },
    )
    .expect("literal index name");
}

#[test]
fn directory_resolution_and_literal_index_names_have_distinct_cache_entries() {
    for direct_first in [false, true] {
        let directory = tempfile::tempdir().expect("temporary directory");
        index_fixture(directory.path(), direct_first);
        let root = directory.path().canonicalize().expect("root identity");
        let imports = ImportExtractor::new(vec!["ts".to_owned()], None)
            .extract(&root)
            .expect("valid sources");
        for (specifier, target) in [
            ("./folder", "folder/index.ts"),
            ("./folder/__index__", "folder/__index__.ts"),
        ] {
            let import = imports
                .iter()
                .find(|import| {
                    import.source == root.join("entry.ts") && import.specifier == specifier
                })
                .expect("entry import");
            assert_eq!(
                import.target,
                ImportTarget::Resolved(root.join(target)),
                "distinct resolution kinds for {specifier}, direct_first={direct_first}"
            );
        }
    }
}

#[test]
fn index_cache_aliasing_cannot_turn_a_real_cycle_into_clean_status() {
    for direct_first in [false, true] {
        let directory = tempfile::tempdir().expect("temporary directory");
        index_fixture(directory.path(), direct_first);
        let result = Command::new(env!("CARGO_BIN_EXE_jscycles"))
            .current_dir(directory.path())
            .args(["--inner", "--no-tsconfig", "--json"])
            .output()
            .expect("CLI starts");
        assert_eq!(
            result.status.code(),
            Some(1),
            "a real cycle must survive resolution caching; direct_first={direct_first}; stderr={}",
            String::from_utf8_lossy(&result.stderr)
        );
        let report: serde_json::Value =
            serde_json::from_slice(&result.stdout).expect("JSON report");
        assert_eq!(
            report["has_file_cycles"], true,
            "file-level cycle is reported"
        );
    }
}

#[test]
fn directory_index_resolution_skips_nonregular_candidates() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let root = directory.path();
    fs::create_dir_all(root.join("folder/index.ts")).expect("directory with a source-like name");
    fs::write(root.join("package.json"), r#"{"name":"index-kind"}"#).expect("manifest");
    fs::write(root.join("entry.ts"), "import './folder';").expect("entry");
    fs::write(root.join("folder/index.tsx"), "import '../entry';").expect("real directory entry");
    let result = Command::new(env!("CARGO_BIN_EXE_jscycles"))
        .current_dir(root)
        .args(["--inner", "--no-tsconfig", "--json"])
        .output()
        .expect("CLI starts");
    assert_eq!(
        result.status.code(),
        Some(1),
        "a directory named index.ts cannot hide the cycle through index.tsx"
    );
}

#[test]
fn javascript_extensions_reject_typescript_annotations() {
    for extension in ["js", "jsx", "mjs", "cjs", "ts", "tsx"] {
        let directory = tempfile::tempdir().expect("temporary directory");
        let root = directory.path();
        fs::write(root.join("package.json"), r#"{"name":"language-boundary"}"#).expect("manifest");
        fs::write(
            root.join(format!("source.{extension}")),
            "export const value: number = 1;",
        )
        .expect("typed source");
        let result = Command::new(env!("CARGO_BIN_EXE_jscycles"))
            .current_dir(root)
            .args([
                "--inner",
                "--no-tsconfig",
                "--json",
                "--extensions",
                extension,
            ])
            .output()
            .expect("CLI starts");
        let expected = if ["ts", "tsx"].contains(&extension) {
            0
        } else {
            2
        };
        assert_eq!(
            result.status.code(),
            Some(expected),
            "grammar must match .{extension}"
        );
    }
}
