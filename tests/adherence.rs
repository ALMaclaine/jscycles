//! Regressions found by the complete Rust-OE contract review.
#![allow(
    clippy::expect_used,
    reason = "Filesystem/process setup failures must fail tests"
)]
use std::fs;
use std::path::Path;
use std::process::{Command, Output};

fn scan(root: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_jscycles"))
        .current_dir(root)
        .args(args)
        .output()
        .expect("CLI starts")
}
fn cycle(root: &Path) {
    fs::create_dir_all(root).expect("source directory");
    fs::write(root.join("package.json"), r#"{"name":"example"}"#).expect("manifest");
    fs::write(root.join("a.ts"), "import './b';").expect("source a");
    fs::write(root.join("b.ts"), "import './a';").expect("source b");
}

#[test]
fn blank_and_comment_only_configuration_use_defaults_without_hiding_cycles() {
    let temp = tempfile::tempdir().expect("directory");
    cycle(temp.path());
    for yaml in ["", "  \n\n", "# no overrides\n"] {
        fs::write(temp.path().join("jscycles.yaml"), yaml).expect("configuration");
        let output = scan(temp.path(), &["--inner", "--json"]);
        assert_eq!(
            output.status.code(),
            Some(1),
            "blank configuration retains the default source graph: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let report: serde_json::Value = serde_json::from_slice(&output.stdout).expect("JSON");
        assert_eq!(
            report["has_file_cycles"], true,
            "the cycle is still visible"
        );
    }
}

#[test]
fn explicit_null_and_wrong_configuration_shapes_are_errors() {
    let temp = tempfile::tempdir().expect("directory");
    cycle(temp.path());
    for yaml in [
        "null",
        "---\n",
        "[]",
        "defaults: null",
        "defaults: []",
        "scan:\n",
    ] {
        fs::write(temp.path().join("jscycles.yaml"), yaml).expect("configuration");
        let output = scan(temp.path(), &["--inner", "--json"]);
        assert_eq!(output.status.code(), Some(2), "invalid shape: {yaml:?}");
        assert!(
            output.stdout.is_empty(),
            "invalid settings publish no report"
        );
        assert!(!output.stderr.is_empty(), "the error is diagnosed");
    }
}

#[test]
fn invalid_package_ignore_patterns_are_errors_even_for_unselected_packages() {
    let temp = tempfile::tempdir().expect("directory");
    cycle(temp.path());
    fs::write(
        temp.path().join("jscycles.yaml"),
        "packages:\n  missing:\n    ignore: ['[']\n",
    )
    .expect("config");
    let output = scan(temp.path(), &["--inner", "--json"]);
    assert_eq!(
        output.status.code(),
        Some(2),
        "invalid config cannot be discarded"
    );
    assert!(output.stdout.is_empty(), "no report");
}
#[test]
fn parser_recovery_is_not_a_completed_scan() {
    let temp = tempfile::tempdir().expect("directory");
    cycle(temp.path());
    fs::write(temp.path().join("a.ts"), "import { broken from './b';").expect("broken source");
    let output = scan(temp.path(), &["--inner", "--json"]);
    assert_eq!(
        output.status.code(),
        Some(2),
        "recovered syntax tree cannot establish clean status"
    );
    assert!(output.stdout.is_empty(), "no partial graph report");
}
#[test]
fn escaped_import_specifiers_use_their_javascript_value() {
    let temp = tempfile::tempdir().expect("directory");
    cycle(temp.path());
    for source in [r"import './\u0062';", r"import './\u{000000062}';"] {
        fs::write(temp.path().join("a.ts"), source).expect("escaped source");
        let output = scan(temp.path(), &["--inner", "--json"]);
        assert_eq!(
            output.status.code(),
            Some(1),
            "escaped b still participates in the cycle"
        );
    }
}

#[test]
fn braced_surrogate_escapes_can_form_a_valid_unicode_module_value() {
    let temp = tempfile::tempdir().expect("directory");
    cycle(temp.path());
    fs::rename(temp.path().join("b.ts"), temp.path().join("\u{10000}.ts")).expect("Unicode module");
    fs::write(temp.path().join("a.ts"), r"import './\u{d800}\u{dc00}';").expect("escaped pair");
    let output = scan(temp.path(), &["--inner", "--json"]);
    assert_eq!(
        output.status.code(),
        Some(1),
        "validate the final UTF-16 value rather than rejecting a paired code unit"
    );
}
#[test]
fn vendor_substrings_do_not_exclude_unrelated_checkout_names() {
    let temp = tempfile::tempdir().expect("directory");
    let root = temp.path().join("node_modules-analysis");
    cycle(&root);
    let output = scan(&root, &["--inner", "--json"]);
    assert_eq!(
        output.status.code(),
        Some(1),
        "only actual vendor directory components excluded"
    );
}
#[test]
fn inherited_aliases_keep_the_declaring_configs_directory() {
    let temp = tempfile::tempdir().expect("directory");
    cycle(temp.path());
    fs::create_dir(temp.path().join("configs")).expect("configs");
    fs::write(
        temp.path().join("configs/base.json"),
        r#"{"compilerOptions":{"baseUrl":"..","paths":{"@b":["b.ts"]}}}"#,
    )
    .expect("base config");
    fs::write(
        temp.path().join("tsconfig.json"),
        r#"{"extends":"./configs/base.json"}"#,
    )
    .expect("child config");
    fs::write(temp.path().join("a.ts"), "import '@b';").expect("alias source");
    let output = scan(
        temp.path(),
        &["--inner", "--json", "--tsconfig", "tsconfig.json"],
    );
    assert_eq!(
        output.status.code(),
        Some(1),
        "inherited relative baseUrl has origin semantics"
    );
}
#[test]
fn tsconfig_comments_and_trailing_commas_preserve_valid_aliases() {
    let temp = tempfile::tempdir().expect("directory");
    cycle(temp.path());
    fs::write(
        temp.path().join("tsconfig.json"),
        "{\n// aliases\n\"compilerOptions\":{\"paths\":{\"@b\":[\"b.ts\",],},},\n}",
    )
    .expect("JSONC config");
    fs::write(temp.path().join("a.ts"), "import '@b';").expect("alias source");
    let output = scan(
        temp.path(),
        &["--inner", "--json", "--tsconfig", "tsconfig.json"],
    );
    assert_eq!(
        output.status.code(),
        Some(1),
        "valid ecosystem syntax must not lose edges: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}
#[test]
fn stdin_and_positional_inputs_cannot_silently_override_each_other() {
    let temp = tempfile::tempdir().expect("directory");
    cycle(temp.path());
    let output = scan(temp.path(), &[".", "--stdin", "--inner", "--json"]);
    assert_eq!(
        output.status.code(),
        Some(2),
        "ambiguous explicit input selection rejected"
    );
    assert!(output.stdout.is_empty(), "no successful empty stdin scan");
}
#[test]
fn computed_dynamic_imports_are_not_mistaken_for_literal_edges() {
    let temp = tempfile::tempdir().expect("directory");
    cycle(temp.path());
    fs::write(
        temp.path().join("a.ts"),
        "const suffix = '.ts'; import('./b' + suffix);",
    )
    .expect("computed import");
    let imports = jscycles::ImportExtractor::new(vec!["ts".to_owned()], None)
        .extract(temp.path())
        .expect("valid source");
    assert!(
        !imports
            .iter()
            .any(|import| import.specifier.contains("+ suffix")),
        "computed expressions are not literal module specifiers"
    );
}

#[test]
fn sequential_extractions_invalidate_filesystem_resolution_cache() {
    let temp = tempfile::tempdir().expect("directory");
    fs::write(temp.path().join("a.ts"), "import './b';").expect("source a");
    let extractor = jscycles::ImportExtractor::new(vec!["ts".to_owned(), "tsx".to_owned()], None);
    let _first = extractor.extract(temp.path()).expect("initial scan");
    fs::write(temp.path().join("b.tsx"), "import './a';").expect("new source b");
    let imports = extractor.extract(temp.path()).expect("new scan");
    assert_eq!(
        jscycles::CycleDetector::detect(&jscycles::DependencyGraph::from_imports(&imports)).len(),
        1,
        "new files must not reuse cached misses"
    );
}

#[test]
fn human_output_escapes_controls_in_untrusted_package_names() {
    let temp = tempfile::tempdir().expect("directory");
    cycle(temp.path());
    fs::write(
        temp.path().join("package.json"),
        r#"{"name":"example\u001b]52;clipboard\u0007"}"#,
    )
    .expect("manifest");
    let output = scan(temp.path(), &["--inner"]);
    assert_eq!(output.status.code(), Some(1), "cycle found");
    assert!(
        !output.stdout.contains(&27) && !output.stdout.contains(&7),
        "manifest cannot emit terminal control sequences"
    );
}

#[test]
fn configuration_semantics_are_validated_independently_of_discovery() {
    for text in [
        "limits:\n  max_work: 0",
        "defaults:\n  extensions: ['.ts']",
        "packages:\n  absent:\n    ignore: ['[']",
    ] {
        assert!(
            jscycles::Config::parse(text).is_err(),
            "invalid domain settings rejected by the public ingress"
        );
    }
}

#[test]
fn alias_fallback_targets_and_directory_indexes_are_resolved() {
    let temp = tempfile::tempdir().expect("directory");
    cycle(temp.path());
    fs::create_dir(temp.path().join("module")).expect("module directory");
    fs::rename(
        temp.path().join("b.ts"),
        temp.path().join("module/index.ts"),
    )
    .expect("directory index");
    fs::write(temp.path().join("module/index.ts"), "import '../a';").expect("back edge");
    fs::write(temp.path().join("a.ts"), "import '@b';").expect("alias");
    fs::write(
        temp.path().join("tsconfig.json"),
        r#"{"compilerOptions":{"paths":{"@b":["absent","module"]}}}"#,
    )
    .expect("fallback alias");
    let output = scan(
        temp.path(),
        &["--inner", "--json", "--tsconfig", "tsconfig.json"],
    );
    assert_eq!(
        output.status.code(),
        Some(1),
        "later path replacement and directory index create the real cycle"
    );
}

#[test]
fn malformed_control_bytes_are_rejected_without_panic() {
    assert!(
        jscycles::Config::parse("pa\0\0\0\0\0\0\0\u{15}\u{19} uns:T\n#]\n").is_err(),
        "sanitizer RSS artifact is malformed YAML, not a valid configuration"
    );
}

#[test]
fn exact_tsconfig_alias_precedes_a_wildcard_alias() {
    let temp = tempfile::tempdir().expect("directory");
    cycle(temp.path());
    fs::write(temp.path().join("a.ts"), "import '@b';").expect("alias");
    fs::write(
        temp.path().join("tsconfig.json"),
        r#"{"compilerOptions":{"paths":{"*":["absent/*"],"@b":["b.ts"]}}}"#,
    )
    .expect("overlapping aliases");
    let output = scan(
        temp.path(),
        &["--inner", "--json", "--tsconfig", "tsconfig.json"],
    );
    assert_eq!(
        output.status.code(),
        Some(1),
        "exact alias wins over wildcard"
    );
}

#[test]
fn verbose_summary_counts_the_displayed_package_cycles() {
    let temp = tempfile::tempdir().expect("directory");
    fs::write(
        temp.path().join("package.json"),
        r#"{"name":"root","workspaces":["packages/*"]}"#,
    )
    .expect("workspace");
    for (name, target) in [("a", "b"), ("b", "a")] {
        let dir = temp.path().join("packages").join(name);
        fs::create_dir_all(&dir).expect("package");
        fs::write(dir.join("package.json"), format!("{{\"name\":\"{name}\"}}")).expect("manifest");
        fs::write(dir.join("index.ts"), format!("import '{target}';")).expect("source");
    }
    let output = scan(temp.path(), &["--outer", "--verbose"]);
    assert_eq!(output.status.code(), Some(1), "cycle found");
    assert!(
        String::from_utf8_lossy(&output.stdout).contains("Summary: 1 inter-package cycles"),
        "summary agrees with detailed cycle output"
    );
}

#[test]
fn inherited_workspace_aliases_preserve_package_cycles() {
    let temp = tempfile::tempdir().expect("directory");
    fs::write(
        temp.path().join("package.json"),
        r#"{"name":"root","workspaces":["packages/*"]}"#,
    )
    .expect("workspace");
    for (name, target) in [("a", "alias-b"), ("b", "a")] {
        let dir = temp.path().join("packages").join(name);
        fs::create_dir_all(&dir).expect("package");
        fs::write(dir.join("package.json"), format!("{{\"name\":\"{name}\"}}")).expect("manifest");
        fs::write(dir.join("index.ts"), format!("import '{target}';")).expect("source");
    }
    fs::create_dir(temp.path().join("configs")).expect("config directory");
    fs::write(
        temp.path().join("configs/base.json"),
        r#"{"compilerOptions":{"baseUrl":"..","paths":{"alias-b":["packages/b/index.ts"]}}}"#,
    )
    .expect("base aliases");
    fs::write(
        temp.path().join("tsconfig.json"),
        r#"{"extends":"./configs/base.json"}"#,
    )
    .expect("root config");
    let output = scan(temp.path(), &["--outer", "--json"]);
    assert_eq!(
        output.status.code(),
        Some(1),
        "workspace aliases use the same origin and inheritance contract"
    );
    let without = scan(temp.path(), &["--outer", "--json", "--no-tsconfig"]);
    assert_eq!(
        without.status.code(),
        Some(0),
        "no-tsconfig disables automatic workspace aliases"
    );
    fs::rename(
        temp.path().join("tsconfig.json"),
        temp.path().join("chosen.json"),
    )
    .expect("explicit config only");
    let explicit = scan(
        temp.path(),
        &["--outer", "--json", "--tsconfig", "chosen.json"],
    );
    assert_eq!(
        explicit.status.code(),
        Some(1),
        "explicit tsconfig also supplies workspace-package aliases"
    );
    fs::create_dir_all(temp.path().join("packages/b-other")).expect("non-package sibling");
    fs::write(
        temp.path().join("packages/b-other/index.ts"),
        "export const value = 1;",
    )
    .expect("sibling target");
    fs::write(
        temp.path().join("configs/base.json"),
        r#"{"compilerOptions":{"baseUrl":"..","paths":{"alias-b":["packages/b-other/index.ts"]}}}"#,
    )
    .expect("sibling mapping");
    let sibling = scan(
        temp.path(),
        &["--outer", "--json", "--tsconfig", "chosen.json"],
    );
    assert_eq!(
        sibling.status.code(),
        Some(0),
        "component prefixes do not invent package membership"
    );
}

#[test]
fn infallible_legacy_workspace_selection_does_not_return_budget_truncation() {
    use jscycles::budget::{AnalysisBudget, Resource};
    let config = jscycles::Config::default();
    let budget = AnalysisBudget::default();
    let discovery = jscycles::PackageDiscovery::new_with_budget(
        &config,
        &["*".to_owned()],
        &[],
        budget.clone(),
    )
    .expect("discovery");
    let remaining = budget
        .limits()
        .max_work
        .checked_sub(budget.used(Resource::Work))
        .expect("admitted counters");
    budget
        .consume(Resource::Work, remaining)
        .expect("exact work quota");
    // Real canonical directories: a spelled `/project` is not absolute on Windows.
    let temp = tempfile::tempdir().expect("directory");
    let project = fs::canonicalize(temp.path()).expect("canonical root");
    fs::create_dir(project.join("a")).expect("member directory");
    let workspace = jscycles::Workspace {
        root: project.clone(),
        format: jscycles::WorkspaceFormat::Npm,
        packages: std::collections::HashMap::from([("a".to_owned(), project.join("a"))]),
        aliases: std::collections::HashMap::default(),
    };
    let selected = discovery.discover_workspace_packages(&workspace, &[project]);
    assert_eq!(
        selected.len(),
        1,
        "legacy selector is complete and explicitly unmetered"
    );
    assert!(
        budget.check().is_ok(),
        "infallible compatibility method does not hide a quota error"
    );
}

#[test]
fn invalid_cli_extension_spelling_is_not_a_clean_empty_scan() {
    let temp = tempfile::tempdir().expect("directory");
    cycle(temp.path());
    for extension in [".ts", "", "ts/path"] {
        let output = scan(
            temp.path(),
            &["--inner", "--json", "--extensions", extension],
        );
        assert_eq!(
            output.status.code(),
            Some(2),
            "invalid explicit suffix rejected"
        );
        assert!(output.stdout.is_empty(), "no empty successful report");
    }
}

#[cfg(unix)]
#[test]
fn dangling_default_config_and_nonregular_sources_are_errors() {
    use std::os::unix::fs::symlink;
    let temp = tempfile::tempdir().expect("directory");
    cycle(temp.path());
    symlink("missing.yaml", temp.path().join("jscycles.yaml")).expect("dangling config");
    let output = scan(temp.path(), &["--inner", "--json"]);
    assert_eq!(
        output.status.code(),
        Some(2),
        "an existing dangling config is not an absent default"
    );
    fs::remove_file(temp.path().join("jscycles.yaml")).expect("remove test link");
    fs::remove_file(temp.path().join("a.ts")).expect("replace source");
    symlink("/dev/zero", temp.path().join("a.ts")).expect("nonregular source");
    let device = scan(temp.path(), &["--inner", "--json"]);
    assert_eq!(
        device.status.code(),
        Some(2),
        "reject device input before reading"
    );
    assert!(device.stdout.is_empty(), "no partial graph");
    assert!(
        String::from_utf8_lossy(&device.stderr).contains("regular file"),
        "device error identifies the ingress contract"
    );
}

#[test]
fn duplicate_package_names_and_distinct_workspace_roots_are_rejected() {
    let temp = tempfile::tempdir().expect("directory");
    for (workspace, package_name) in [("one", "a"), ("two", "b")] {
        let root = temp.path().join(workspace);
        fs::create_dir(&root).expect("root");
        fs::write(
            root.join("package.json"),
            r#"{"name":"root","workspaces":["packages/*"]}"#,
        )
        .expect("workspace");
        let dir = root.join("packages/first");
        cycle(&dir);
        fs::write(
            dir.join("package.json"),
            format!("{{\"name\":\"{package_name}\"}}"),
        )
        .expect("manifest");
    }
    let output = scan(temp.path(), &["one", "two", "--outer", "--json"]);
    assert_eq!(
        output.status.code(),
        Some(2),
        "no silently omitted second workspace"
    );
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("one workspace"),
        "explicit scope diagnostic"
    );
    let duplicate = temp.path().join("one/packages/second");
    cycle(&duplicate);
    fs::write(duplicate.join("package.json"), r#"{"name":"a"}"#).expect("duplicate name");
    let duplicate_result = scan(&temp.path().join("one"), &["--outer", "--json"]);
    assert_eq!(
        duplicate_result.status.code(),
        Some(2),
        "distinct directories cannot share one package graph node"
    );
    assert!(
        String::from_utf8_lossy(&duplicate_result.stderr)
            .contains("duplicate workspace package name"),
        "domain diagnostic"
    );
}

#[test]
fn excluded_names_above_the_scan_root_do_not_hide_the_checkout() {
    // Exclusions apply beneath the walk root only. A checkout located under a
    // `build`/`dist`/`node_modules`/`.git`/`.next` ancestor (CI workspaces such
    // as `/builds/...` or `/build/...`) must not be excluded as a whole.
    for ancestor in ["build", "dist", "node_modules", ".git", ".next"] {
        let temp = tempfile::tempdir().expect("directory");
        let root = temp.path().join(ancestor).join("project");
        cycle(&root);
        let implicit = scan(&root, &["--inner", "--json"]);
        assert_eq!(
            implicit.status.code(),
            Some(1),
            "cwd scan under {ancestor}/ finds the cycle: {}",
            String::from_utf8_lossy(&implicit.stderr)
        );
        let explicit = scan(
            temp.path(),
            &["--inner", "--json", root.to_str().expect("UTF-8 path")],
        );
        assert_eq!(
            explicit.status.code(),
            Some(1),
            "explicit operand under {ancestor}/ finds the cycle"
        );
    }
}

#[test]
fn excluded_names_beneath_the_scan_root_remain_excluded() {
    let temp = tempfile::tempdir().expect("directory");
    fs::write(temp.path().join("package.json"), r#"{"name":"example"}"#).expect("manifest");
    fs::write(temp.path().join("a.ts"), "import './build/b';").expect("source a");
    fs::create_dir(temp.path().join("build")).expect("build directory");
    fs::write(temp.path().join("build/b.ts"), "import '../a';").expect("source b");
    let output = scan(temp.path(), &["--inner", "--json"]);
    assert_eq!(
        output.status.code(),
        Some(0),
        "build output inside the package is not a source vertex"
    );
}

#[test]
fn workspace_under_an_excluded_ancestor_still_reports_package_cycles() {
    let temp = tempfile::tempdir().expect("directory");
    let root = temp.path().join("build").join("mono");
    for (name, dependency) in [("a", "b"), ("b", "a")] {
        let package = root.join("packages").join(name);
        fs::create_dir_all(&package).expect("package directory");
        fs::write(
            package.join("package.json"),
            format!(r#"{{"name":"{name}","dependencies":{{"{dependency}":"*"}}}}"#),
        )
        .expect("package manifest");
        fs::write(package.join("index.ts"), format!("import '{dependency}';"))
            .expect("package source");
    }
    fs::write(
        root.join("package.json"),
        r#"{"name":"root","private":true,"workspaces":["packages/*"]}"#,
    )
    .expect("root manifest");
    let output = scan(&root, &["--outer", "--json"]);
    assert_eq!(
        output.status.code(),
        Some(1),
        "workspace members under a build/ ancestor are discovered: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn ignore_globs_match_package_relative_paths() {
    let temp = tempfile::tempdir().expect("directory");
    let root = temp.path().join("fixtures").join("project");
    cycle(&root);
    // An ancestor named like an ignored directory must not exclude the package.
    fs::write(
        root.join("jscycles.yaml"),
        "packages:\n  example:\n    ignore:\n      - \"**/fixtures/**\"\n",
    )
    .expect("configuration");
    assert_eq!(
        scan(&root, &["--inner", "--json"]).status.code(),
        Some(1),
        "ancestor directory names are outside the ignore scope"
    );
    // Package-relative patterns select the intended file.
    fs::write(
        root.join("jscycles.yaml"),
        "packages:\n  example:\n    ignore:\n      - \"b.ts\"\n",
    )
    .expect("configuration");
    assert_eq!(
        scan(&root, &["--inner", "--json"]).status.code(),
        Some(0),
        "a package-relative ignore removes the cycle member"
    );
}

#[test]
fn tsconfig_array_extends_applies_entries_in_order() {
    let temp = tempfile::tempdir().expect("directory");
    cycle(temp.path());
    fs::write(temp.path().join("c.ts"), "export {};").expect("non-cyclic target");
    fs::create_dir(temp.path().join("configs")).expect("configs");
    fs::write(
        temp.path().join("configs/first.json"),
        r#"{"compilerOptions":{"baseUrl":"..","paths":{"@b":["c.ts"]}}}"#,
    )
    .expect("first base");
    fs::write(
        temp.path().join("configs/second.json"),
        r#"{"compilerOptions":{"baseUrl":"..","paths":{"@b":["b.ts"]}}}"#,
    )
    .expect("second base");
    fs::write(
        temp.path().join("tsconfig.json"),
        r#"{"extends":["./configs/first.json","./configs/second.json"]}"#,
    )
    .expect("child config");
    fs::write(temp.path().join("a.ts"), "import '@b';").expect("alias source");
    let output = scan(
        temp.path(),
        &["--inner", "--json", "--tsconfig", "tsconfig.json"],
    );
    assert_eq!(
        output.status.code(),
        Some(1),
        "the later array entry wins: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    fs::write(
        temp.path().join("tsconfig.json"),
        r#"{"extends":["./configs/second.json","./configs/first.json"]}"#,
    )
    .expect("reordered child config");
    assert_eq!(
        scan(
            temp.path(),
            &["--inner", "--json", "--tsconfig", "tsconfig.json"]
        )
        .status
        .code(),
        Some(0),
        "reordering the array changes which alias applies"
    );

    fs::write(
        temp.path().join("tsconfig.json"),
        r#"{"extends":["./configs/second.json","./configs/missing.json"]}"#,
    )
    .expect("broken child config");
    assert_eq!(
        scan(
            temp.path(),
            &["--inner", "--json", "--tsconfig", "tsconfig.json"]
        )
        .status
        .code(),
        Some(2),
        "a missing array entry is a strict --tsconfig error"
    );
}

/// Report text with `/` separators: Windows prints native `\` paths, escaped
/// as `\\` in JSON.
fn slashes(text: &str) -> String {
    text.replace(r"\\", "/").replace('\\', "/")
}

fn status(root: &Path, args: &[&str]) -> Option<i32> {
    scan(root, args).status.code()
}

fn write(root: &Path, relative: &str, contents: &str) {
    let path = root.join(relative);
    fs::create_dir_all(path.parent().expect("parent")).expect("directories");
    fs::write(path, contents).expect("file");
}

#[test]
fn directory_specifiers_resolve_only_to_the_directory_index() {
    // Node/TypeScript resolve `./b/`, `.` and `..` as directories. A sibling
    // `b.ts` / `<dir>.ts` must not be chosen, or a nonexistent cycle appears.
    let cases: [&[(&str, &str)]; 3] = [
        &[
            ("src/a.ts", "import './b/';"),
            ("src/b.ts", "import './a';"),
            ("src/b/index.ts", "export {};"),
        ],
        &[
            ("pkg/a.ts", "import '.';"),
            ("pkg.ts", "import './pkg/a';"),
            ("pkg/index.ts", "export {};"),
        ],
        &[
            ("src/lib/a.ts", "import '..';"),
            ("src.ts", "import './src/lib/a';"),
            ("src/index.ts", "export {};"),
        ],
    ];
    for files in cases {
        let temp = tempfile::tempdir().expect("directory");
        write(temp.path(), "package.json", r#"{"name":"example"}"#);
        for (path, contents) in files {
            write(temp.path(), path, contents);
        }
        assert_eq!(
            status(temp.path(), &["--inner", "--json", "--no-tsconfig"]),
            Some(0),
            "no cycle through a sibling file: {files:?}"
        );
    }

    // The index itself is still resolved, so a real cycle through it is found.
    let temp = tempfile::tempdir().expect("directory");
    write(temp.path(), "package.json", r#"{"name":"example"}"#);
    write(temp.path(), "src/a.ts", "import './b/';");
    write(temp.path(), "src/b/index.ts", "import '../a';");
    assert_eq!(
        status(temp.path(), &["--inner", "--json", "--no-tsconfig"]),
        Some(1),
        "a trailing-slash import reaches the directory index"
    );
}

#[test]
fn alias_directory_specifiers_resolve_only_to_the_directory_index() {
    let temp = tempfile::tempdir().expect("directory");
    write(temp.path(), "package.json", r#"{"name":"example"}"#);
    write(
        temp.path(),
        "tsconfig.json",
        r#"{"compilerOptions":{"paths":{"@/*":["./src/*"]}}}"#,
    );
    write(temp.path(), "src/a.ts", "import '@/b/';");
    write(temp.path(), "src/b.ts", "import './a';");
    write(temp.path(), "src/b/index.ts", "export {};");
    assert_eq!(
        status(
            temp.path(),
            &["--inner", "--json", "--tsconfig", "tsconfig.json"]
        ),
        Some(0),
        "`@/b/` selects src/b/index.ts, not src/b.ts"
    );
}

fn two_package_cycle(root: &Path) {
    write(root, "packages/a/package.json", r#"{"name":"a"}"#);
    write(root, "packages/b/package.json", r#"{"name":"b"}"#);
    write(root, "packages/a/index.ts", "import 'b';");
    write(root, "packages/b/index.ts", "import 'a';");
}

#[test]
fn dot_slash_workspace_globs_select_members() {
    let temp = tempfile::tempdir().expect("directory");
    two_package_cycle(temp.path());
    write(
        temp.path(),
        "package.json",
        r#"{"name":"root","private":true,"workspaces":["./packages/*"]}"#,
    );
    assert_eq!(
        status(temp.path(), &["--outer", "--json"]),
        Some(1),
        "`./packages/*` is the same selection as `packages/*`"
    );

    write(
        temp.path(),
        "pnpm-workspace.yaml",
        "packages:\n  - \"./packages/*\"\n  - \"!./packages/b\"\n",
    );
    assert_eq!(
        status(temp.path(), &["--outer", "--json"]),
        Some(0),
        "a `./` negation removes its member"
    );
}

#[test]
fn member_tsconfig_references_do_not_replace_the_package_manager_root() {
    let temp = tempfile::tempdir().expect("directory");
    two_package_cycle(temp.path());
    write(
        temp.path(),
        "package.json",
        r#"{"name":"root","private":true,"workspaces":["packages/*"]}"#,
    );
    write(
        temp.path(),
        "packages/a/tsconfig.json",
        r#"{"references":[{"path":"../b"}]}"#,
    );
    assert_eq!(
        status(
            temp.path(),
            &["--outer", "--json", "packages/a", "packages/b"]
        ),
        Some(1),
        "both operands belong to the npm workspace"
    );
}

#[test]
fn many_operands_share_one_workspace_discovery() {
    let temp = tempfile::tempdir().expect("directory");
    two_package_cycle(temp.path());
    write(
        temp.path(),
        "package.json",
        r#"{"name":"root","private":true,"workspaces":["packages/*"]}"#,
    );
    for index in 0..30 {
        write(
            temp.path(),
            &format!("packages/n{index}/package.json"),
            &format!(r#"{{"name":"n{index}"}}"#),
        );
    }
    let mut child = Command::new(env!("CARGO_BIN_EXE_jscycles"))
        .current_dir(temp.path())
        .args(["--outer", "--json", "--stdin"])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("CLI starts");
    let input = "packages/a\npackages/b\n".repeat(4000);
    {
        use std::io::Write as _;
        let mut stdin = child.stdin.take().expect("stdin");
        stdin.write_all(input.as_bytes()).expect("operands");
    }
    let output = child.wait_with_output().expect("CLI finishes");
    assert_eq!(
        output.status.code(),
        Some(1),
        "8000 operands do not re-expand the workspace each time: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn bare_manifest_operands_select_the_current_package() {
    let temp = tempfile::tempdir().expect("directory");
    cycle(temp.path());
    assert_eq!(status(temp.path(), &["--inner", "package.json"]), Some(1));
    let mut child = Command::new(env!("CARGO_BIN_EXE_jscycles"))
        .current_dir(temp.path())
        .args(["--inner", "--stdin"])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::null())
        .spawn()
        .expect("CLI starts");
    {
        use std::io::Write as _;
        let mut stdin = child.stdin.take().expect("stdin");
        stdin.write_all(b"package.json\n").expect("operand");
    }
    assert_eq!(
        child.wait().expect("CLI finishes").code(),
        Some(1),
        "stdin operands share the manifest rule"
    );
}

#[test]
fn explicit_manifest_without_a_name_is_an_error() {
    let temp = tempfile::tempdir().expect("directory");
    cycle(temp.path());
    fs::write(temp.path().join("package.json"), r#"{"version":"1.0.0"}"#).expect("manifest");
    assert_eq!(
        status(temp.path(), &["--inner", "package.json"]),
        Some(2),
        "a selected manifest that defines no package is not a clean scan"
    );
}

#[test]
fn dot_slash_include_roots_select_their_directory() {
    let temp = tempfile::tempdir().expect("directory");
    cycle(&temp.path().join("libs/x"));
    fs::write(
        temp.path().join("jscycles.yaml"),
        "scan:\n  include: [\"./libs\"]\n",
    )
    .expect("configuration");
    assert_eq!(status(temp.path(), &["--inner", "--json"]), Some(1));
}

#[test]
fn explicit_null_package_overrides_are_errors() {
    let temp = tempfile::tempdir().expect("directory");
    cycle(temp.path());
    for yaml in [
        "packages: {example: {extensions: null}}",
        "packages: {example: {ignore: null}}",
    ] {
        fs::write(temp.path().join("jscycles.yaml"), yaml).expect("configuration");
        assert_eq!(status(temp.path(), &["--inner"]), Some(2), "{yaml}");
    }
    fs::write(temp.path().join("jscycles.yaml"), "packages: {example: {}}").expect("configuration");
    assert_eq!(
        status(temp.path(), &["--inner"]),
        Some(1),
        "omitted overrides"
    );
}

#[test]
fn references_project_inside_an_unrelated_npm_workspace_keeps_its_packages() {
    // An npm workspace higher up (for example in a home directory) that does
    // not contain the project must not capture a TypeScript references repo.
    let temp = tempfile::tempdir().expect("directory");
    write(
        temp.path(),
        "package.json",
        r#"{"name":"home","private":true,"workspaces":["projects/*"]}"#,
    );
    let repo = temp.path().join("repo");
    write(&repo, "packages/a/package.json", r#"{"name":"@x/a"}"#);
    write(&repo, "packages/b/package.json", r#"{"name":"@x/b"}"#);
    write(&repo, "packages/a/index.ts", "import '@x/b';");
    write(&repo, "packages/b/index.ts", "import '@x/a';");
    write(
        &repo,
        "tsconfig.json",
        r#"{"references":[{"path":"packages/a"},{"path":"packages/b"}]}"#,
    );
    assert_eq!(status(&repo, &["--outer", "--json"]), Some(1));
    assert_eq!(
        status(&repo, &["--outer", "--json", "packages/a", "packages/b"]),
        Some(1),
        "explicit operands select the same references workspace"
    );
}

#[test]
fn dot_slash_and_absolute_exclude_and_ignore_patterns_still_apply() {
    // Absolute patterns are compared lexically with the cwd, so run from a
    // canonical spelling: macOS temp is `/var` -> `/private/var`, and Windows
    // runners spell TEMP with 8.3 names (`RUNNER~1`).
    let canonical = fs::canonicalize(std::env::temp_dir()).expect("canonical temp");
    let canonical = canonical.to_str().expect("UTF-8 path");
    let base = canonical.strip_prefix(r"\\?\").unwrap_or(canonical);
    let temp = tempfile::tempdir_in(base).expect("directory");
    cycle(&temp.path().join("apps/legacy"));
    fs::write(
        temp.path().join("apps/legacy/package.json"),
        r#"{"name":"legacy"}"#,
    )
    .expect("manifest");
    let absolute = temp.path().join("apps/legacy");
    let absolute = absolute.to_str().expect("UTF-8 path").replace('\\', "/");
    for exclude in ["./apps/legacy", absolute.as_str()] {
        fs::write(
            temp.path().join("jscycles.yaml"),
            format!("scan:\n  exclude: [\"{exclude}\"]\n"),
        )
        .expect("configuration");
        assert_eq!(
            status(temp.path(), &["--inner", "--json"]),
            Some(0),
            "exclude {exclude:?} removes the package"
        );
    }

    // Relative patterns are relative to the walked root: the operand here.
    fs::write(
        temp.path().join("jscycles.yaml"),
        "scan:\n  exclude: [\"legacy\"]\n",
    )
    .expect("configuration");
    assert_eq!(status(temp.path(), &["--inner", "--json", "apps"]), Some(0));

    fs::write(
        temp.path().join("jscycles.yaml"),
        "packages:\n  legacy:\n    ignore: [\"./b.ts\"]\n",
    )
    .expect("configuration");
    assert_eq!(
        status(temp.path(), &["--inner", "--json"]),
        Some(0),
        "a `./` package ignore removes the cycle member"
    );
}

#[test]
fn wildcard_alias_targets_ending_in_a_separator_are_directories() {
    let temp = tempfile::tempdir().expect("directory");
    write(temp.path(), "package.json", r#"{"name":"example"}"#);
    write(
        temp.path(),
        "tsconfig.json",
        r#"{"compilerOptions":{"paths":{"@x/*":["src/*/"]}}}"#,
    );
    write(temp.path(), "src/e.ts", "import '@x/lib';");
    write(temp.path(), "src/lib.ts", "import './e';");
    write(temp.path(), "src/lib/index.ts", "export {};");
    assert_eq!(
        status(
            temp.path(),
            &["--inner", "--json", "--tsconfig", "tsconfig.json"]
        ),
        Some(0),
        "`src/*/` selects src/lib/index.ts, not src/lib.ts"
    );
}

#[test]
fn multi_segment_wildcard_alias_suffixes_resolve() {
    // Guards Windows verbatim bases, where a pasted `utils/math` component
    // would not be split on `/` and the cycle would vanish.
    let temp = tempfile::tempdir().expect("directory");
    write(temp.path(), "package.json", r#"{"name":"example"}"#);
    write(
        temp.path(),
        "tsconfig.json",
        r#"{"compilerOptions":{"paths":{"@/*":["src/*"]}}}"#,
    );
    write(temp.path(), "src/a.ts", "import '@/utils/deep/math';");
    write(temp.path(), "src/utils/deep/math.ts", "import '@/a';");
    assert_eq!(
        status(
            temp.path(),
            &["--inner", "--json", "--tsconfig", "tsconfig.json"]
        ),
        Some(1),
        "nested alias suffixes reach their file"
    );
}

#[test]
fn invalid_extends_and_null_overrides_explain_the_expected_shape() {
    let temp = tempfile::tempdir().expect("directory");
    cycle(temp.path());
    fs::write(temp.path().join("tsconfig.json"), r#"{"extends":5}"#).expect("config");
    let output = scan(temp.path(), &["--inner", "--tsconfig", "tsconfig.json"]);
    assert_eq!(output.status.code(), Some(2));
    assert!(
        String::from_utf8_lossy(&output.stderr)
            .contains("must be a path string or an array of path strings"),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    fs::write(
        temp.path().join("jscycles.yaml"),
        "packages: {example: {ignore: null}}",
    )
    .expect("configuration");
    let null_override = scan(temp.path(), &["--inner"]);
    assert_eq!(null_override.status.code(), Some(2));
    assert!(
        String::from_utf8_lossy(&null_override.stderr).contains("explicit null is not allowed")
    );
}

fn references_repo(repo: &Path) {
    write(repo, "packages/a/package.json", r#"{"name":"@x/a"}"#);
    write(repo, "packages/b/package.json", r#"{"name":"@x/b"}"#);
    write(repo, "packages/a/index.ts", "import '@x/b';");
    write(repo, "packages/b/index.ts", "import '@x/a';");
    write(
        repo,
        "tsconfig.json",
        r#"{"references":[{"path":"packages/a"},{"path":"packages/b"}]}"#,
    );
}

#[test]
fn broken_or_unrelated_workspaces_above_a_references_repo_are_not_read_as_its_root() {
    // A stray ~/package.json or ~/pnpm-workspace.yaml must not break or
    // capture every TypeScript references repository below it.
    let ancestors: [&[(&str, &str)]; 4] = [
        &[("pnpm-workspace.yaml", "packages:\n  - \"[bad\"\n")],
        &[("package.json", "{ not json")],
        &[
            (
                "package.json",
                r#"{"name":"home","private":true,"workspaces":["*"]}"#,
            ),
            ("other/package.json", "{ not json"),
        ],
        &[
            ("pnpm-workspace.yaml", "packages:\n  - \"dups/*\"\n"),
            ("dups/one/package.json", r#"{"name":"dup"}"#),
            ("dups/two/package.json", r#"{"name":"dup"}"#),
        ],
    ];
    for files in ancestors {
        let temp = tempfile::tempdir().expect("directory");
        for (path, contents) in files {
            write(temp.path(), path, contents);
        }
        let repo = temp.path().join("repo");
        references_repo(&repo);
        assert_eq!(
            status(&repo, &["--outer", "--json", "packages/a", "packages/b"]),
            Some(1),
            "references workspace still applies under {files:?}"
        );
    }
}

#[test]
fn unnamed_member_tsconfig_references_do_not_replace_the_package_manager_root() {
    let temp = tempfile::tempdir().expect("directory");
    two_package_cycle(temp.path());
    write(
        temp.path(),
        "package.json",
        r#"{"name":"root","private":true,"workspaces":["packages/*", "tools/*"]}"#,
    );
    // A member manifest without a name still makes `tools/scripts` a member.
    write(
        temp.path(),
        "tools/scripts/package.json",
        r#"{"private":true}"#,
    );
    write(
        temp.path(),
        "tools/scripts/tsconfig.json",
        r#"{"references":[{"path":"../../packages/a"}]}"#,
    );
    write(temp.path(), "tools/scripts/src/x.ts", "export {};");
    assert_eq!(
        status(
            temp.path(),
            &[
                "--outer",
                "--json",
                "tools/scripts",
                "packages/a",
                "packages/b"
            ]
        ),
        Some(1),
        "the npm workspace owns every operand"
    );
}

#[test]
fn empty_exclude_patterns_and_explicit_operands_are_never_pruned_as_a_whole() {
    let temp = tempfile::tempdir().expect("directory");
    cycle(&temp.path().join("apps/x"));
    for exclude in ["\"\"", "\"./\""] {
        fs::write(
            temp.path().join("jscycles.yaml"),
            format!("scan:\n  exclude: [{exclude}]\n"),
        )
        .expect("configuration");
        assert_eq!(
            status(temp.path(), &["--inner", "--json"]),
            Some(1),
            "exclude {exclude} does not remove the scan root"
        );
    }
    // An explicitly selected operand is scanned even when a pattern from the
    // automatic scan root names it; patterns prune beneath their walk root.
    fs::write(
        temp.path().join("jscycles.yaml"),
        "scan:\n  exclude: [\"apps/*\"]\n",
    )
    .expect("configuration");
    assert_eq!(
        status(temp.path(), &["--inner", "--json", "apps/x"]),
        Some(1)
    );
    assert_eq!(
        status(temp.path(), &["--inner", "--json"]),
        Some(0),
        "an automatic scan still excludes apps/x"
    );
}

#[test]
fn single_star_member_patterns_do_not_select_deeper_references_projects() {
    // `packages/*` selects one path component, as in expansion. A references
    // project two levels down is not a member, so it remains the workspace.
    let temp = tempfile::tempdir().expect("directory");
    write(
        temp.path(),
        "package.json",
        r#"{"name":"root","private":true,"workspaces":["packages/*"]}"#,
    );
    let project = temp.path().join("packages/group/inner");
    write(&project, "package.json", r#"{"private":true}"#);
    references_repo(&project);
    assert_eq!(
        status(&project, &["--outer", "--json", "packages/a", "packages/b"]),
        Some(1),
        "the nested references project keeps its packages"
    );
}

#[test]
fn valid_syntax_rejected_by_the_upstream_grammars_still_scans() {
    // Upstream tree-sitter 0.23 grammars report ERROR nodes for these valid
    // constructs, which the strict syntax policy would turn into exit 2.
    let cases = [
        (
            "b.ts",
            "const s = String.raw`\\underbrace{x}`;\nimport './a';",
        ),
        (
            "b.tsx",
            "const s = String.raw`\\underbrace{x}`;\nimport './a';",
        ),
        ("b.js", "const s = String.raw`\\unicode`;\nimport './a';"),
        (
            "b.ts",
            "vi.mock('./c', async (importOriginal) => ({\n  ...(await importOriginal<typeof import('./c')>()),\n}));\nimport './a';",
        ),
        (
            "b.tsx",
            "vi.mock('./c', async (importOriginal) => ({\n  ...(await importOriginal<typeof import('./c')>()),\n}));\nimport './a';",
        ),
    ];
    for (file, source) in cases {
        let temp = tempfile::tempdir().expect("directory");
        write(temp.path(), "package.json", r#"{"name":"example"}"#);
        write(
            temp.path(),
            "a.ts",
            &format!("import './{}';", file.split('.').next().expect("stem")),
        );
        write(temp.path(), "c.ts", "export const c = 1;");
        write(temp.path(), file, source);
        let output = scan(temp.path(), &["--inner", "--json"]);
        assert_eq!(
            output.status.code(),
            Some(1),
            "{file}: {source}\n{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

#[test]
fn genuinely_broken_syntax_is_rejected_with_its_location() {
    let temp = tempfile::tempdir().expect("directory");
    write(temp.path(), "package.json", r#"{"name":"example"}"#);
    write(
        temp.path(),
        "a.ts",
        "import './b';\nexport function broken(value: string {\n  return value;\n}\n",
    );
    let output = scan(temp.path(), &["--inner"]);
    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("a.ts"), "{stderr}");
    assert!(stderr.contains("at line 2,"), "{stderr}");
}

#[test]
fn each_file_belongs_to_its_nearest_package() {
    let temp = tempfile::tempdir().expect("directory");
    let root = temp.path();
    write(
        root,
        "package.json",
        r#"{"name":"root","workspaces":["packages/*"]}"#,
    );
    write(root, "packages/a/package.json", r#"{"name":"a"}"#);
    write(root, "packages/a/x.ts", "import './y';");
    write(root, "packages/a/y.ts", "import './x';");
    write(root, "src/p.ts", "import './q';");
    write(root, "src/q.ts", "import './p';");
    for args in [
        &["--inner", "--json"][..],
        &["--inner", "--json", ".", "packages/a"],
    ] {
        let output = scan(root, args);
        assert_eq!(output.status.code(), Some(1), "{args:?}");
        let json = slashes(&String::from_utf8(output.stdout).expect("utf-8"));
        assert_eq!(
            json.matches("x.ts").count(),
            2,
            "member cycle reported once: {json}"
        );
        assert!(
            !json.contains("packages/a/x.ts"),
            "root does not own member files: {json}"
        );
        assert_eq!(
            json.matches("src/p.ts").count(),
            2,
            "root keeps its own files: {json}"
        );
    }
    // A member removed by a name filter still owns its directory, so the root
    // does not re-report the member's cycles under its own name.
    for args in [
        &["--inner", "--json", "--only", "root"][..],
        &["--inner", "--json", "--exclude", "a"],
    ] {
        let output = scan(root, args);
        assert_eq!(output.status.code(), Some(1), "{args:?}");
        let json = String::from_utf8(output.stdout).expect("utf-8");
        assert!(!json.contains("x.ts"), "{args:?}: {json}");
        assert!(json.contains("src/p.ts"), "{args:?}: {json}");
    }
    let output = scan(root, &["--inner", "--json", "--only", "a"]);
    let json = String::from_utf8(output.stdout).expect("utf-8");
    assert!(
        json.contains("x.ts") && !json.contains("src/p.ts"),
        "{json}"
    );
}

#[test]
fn scan_exclude_directories_are_skipped_by_enclosing_packages() {
    let temp = tempfile::tempdir().expect("directory");
    let root = temp.path();
    write(root, "package.json", r#"{"name":"root"}"#);
    write(root, "wt/copy/package.json", r#"{"name":"root"}"#);
    write(root, "wt/copy/a.ts", "import './b';");
    write(root, "wt/copy/b.ts", "import './a';");
    write(root, "wt/copy/broken.ts", "broken(");
    write(root, "src/a.ts", "export const a = 1;");
    write(root, "jscycles.yaml", "scan:\n  exclude:\n    - \"wt\"\n");
    for args in [&["--inner"][..], &["--inner", "."]] {
        let output = scan(root, args);
        assert_eq!(
            output.status.code(),
            Some(0),
            "{args:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    // Only directories become boundaries: a file matched by scan.exclude is
    // still analyzed by its package.
    write(root, "src/b.ts", "import './c';");
    write(root, "src/c.ts", "import './b';");
    write(
        root,
        "jscycles.yaml",
        "scan:\n  exclude:\n    - \"wt\"\n    - \"src/b.ts\"\n",
    );
    assert_eq!(status(root, &["--inner"]), Some(1));
    // Without the exclusion the copy is a duplicate package and is broken.
    fs::remove_file(root.join("jscycles.yaml")).expect("remove config");
    assert_eq!(status(root, &["--inner"]), Some(2));
}

fn member_cycle(root: &Path) {
    write(
        root,
        "package.json",
        r#"{"name":"root","workspaces":["packages/*"]}"#,
    );
    write(root, "packages/a/package.json", r#"{"name":"a"}"#);
    write(root, "packages/a/src/x.ts", "import './y';");
    write(root, "packages/a/src/y.ts", "import './x';");
}

#[test]
fn include_and_exclude_globs_accept_trailing_slashes_and_absolute_spellings() {
    let temp = tempfile::tempdir().expect("directory");
    let root = fs::canonicalize(temp.path()).expect("canonical root");
    member_cycle(&root);
    let absolute = root.join("packages").display().to_string();
    for include in ["packages/", "packages/*/", "./packages/", absolute.as_str()] {
        write(
            &root,
            "jscycles.yaml",
            &format!("scan:\n  include:\n    - {include:?}\n"),
        );
        assert_eq!(status(&root, &["--inner"]), Some(1), "include {include:?}");
    }
    write(root.as_path(), "packages/a/legacy/p.ts", "import './q';");
    write(root.as_path(), "packages/a/legacy/q.ts", "import './p';");
    let legacy = root.join("packages/a/legacy").display().to_string();
    // Relative excludes are walk-root relative (R08); absolute ones must also
    // match operands spelled with `..`.
    let cases: [(&str, &[&str]); 3] = [
        ("packages/a/legacy/", &["--inner", "--json"]),
        (legacy.as_str(), &["--inner", "--json"]),
        (
            legacy.as_str(),
            &["--inner", "--json", "./packages/../packages"],
        ),
    ];
    for (exclude, args) in cases {
        write(
            &root,
            "jscycles.yaml",
            &format!("scan:\n  exclude:\n    - {exclude:?}\n"),
        );
        let output = scan(&root, args);
        let json = String::from_utf8_lossy(&output.stdout);
        assert!(
            !json.contains("p.ts"),
            "exclude {exclude:?} {args:?}: {json}"
        );
        assert!(
            json.contains("x.ts"),
            "exclude {exclude:?} {args:?}: {json}"
        );
    }
}

#[test]
fn filtered_manifest_operands_keep_their_directories() {
    let temp = tempfile::tempdir().expect("directory");
    member_cycle(temp.path());
    let output = scan(
        temp.path(),
        &[
            "--inner",
            "--json",
            "./package.json",
            "packages/a/package.json",
            "--exclude",
            "a",
        ],
    );
    assert_eq!(
        output.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&output.stdout)
    );
}

#[cfg(unix)]
#[test]
fn file_links_into_a_nested_package_belong_to_that_package() {
    let temp = tempfile::tempdir().expect("directory");
    let root = temp.path();
    member_cycle(root);
    std::os::unix::fs::symlink(root.join("packages/a/src/x.ts"), root.join("link.ts"))
        .expect("link");
    std::os::unix::fs::symlink(root.join("packages/a/src/y.ts"), root.join("link2.ts"))
        .expect("link");
    let output = scan(root, &["--inner", "--json"]);
    assert_eq!(output.status.code(), Some(1));
    let json = String::from_utf8(output.stdout).expect("utf-8");
    assert_eq!(json.matches("\"src/x.ts\"").count(), 2, "{json}");
    assert!(!json.contains("packages/a/src"), "{json}");
}

#[test]
fn ignored_automatic_tsconfigs_are_reported_on_stderr() {
    let temp = tempfile::tempdir().expect("directory");
    let root = temp.path();
    write(root, "package.json", r#"{"name":"example"}"#);
    write(root, "a.ts", "import '@/b';");
    write(root, "b.ts", "import '@/a';");
    write(
        root,
        "tsconfig.json",
        r#"{"extends":"./missing.json","compilerOptions":{"paths":{"@/*":["./*"]}}}"#,
    );
    let output = scan(root, &["--inner"]);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("warning: ignored tsconfig"), "{stderr}");
    assert!(stderr.contains("tsconfig.json"), "{stderr}");
    assert!(stderr.contains("--no-tsconfig"), "{stderr}");
    let quiet = scan(root, &["--inner", "--no-tsconfig"]);
    assert!(
        quiet.stderr.is_empty(),
        "{}",
        String::from_utf8_lossy(&quiet.stderr)
    );
}

#[test]
fn declaration_files_resolve_in_typescript_order() {
    // TypeScript probes `.ts`, `.tsx`, `.d.ts`, then JavaScript, and the same
    // for directory indexes; a cycle through declaration files is real.
    let temp = tempfile::tempdir().expect("directory");
    let root = temp.path();
    write(root, "package.json", r#"{"name":"example"}"#);
    write(root, "lib/a.d.ts", "export * from './b';");
    write(root, "lib/b/index.d.ts", "export * from '../a';");
    write(root, "lib/b/index.js", "export {};");
    let output = scan(root, &["--inner", "--json"]);
    assert_eq!(output.status.code(), Some(1));
    let json = slashes(&String::from_utf8(output.stdout).expect("utf-8"));
    assert!(json.contains("lib/b/index.d.ts"), "{json}");
    // A source file still wins over its declaration.
    write(root, "lib/a.ts", "export const a = 1;");
    write(root, "src/x.ts", "import '../lib/a';");
    write(root, "lib/b/index.d.ts", "export * from '../../src/x';");
    let preferred = scan(root, &["--inner", "--json"]);
    assert_eq!(
        preferred.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&preferred.stdout)
    );
}

#[test]
fn a_package_importing_its_own_name_keeps_the_file_edge() {
    let temp = tempfile::tempdir().expect("directory");
    let root = temp.path();
    write(
        root,
        "package.json",
        r#"{"name":"root","workspaces":["packages/*"]}"#,
    );
    write(root, "packages/a/package.json", r#"{"name":"@x/a"}"#);
    write(
        root,
        "packages/a/tsconfig.json",
        r#"{"compilerOptions":{"baseUrl":".","paths":{"@x/a/*":["src/*"]}}}"#,
    );
    write(root, "packages/a/src/index.ts", "import '@x/a/b';");
    write(root, "packages/a/src/b.ts", "import './index';");
    for args in [&[][..], &["packages/a"], &["--inner"]] {
        assert_eq!(status(root, args), Some(1), "{args:?}");
    }
}

#[test]
fn exact_workspace_aliases_may_contain_extra_segments() {
    let temp = tempfile::tempdir().expect("directory");
    let root = temp.path();
    two_package_cycle(root);
    write(
        root,
        "package.json",
        r#"{"name":"root","workspaces":["packages/*"]}"#,
    );
    write(
        root,
        "tsconfig.json",
        r#"{"compilerOptions":{"baseUrl":".","paths":{"lib/a":["packages/a/index.ts"],"@app/core/b":["packages/b/index.ts"]}}}"#,
    );
    write(root, "packages/a/index.ts", "import '@app/core/b';");
    write(root, "packages/b/index.ts", "import 'lib/a';");
    let output = scan(root, &["--outer", "--json", "-v"]);
    assert_eq!(
        output.status.code(),
        Some(1),
        "{}",
        String::from_utf8_lossy(&output.stdout)
    );
    let json = String::from_utf8(output.stdout).expect("utf-8");
    assert!(
        !json.contains("\"subpath\""),
        "exact aliases name no subpath: {json}"
    );
    write(root, "packages/b/index.ts", "import 'lib/a/util';");
    let with_subpath = scan(root, &["--outer", "--json", "-v"]);
    let report = String::from_utf8(with_subpath.stdout).expect("utf-8");
    assert!(report.contains("\"subpath\": \"util\""), "{report}");
}

#[test]
fn jsx_specifiers_prefer_tsx_like_typescript() {
    let temp = tempfile::tempdir().expect("directory");
    let root = temp.path();
    write(root, "package.json", r#"{"name":"example"}"#);
    write(root, "a.ts", "import './b.jsx';");
    write(root, "b.ts", "export {};");
    write(root, "b.tsx", "import './a';");
    assert_eq!(status(root, &["--inner"]), Some(1));
}

#[test]
fn package_evidence_lists_each_file_and_subpath_once() {
    let temp = tempfile::tempdir().expect("directory");
    let root = temp.path();
    two_package_cycle(root);
    write(
        root,
        "package.json",
        r#"{"name":"root","workspaces":["packages/*"]}"#,
    );
    write(
        root,
        "packages/a/index.ts",
        "import 'b';\nimport type { T } from 'b';\nimport 'b/';",
    );
    let output = scan(root, &["--outer", "--json", "-v"]);
    assert_eq!(output.status.code(), Some(1));
    let json = String::from_utf8(output.stdout).expect("utf-8");
    assert_eq!(json.matches("index.ts").count(), 2, "{json}");
    assert!(!json.contains("\"subpath\": \"\""), "{json}");
}

#[cfg(unix)]
#[test]
fn file_links_within_their_own_package_are_analyzed() {
    // Found by scripts/differential/difftest.py: the package's own root was in
    // its nested-package set, so every file link inside a package was skipped.
    let temp = tempfile::tempdir().expect("directory");
    let root = temp.path();
    member_cycle(root);
    write(root, "packages/a/src/real.ts", "import './y';");
    write(root, "packages/a/src/y.ts", "import './link';");
    std::os::unix::fs::symlink("real.ts", root.join("packages/a/src/link.ts")).expect("link");
    write(root, "packages/a/src/x.ts", "export {};");
    let output = scan(root, &["--inner", "--json"]);
    assert_eq!(
        output.status.code(),
        Some(1),
        "{}",
        String::from_utf8_lossy(&output.stdout)
    );
    let json = String::from_utf8(output.stdout).expect("utf-8");
    assert!(json.contains("src/real.ts"), "{json}");
}

#[cfg(unix)]
#[test]
fn file_links_to_a_sibling_package_belong_to_that_package() {
    // Found by scripts/differential/difftest.py: a link into a sibling package
    // was analyzed by both, adding a false package edge and a duplicate cycle.
    let temp = tempfile::tempdir().expect("directory");
    let root = temp.path();
    two_package_cycle(root);
    write(
        root,
        "package.json",
        r#"{"name":"root","workspaces":["packages/*"]}"#,
    );
    write(root, "packages/a/index.ts", "export {};");
    write(root, "packages/b/self.ts", "import './self';");
    std::os::unix::fs::symlink("../b/index.ts", root.join("packages/a/link.ts")).expect("link");
    let combined = scan(root, &["--json"]);
    let json = String::from_utf8(combined.stdout).expect("utf-8");
    assert!(
        json.contains("\"package_cycles\": []"),
        "no false a -> b edge: {json}"
    );
    std::os::unix::fs::symlink("../b/self.ts", root.join("packages/a/self-link.ts")).expect("link");
    let inner = scan(root, &["--inner", "--json"]);
    let report = String::from_utf8(inner.stdout).expect("utf-8");
    assert_eq!(
        report.matches("self.ts").count(),
        2,
        "cycle reported once, under b: {report}"
    );
}

#[test]
fn workspace_aliases_into_a_nested_non_member_stay_file_aliases() {
    // Found by scripts/differential/difftest.py: an alias into a nested package
    // that is not a workspace member was attributed to the enclosing member.
    let temp = tempfile::tempdir().expect("directory");
    let root = temp.path();
    write(
        root,
        "package.json",
        r#"{"name":"root","workspaces":["packages/a"]}"#,
    );
    write(root, "packages/a/package.json", r#"{"name":"a"}"#);
    write(root, "packages/a/inner/package.json", r#"{"name":"inner"}"#);
    write(root, "packages/a/inner/src/f.ts", "import '@alias/inner';");
    write(
        root,
        "tsconfig.json",
        r#"{"compilerOptions":{"baseUrl":".","paths":{"@alias/inner":["packages/a/inner/src/f"]}}}"#,
    );
    for args in [&[][..], &["--inner"]] {
        let output = scan(root, args);
        let stdout = slashes(&String::from_utf8_lossy(&output.stdout));
        assert_eq!(output.status.code(), Some(1), "{args:?}: {stdout}");
        assert!(
            stdout.contains("src/f.ts -> src/f.ts"),
            "{args:?}: {stdout}"
        );
    }
}

#[cfg(unix)]
#[test]
fn self_imports_are_judged_by_the_source_location() {
    // Found by scripts/differential/difftest.py: a link into an unscanned
    // member made that member's self-import look like a package edge in the
    // default mode but a self-cycle with --inner.
    let temp = tempfile::tempdir().expect("directory");
    let root = temp.path();
    write(
        root,
        "package.json",
        r#"{"name":"root","workspaces":["packages/*"]}"#,
    );
    write(root, "packages/a/package.json", r#"{"name":"a"}"#);
    write(root, "packages/b/package.json", r#"{"name":"b"}"#);
    write(root, "packages/a/src/f.ts", "import '@alias/a';");
    write(
        root,
        "tsconfig.json",
        r#"{"compilerOptions":{"baseUrl":".","paths":{"@alias/a":["packages/a/src/f"]}}}"#,
    );
    std::os::unix::fs::symlink("../a/src/f.ts", root.join("packages/b/link.ts")).expect("link");
    let file_cycles = |args: &[&str]| {
        let output = scan(root, args);
        let report: serde_json::Value = serde_json::from_slice(&output.stdout).expect("JSON");
        report["file_cycles"].clone()
    };
    let inner = file_cycles(&["--inner", "--json", "packages/b"]);
    assert_eq!(file_cycles(&["--json", "packages/b"]), inner);
    assert!(inner.to_string().contains("f.ts"), "{inner}");
}

#[test]
fn aliases_into_a_nested_non_member_add_no_package_edge() {
    // `@alias/inner` names a file of the non-member package nested in `a`, so
    // `b` importing it is not an edge to `a` (and no false a <-> b cycle).
    let temp = tempfile::tempdir().expect("directory");
    let root = temp.path();
    write(
        root,
        "package.json",
        r#"{"name":"root","workspaces":["packages/a","packages/b"]}"#,
    );
    write(root, "packages/a/package.json", r#"{"name":"a"}"#);
    write(root, "packages/b/package.json", r#"{"name":"b"}"#);
    write(root, "packages/a/inner/package.json", r#"{"name":"inner"}"#);
    write(root, "packages/a/inner/f.ts", "export {};");
    write(root, "packages/a/index.ts", "import 'b';");
    write(root, "packages/b/index.ts", "import '@alias/inner';");
    write(
        root,
        "tsconfig.json",
        r#"{"compilerOptions":{"baseUrl":".","paths":{"@alias/inner":["packages/a/inner/f"]}}}"#,
    );
    let output = scan(root, &["--outer"]);
    assert_eq!(
        output.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&output.stdout)
    );
}

#[test]
fn package_extends_follow_typescript_config_lookup() {
    // Each (manifest, files, extends) installs a base under node_modules. When
    // TypeScript finds the base, the child's alias cycle is reported with no
    // warning; when it does not, the tsconfig is ignored with a warning.
    let cases: [(&str, &[&str], &str, bool); 12] = [
        (
            r#"{"name":"@cfg/base"}"#,
            &["tsconfig.json"],
            "@cfg/base",
            true,
        ),
        (
            r#"{"name":"@cfg/base"}"#,
            &["base.json"],
            "@cfg/base/base",
            true,
        ),
        (
            r#"{"name":"@cfg/base","tsconfig":"./c/main.json"}"#,
            &["c/main.json"],
            "@cfg/base",
            true,
        ),
        (
            r#"{"name":"@cfg/base","exports":{".":"./c/main.json"}}"#,
            &["c/main.json"],
            "@cfg/base",
            true,
        ),
        (
            r#"{"name":"@cfg/base","exports":{".":{"types":"./index.d.ts","default":"./c/main.json"}}}"#,
            &["c/main.json"],
            "@cfg/base",
            true,
        ),
        (
            r#"{"name":"@cfg/base","exports":{"./*":"./c/*.json"}}"#,
            &["c/strict.json"],
            "@cfg/base/strict",
            true,
        ),
        (
            r#"{"name":"@cfg/base"}"#,
            &["sub/tsconfig.json"],
            "@cfg/base/sub",
            true,
        ),
        // A bare condition map is the `.` entry.
        (
            r#"{"name":"@cfg/base","exports":{"require":"./c/main.json"}}"#,
            &["c/main.json"],
            "@cfg/base",
            true,
        ),
        // Conditions are tried in written order: `require` before `node`.
        (
            r#"{"name":"@cfg/base","exports":{".":{"require":"./c/main.json","node":"./c/broken.json"}}}"#,
            &["c/main.json"],
            "@cfg/base",
            true,
        ),
        // `exports` decide alone: an unexported file is not found.
        (
            r#"{"name":"@cfg/base","exports":"./c/main.json"}"#,
            &["c/main.json"],
            "@cfg/base/x",
            false,
        ),
        (
            r#"{"name":"@cfg/base","exports":{"./x":"./c/x.json"}}"#,
            &["c/main.json"],
            "@cfg/base/c/main.json",
            false,
        ),
        (
            r#"{"name":"@cfg/base","exports":{".":{"import":"./c/main.json"}}}"#,
            &["c/main.json"],
            "@cfg/base",
            false,
        ),
    ];
    for (manifest, files, extends, found) in cases {
        let temp = tempfile::tempdir().expect("directory");
        let root = temp.path();
        write(root, "package.json", r#"{"name":"example"}"#);
        write(root, "src/a.ts", "import '@app/b';");
        write(root, "src/b.ts", "import '@app/a';");
        write(root, "node_modules/@cfg/base/package.json", manifest);
        for file in files {
            write(
                root,
                &format!("node_modules/@cfg/base/{file}"),
                "{\n  // base\n  \"compilerOptions\": {\"strict\": true,},\n}\n",
            );
        }
        // A `types` target that exists is still skipped: config lookup takes
        // only `.json` files, so resolution falls through to `default`.
        if manifest.contains("index.d.ts") {
            write(root, "node_modules/@cfg/base/index.d.ts", "export {};");
        }
        if manifest.contains("broken.json") {
            write(root, "node_modules/@cfg/base/c/broken.json", "{");
        }
        write(
            root,
            "tsconfig.json",
            &format!(
                r#"{{"extends":{extends:?},"compilerOptions":{{"paths":{{"@app/*":["./src/*"]}}}}}}"#
            ),
        );
        let output = scan(root, &["--inner"]);
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_eq!(
            output.status.code(),
            Some(i32::from(found)),
            "{extends} {manifest}: {stderr}"
        );
        assert_eq!(
            stderr.contains("warning: ignored tsconfig"),
            !found,
            "{extends} {manifest}: {stderr}"
        );
    }
}

#[test]
fn byte_order_marks_and_crlf_are_accepted_everywhere() {
    // Windows editors write a UTF-8 BOM and CRLF; npm, TypeScript and parsers
    // ignore both, so a BOM must not turn a manifest into a parse error.
    let temp = tempfile::tempdir().expect("directory");
    let root = temp.path();
    write(root, "package.json", "\u{feff}{\"name\":\"example\"}\r\n");
    write(
        root,
        "tsconfig.json",
        "\u{feff}{\r\n  // alias\r\n  \"compilerOptions\": {\"paths\": {\"@/*\": [\"./src/*\"]}}\r\n}\r\n",
    );
    write(root, "src/a.ts", "\u{feff}import '@/b';\r\n");
    write(root, "src/b.ts", "import './a';\r\n");
    write(
        root,
        "jscycles.yaml",
        "\u{feff}defaults:\r\n  extensions: [ts]\r\n",
    );
    let output = scan(root, &["--inner", "--json"]);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(output.status.code(), Some(1), "{stderr}");
    assert!(stderr.is_empty(), "{stderr}");
    let mut child = Command::new(env!("CARGO_BIN_EXE_jscycles"))
        .current_dir(root)
        .args(["--stdin", "--inner"])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::null())
        .spawn()
        .expect("CLI starts");
    std::io::Write::write_all(
        child.stdin.as_mut().expect("stdin"),
        "\u{feff}.\r\n".as_bytes(),
    )
    .expect("stdin written");
    assert_eq!(child.wait().expect("exit").code(), Some(1));
}

#[test]
fn type_only_imports_can_be_left_out() {
    // A cycle closed only by `import type` (or by a declaration file) is real
    // for type checking but not at runtime; `--skip-type-imports` or
    // `skip_type_imports` leaves such edges out.
    let temp = tempfile::tempdir().expect("directory");
    let root = temp.path();
    write(
        root,
        "package.json",
        r#"{"name":"root","workspaces":["packages/*"]}"#,
    );
    write(root, "packages/a/package.json", r#"{"name":"a"}"#);
    write(root, "packages/b/package.json", r#"{"name":"b"}"#);
    write(root, "packages/a/x.ts", "import { y } from './y';");
    write(
        root,
        "packages/a/y.ts",
        "import type { X } from './x';\nexport const y = 1;",
    );
    write(root, "packages/a/p.ts", "export * from './q';");
    write(root, "packages/a/q.d.ts", "export * from './p';");
    write(root, "packages/b/m.ts", "import { y } from 'a';");
    write(root, "packages/a/r.ts", "export type { M } from 'b';");
    let cycles = |args: &[&str]| {
        let output = scan(root, &[&["--json"][..], args].concat());
        let json: serde_json::Value = serde_json::from_slice(&output.stdout).expect("JSON");
        let files = json["file_cycles"]
            .as_array()
            .expect("file cycles")
            .iter()
            .map(|entry| entry["cycles"].as_array().map_or(0, Vec::len))
            .sum::<usize>();
        let packages = json["package_cycles"].as_array().map_or(0, Vec::len);
        (output.status.code(), files, packages)
    };
    assert_eq!(
        cycles(&[]),
        (Some(1), 2, 1),
        "type imports count by default"
    );
    assert_eq!(cycles(&["--skip-type-imports"]), (Some(0), 0, 0));
    write(root, "skip.yaml", "defaults:\n  skip_type_imports: true\n");
    assert_eq!(cycles(&["--config", "skip.yaml"]), (Some(0), 0, 0));
    write(
        root,
        "override.yaml",
        "defaults:\n  skip_type_imports: true\npackages:\n  a:\n    skip_type_imports: false\n",
    );
    assert_eq!(cycles(&["--config", "override.yaml"]), (Some(1), 2, 1));
    // Declaration files are still parsed.
    write(root, "packages/a/q.d.ts", "export * from (");
    assert_eq!(status(root, &["--skip-type-imports"]), Some(2));
}
