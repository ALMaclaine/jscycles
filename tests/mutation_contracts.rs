//! Consumer contracts added after mutation analysis of graph and metadata paths.

use jscycles::budget::{AnalysisBudget, AnalysisLimits, Resource};
use jscycles::cycles::PackageCycleDetector;
use jscycles::graph::PackageDependencyGraph;
use jscycles::{Config, TsConfig, TsConfigError};
use jscycles::{TsConfigCache, Workspace};
use std::sync::Arc;

#[test]
fn package_graph_retains_workspace_members_without_any_imports() {
    let imports = ["empty-a", "empty-b", "empty-c"]
        .map(|name| (name.to_owned(), Vec::new()))
        .into_iter()
        .collect();
    let graph = PackageDependencyGraph::from_imports(&imports);
    assert_eq!(
        graph.node_count(),
        3,
        "all workspace members remain observable"
    );
    assert_eq!(graph.edge_count(), 0, "no invented dependency edges");
    for name in ["empty-a", "empty-b", "empty-c"] {
        assert!(
            graph.nodes().contains(name),
            "isolated member identity retained"
        );
    }
}

#[test]
fn cached_discovery_keeps_its_selected_parent_until_a_new_cache_is_created() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let root = directory.path();
    let child = root.join("child");
    std::fs::create_dir(&child).expect("child directory");
    std::fs::write(root.join("tsconfig.json"), "{}").expect("parent config");
    let cache = TsConfigCache::new();
    let initial = cache.discover_shared(&child).expect("parent selected");
    std::fs::write(child.join("tsconfig.json"), "{}").expect("new nearer config");
    let repeated = cache.discover_shared(&child).expect("cached selection");
    assert!(
        Arc::ptr_eq(&initial, &repeated),
        "directory selection is part of the immutable snapshot"
    );
    let refreshed = TsConfigCache::new()
        .discover_shared(&child)
        .expect("fresh child selection");
    assert_eq!(
        refreshed.base_url,
        child.canonicalize().expect("child identity"),
        "new caches see newly created metadata"
    );
    assert_ne!(refreshed.base_url, initial.base_url, "refresh is explicit");
}

#[test]
fn cached_metadata_does_not_escape_an_exhausted_discovery_budget() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let child = directory.path().join("child");
    std::fs::create_dir(&child).expect("child directory");
    let path = directory.path().join("tsconfig.json");
    std::fs::write(&path, "{}").expect("parent config");
    for resource in [Resource::Work, Resource::GraphBytes] {
        let mut failures = 0;
        for headroom in [0, 1, 2, 3, 4, 8, 16, 32, 64, 128, 256, 384, 512] {
            let budget = AnalysisBudget::new(AnalysisLimits {
                max_work: 10_000,
                max_graph_bytes: 10_000,
                ..AnalysisLimits::default()
            })
            .expect("positive limits");
            let cache = TsConfigCache::new().with_budget(budget.clone());
            assert!(
                cache.load(&path).is_some(),
                "prime the parsed-config cache without directory entries"
            );
            let reserve = budget.limits().limit(resource) - budget.used(resource) - headroom;
            budget
                .consume(resource, reserve)
                .expect("leave selected headroom");
            let result = cache.discover_shared(&child);
            if let Err(error) = budget.check() {
                failures += 1;
                assert_eq!(error.resource(), resource, "selected resource exhausts");
                assert!(
                    result.is_none(),
                    "cached success cannot bypass an observed {resource} failure with {headroom} units remaining"
                );
            } else {
                assert!(
                    result.is_some(),
                    "successful discovery keeps the cached config"
                );
            }
        }
        assert!(
            failures > 0,
            "headroom sweep exercises {resource} exhaustion"
        );
    }
}

#[test]
fn alias_substitution_retains_components_after_the_wildcard() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let target = directory.path().join("src/widget/entry.ts");
    std::fs::create_dir_all(target.parent().expect("target parent")).expect("source directory");
    std::fs::write(&target, "export {};").expect("source file");
    let path = directory.path().join("tsconfig.json");
    std::fs::write(
        &path,
        r#"{"compilerOptions":{"paths":{"@entry/*":["src/*/entry.ts"]}}}"#,
    )
    .expect("mapping");
    assert_eq!(
        TsConfig::load(&path)
            .expect("configuration")
            .resolve_alias("@entry/widget"),
        Some(target.canonicalize().expect("target identity")),
        "suffix path components survive substitution"
    );
}

#[test]
fn extensionless_extends_supports_directory_configs_and_json_file_precedence() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let root = directory.path().canonicalize().expect("root identity");
    std::fs::create_dir(root.join("base")).expect("base directory");
    std::fs::write(
        root.join("base/tsconfig.json"),
        r#"{"compilerOptions":{"baseUrl":"dir-source"}}"#,
    )
    .expect("directory config");
    let child = root.join("tsconfig.json");
    std::fs::write(&child, r#"{"extends":"./base"}"#).expect("child config");
    assert_eq!(
        TsConfig::load(&child).expect("directory extends").base_url,
        root.join("base/dir-source"),
        "directory parents resolve their own declaration origin"
    );
    std::fs::write(
        root.join("base.json"),
        r#"{"compilerOptions":{"baseUrl":"file-source"}}"#,
    )
    .expect("json sibling");
    assert_eq!(
        TsConfig::load(&child).expect("file precedence").base_url,
        root.join("file-source"),
        "extension completion precedes directory fallback"
    );
}

#[test]
fn workspace_patterns_support_question_classes_and_literal_braces() {
    for (pattern, expected) in [
        ("packages/pkg?", vec!["pkg1", "pkg2"]),
        ("packages/pkg[12]", vec!["pkg1", "pkg2"]),
        ("packages/{literal}", vec!["{literal}"]),
    ] {
        let directory = tempfile::tempdir().expect("temporary directory");
        let root = directory.path();
        std::fs::write(
            root.join("package.json"),
            serde_json::json!({"workspaces": [pattern]}).to_string(),
        )
        .expect("workspace manifest");
        for name in ["pkg1", "pkg2", "pkg10", "{literal}", "ignored"] {
            let package = root.join("packages").join(name);
            std::fs::create_dir_all(&package).expect("package directory");
            if name == "ignored" {
                std::fs::create_dir(package.join("package.json"))
                    .expect("coincidentally named directory");
            } else {
                std::fs::write(
                    package.join("package.json"),
                    serde_json::json!({"name": name}).to_string(),
                )
                .expect("package manifest");
            }
        }
        let workspace = Workspace::discover(root)
            .expect("workspace discovery")
            .expect("workspace exists");
        let mut actual: Vec<_> = workspace.packages.keys().map(String::as_str).collect();
        actual.sort_unstable();
        assert_eq!(actual, expected, "membership for {pattern}");
    }
}

#[test]
fn package_json_directories_do_not_become_workspace_manifests() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let root = directory.path();
    std::fs::write(
        root.join("package.json"),
        r#"{"workspaces":["packages/*"]}"#,
    )
    .expect("workspace manifest");
    std::fs::create_dir_all(root.join("packages/ignored/package.json"))
        .expect("non-manifest directory");
    let workspace = Workspace::discover(root)
        .expect("coincidental name is not malformed JSON")
        .expect("workspace");
    assert!(
        workspace.packages.is_empty(),
        "only regular manifest files define packages"
    );
}
use jscycles::{CycleDetector, DependencyGraph};

#[test]
fn effective_package_settings_preserve_defaults_globs_and_lexical_precedence() {
    let config = Config::parse(
        r#"
defaults:
  extensions: [ts]
packages:
  "@org/*":
    extensions: [tsx]
    ignore: ["**/generated/**", "**/*.spec.ts"]
  "@org/app":
    extensions: [js]
  "plain":
    extensions: [jsx]
    ignore: []
"#,
    )
    .expect("valid configuration");
    assert_eq!(
        config.extensions_for_package("unmatched"),
        ["ts"],
        "defaults apply outside configured patterns"
    );
    assert_eq!(
        config.ignore_for_package("unmatched"),
        None,
        "no implicit ignore override"
    );
    assert_eq!(
        config.extensions_for_package("@org/app"),
        ["tsx"],
        "the lexical first matching pattern wins"
    );
    assert_eq!(
        config.ignore_for_package("@org/lib"),
        Some(vec![
            "**/generated/**".to_owned(),
            "**/*.spec.ts".to_owned()
        ]),
        "preserve ignore values and order"
    );
    assert_eq!(
        config.extensions_for_package("plain"),
        ["jsx"],
        "exact package override applies"
    );
    assert_eq!(
        config.ignore_for_package("plain"),
        Some(vec![]),
        "explicit empty ignore list differs from absence"
    );
}

#[test]
fn config_text_accepts_the_exact_size_limit_and_rejects_one_more_byte() {
    let mut text = "{}\n#".to_owned();
    text.push_str(&" ".repeat(16 * 1024 * 1024 - text.len()));
    assert!(
        Config::parse(&text).is_ok(),
        "a valid exact-limit configuration is accepted"
    );
    text.push(' ');
    assert!(
        matches!(Config::parse(&text), Err(jscycles::ConfigError::Invalid(_))),
        "oversized valid YAML is a size error"
    );
}

#[test]
fn configuration_loading_distinguishes_absence_errors_and_best_effort() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let path = directory.path().join("jscycles.yaml");
    assert_eq!(
        Config::try_load_or_default(&path)
            .expect("absent config")
            .defaults
            .extensions,
        Config::default().defaults.extensions,
        "absence selects defaults"
    );
    std::fs::write(&path, "defaults:\n  extensions: [mjs]\n").expect("valid config");
    assert_eq!(
        Config::load_or_default(&path).defaults.extensions,
        ["mjs"],
        "the compatibility loader must actually load valid settings"
    );
    assert!(
        Config::try_load_or_default(&path.join("child")).is_err(),
        "a non-directory prefix is an input error"
    );
    std::fs::write(&path, "defaults: [").expect("malformed config");
    assert!(
        Config::try_load_or_default(&path).is_err(),
        "malformed existing input is not absence"
    );
    assert_eq!(
        Config::load_or_default(&path).defaults.extensions,
        Config::default().defaults.extensions,
        "legacy best effort retains its documented fallback"
    );
}

#[test]
fn selected_aliases_reject_unsupported_patterns_and_target_cardinality() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let path = directory.path().join("tsconfig.json");
    for (pattern, replacements) in [
        ("@bad*tail", vec!["src/*"]),
        ("@bad**", vec!["src/*"]),
        ("@empty", vec![]),
        ("@wild/*", vec!["src/plain"]),
        ("@exact", vec!["src/*"]),
        ("@wild/*", vec!["src/**/*"]),
    ] {
        let text = serde_json::json!({"compilerOptions": {"paths": {pattern: replacements}}});
        std::fs::write(&path, text.to_string()).expect("write selected config");
        assert!(
            matches!(TsConfig::load(&path), Err(TsConfigError::InvalidPath(_))),
            "unsupported selected mapping {pattern}"
        );
    }
    for (pattern, replacements) in [
        ("@exact", vec!["src/plain"]),
        ("@wild/*", vec!["src/*", "fallback/*"]),
    ] {
        let text = serde_json::json!({"compilerOptions": {"paths": {pattern: replacements}}});
        std::fs::write(&path, text.to_string()).expect("write valid selected config");
        assert_eq!(
            TsConfig::load(&path)
                .expect("supported mapping")
                .paths
                .len(),
            1,
            "valid mapping retained"
        );
    }
}

#[test]
fn public_detectors_return_cycles_in_lexical_order() {
    let expected = vec![vec!["a", "b", "a"], vec!["x", "y", "x"]];
    for connect in [false, true] {
        let mut files = DependencyGraph::new();
        let mut packages = PackageDependencyGraph::new();
        for (from, to) in [("y", "x"), ("b", "a"), ("x", "y"), ("a", "b")] {
            files.add_edge(PathBuf::from(from), PathBuf::from(to));
            packages.add_edge(from.to_owned(), to.to_owned());
        }
        if connect {
            files.add_edge(PathBuf::from("b"), PathBuf::from("x"));
            packages.add_edge("b".to_owned(), "x".to_owned());
        }
        let found: Vec<Vec<String>> = CycleDetector::detect(&files)
            .into_iter()
            .map(|cycle| {
                cycle
                    .path
                    .into_iter()
                    .map(|path| path.to_string_lossy().into_owned())
                    .collect()
            })
            .collect();
        assert_eq!(
            found, expected,
            "file SCC completion order is not report order"
        );
        let package_cycles: Vec<_> = PackageCycleDetector::detect(&packages)
            .into_iter()
            .map(|cycle| cycle.packages)
            .collect();
        assert_eq!(
            package_cycles, expected,
            "package traversal order is not report order"
        );
    }
}

#[test]
fn multi_file_components_prefer_a_multi_file_witness_over_a_self_import() {
    let mut graph = DependencyGraph::new();
    for (from, to) in [("a", "a"), ("a", "b"), ("b", "a")] {
        graph.add_edge(PathBuf::from(from), PathBuf::from(to));
    }
    let found = CycleDetector::detect(&graph);
    assert_eq!(found.len(), 1, "one cyclic component");
    assert_eq!(
        found[0].path,
        ["a", "b", "a"].map(PathBuf::from),
        "a self import must not hide a cycle between files"
    );
}
use std::path::PathBuf;

#[test]
fn long_graph_labels_cannot_bypass_payload_admission() {
    let file = PathBuf::from("😀".repeat(256));
    let mut files = DependencyGraph::new();
    files.add_edge(file.clone(), file);
    let budget = AnalysisBudget::new(AnalysisLimits {
        max_graph_bytes: 768,
        ..AnalysisLimits::default()
    })
    .expect("positive limit");
    assert_eq!(
        CycleDetector::try_detect(&files, &budget)
            .expect_err("label cannot fit")
            .resource(),
        Resource::GraphBytes,
        "file labels are retained payload"
    );
    let name = "😀".repeat(256);
    let mut packages = PackageDependencyGraph::new();
    packages.add_edge(name.clone(), "other".to_owned());
    packages.add_edge("other".to_owned(), name);
    let package_budget = AnalysisBudget::new(AnalysisLimits {
        max_graph_bytes: 768,
        ..AnalysisLimits::default()
    })
    .expect("positive limit");
    assert_eq!(
        PackageCycleDetector::try_detect(&packages, &package_budget)
            .expect_err("label cannot fit")
            .resource(),
        Resource::GraphBytes,
        "package labels are retained payload"
    );
}

#[test]
fn long_cycle_labels_cannot_bypass_result_payload_admission() {
    let mut files = DependencyGraph::new();
    let file = PathBuf::from("😀".repeat(256));
    files.add_edge(file.clone(), file);
    let budget = AnalysisBudget::new(AnalysisLimits {
        max_cycle_bytes: 768,
        ..AnalysisLimits::default()
    })
    .expect("positive limit");
    assert_eq!(
        CycleDetector::try_detect(&files, &budget)
            .expect_err("cycle label cannot fit")
            .resource(),
        Resource::CycleBytes,
        "returned paths must admit label bytes"
    );
    let name = "😀".repeat(256);
    let mut packages = PackageDependencyGraph::new();
    packages.add_edge(name.clone(), "other".to_owned());
    packages.add_edge("other".to_owned(), name);
    let package_budget = AnalysisBudget::new(AnalysisLimits {
        max_cycle_bytes: 768,
        ..AnalysisLimits::default()
    })
    .expect("positive limit");
    assert_eq!(
        PackageCycleDetector::try_detect(&packages, &package_budget)
            .expect_err("cycle label cannot fit")
            .resource(),
        Resource::CycleBytes,
        "returned package cycles must admit label bytes"
    );
}
