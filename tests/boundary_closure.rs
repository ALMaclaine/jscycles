//! Counterexamples from the rule-by-rule INPUT/CLI configuration review.
#![allow(
    clippy::expect_used,
    reason = "Setup failures must fail consumer tests"
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

fn package(root: &Path, name: &str, import: &str) {
    fs::create_dir_all(root).expect("package directory");
    fs::write(root.join("package.json"), format!(r#"{{"name":"{name}"}}"#)).expect("manifest");
    fs::write(root.join("index.ts"), format!("import '{import}';")).expect("source");
}

#[test]
fn duplicate_names_fail_without_workspace_metadata_in_every_output_mode() {
    let temp = tempfile::tempdir().expect("directory");
    package(&temp.path().join("a"), "duplicate", "./index");
    package(&temp.path().join("b"), "duplicate", "./index");
    for args in [vec!["--inner", "--json"], vec!["--inner", "--quiet"]] {
        let output = scan(temp.path(), &args);
        assert_eq!(output.status.code(), Some(2), "ambiguous identity rejected");
        assert!(output.stdout.is_empty(), "no incomplete report");
        assert!(
            String::from_utf8_lossy(&output.stderr).contains("duplicate"),
            "context"
        );
    }
}

#[test]
fn overlapping_operands_select_a_package_once() {
    let temp = tempfile::tempdir().expect("directory");
    package(&temp.path().join("a"), "a", "./index");
    let output = scan(
        temp.path(),
        &["--inner", "--json", "a", "a/package.json", "."],
    );
    assert_eq!(output.status.code(), Some(1), "self-cycle found");
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).expect("report");
    assert_eq!(
        report["file_cycles"].as_array().expect("packages").len(),
        1,
        "identity, not operand count"
    );
}

#[cfg(unix)]
#[test]
fn dangling_workspace_metadata_is_an_error_instead_of_absence() {
    for name in ["pnpm-workspace.yaml", "package.json", "tsconfig.json"] {
        let temp = tempfile::tempdir().expect("directory");
        package(&temp.path().join("a"), "a", "b");
        package(&temp.path().join("b"), "b", "a");
        std::os::unix::fs::symlink("absent", temp.path().join(name)).expect("dangling metadata");
        let error = jscycles::Workspace::discover_without_aliases(
            temp.path(),
            &jscycles::budget::AnalysisBudget::default(),
        )
        .expect_err("an existing broken input is not absent");
        assert!(
            error.to_string().contains(name),
            "failing metadata path retained"
        );
        let output = scan(temp.path(), &["--outer", "--json"]);
        assert_eq!(
            output.status.code(),
            Some(2),
            "broken membership cannot establish a clean graph"
        );
        assert!(output.stdout.is_empty(), "no partial report");
    }
}

#[test]
fn workspace_pattern_prefix_must_be_a_directory() {
    let temp = tempfile::tempdir().expect("directory");
    fs::write(
        temp.path().join("package.json"),
        r#"{"workspaces":["packages/*"]}"#,
    )
    .expect("workspace");
    fs::write(temp.path().join("packages"), "not a directory").expect("invalid prefix");
    let output = scan(temp.path(), &["--outer", "--json"]);
    assert_eq!(
        output.status.code(),
        Some(2),
        "invalid traversal input cannot become zero packages"
    );
    assert!(output.stdout.is_empty(), "no report");
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("packages"),
        "prefix context"
    );
}

#[test]
fn missing_typescript_reference_is_an_error() {
    let temp = tempfile::tempdir().expect("directory");
    fs::write(
        temp.path().join("tsconfig.json"),
        r#"{"references":[{"path":"./missing"}]}"#,
    )
    .expect("references");
    let output = scan(temp.path(), &["--outer", "--json"]);
    assert_eq!(
        output.status.code(),
        Some(2),
        "explicit missing membership cannot be discarded"
    );
    assert!(output.stdout.is_empty(), "no report");
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("missing"),
        "reference path context"
    );
}

#[test]
fn typescript_references_accept_named_config_files() {
    let temp = tempfile::tempdir().expect("directory");
    for (name, dependency) in [("a", "b"), ("b", "a")] {
        let directory = temp.path().join(name);
        package(&directory, name, dependency);
        fs::write(directory.join("build.config.json"), "{}").expect("referenced config");
    }
    fs::write(
        temp.path().join("tsconfig.json"),
        r#"{"references":[{"path":"./a/build.config.json"},{"path":"./b/build.config.json"}]}"#,
    )
    .expect("references");
    let output = scan(temp.path(), &["--outer", "--json"]);
    assert_eq!(
        output.status.code(),
        Some(1),
        "file references retain the a/b package cycle: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn extends_appends_json_without_replacing_a_dotted_stem() {
    let temp = tempfile::tempdir().expect("directory");
    package(temp.path(), "a", "@self");
    fs::write(temp.path().join("base.json"), "{}").expect("different config");
    fs::write(
        temp.path().join("base.profile.json"),
        r#"{"compilerOptions":{"paths":{"@self":["index.ts"]}}}"#,
    )
    .expect("selected base");
    fs::write(
        temp.path().join("tsconfig.json"),
        r#"{"extends":"./base.profile"}"#,
    )
    .expect("child");
    let output = scan(
        temp.path(),
        &["--inner", "--json", "--tsconfig", "tsconfig.json"],
    );
    assert_eq!(
        output.status.code(),
        Some(1),
        "do not load base.json in place of base.profile.json"
    );
}

#[test]
fn inherited_config_failures_identify_the_actual_failing_file() {
    let temp = tempfile::tempdir().expect("directory");
    package(temp.path(), "a", "./index");
    fs::write(
        temp.path().join("tsconfig.json"),
        r#"{"extends":"./broken-parent.json"}"#,
    )
    .expect("child");
    fs::write(temp.path().join("broken-parent.json"), "{invalid").expect("malformed parent");
    let error = jscycles::TsConfig::load(&temp.path().join("tsconfig.json"))
        .expect_err("inherited parse error");
    let mut cause: Option<&(dyn std::error::Error + 'static)> = Some(&error);
    let mut retained = false;
    while let Some(current) = cause {
        retained |= current.downcast_ref::<serde_json::Error>().is_some();
        cause = current.source();
    }
    assert!(retained, "context preserves the typed parser cause");
    let output = scan(
        temp.path(),
        &["--inner", "--json", "--tsconfig", "tsconfig.json"],
    );
    assert_eq!(output.status.code(), Some(2), "parse failure");
    assert!(output.stdout.is_empty(), "no report");
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("broken-parent.json"),
        "error names parent, not just requested child"
    );
}

#[test]
fn extends_depth_accepts_ten_files_and_rejects_the_eleventh() {
    let temp = tempfile::tempdir().expect("directory");
    for index in 0..10 {
        let contents = if index == 9 {
            "{}".to_owned()
        } else {
            format!(r#"{{"extends":"./{}.json"}}"#, index + 1)
        };
        fs::write(temp.path().join(format!("{index}.json")), contents).expect("chain");
    }
    let path = temp.path().join("0.json");
    assert!(
        jscycles::TsConfig::load(&path).is_ok(),
        "largest selected chain"
    );
    fs::write(temp.path().join("9.json"), r#"{"extends":"./10.json"}"#).expect("extend chain");
    fs::write(temp.path().join("10.json"), "{}").expect("extra file");
    assert!(
        jscycles::TsConfig::load(&path).is_err(),
        "first excessive depth"
    );
}

#[test]
fn duplicate_yaml_map_keys_are_rejected_before_selecting_overrides() {
    for yaml in [
        "packages:\n  a: {extensions: [ts]}\n  a: {extensions: [js]}\n",
        "packages:\n  a: {ignore: [], ignore: ['*.ts']}\n",
    ] {
        assert!(
            jscycles::Config::parse(yaml).is_err(),
            "duplicate settings have no implicit last-writer policy"
        );
    }
}

#[test]
fn workspace_parser_errors_retain_their_typed_causes() {
    for (name, text) in [
        ("package.json", "{\"workspaces\":"),
        ("pnpm-workspace.yaml", "packages: ["),
    ] {
        let temp = tempfile::tempdir().expect("directory");
        fs::write(temp.path().join(name), text).expect("malformed metadata");
        let error = jscycles::Workspace::discover_without_aliases(
            temp.path(),
            &jscycles::budget::AnalysisBudget::default(),
        )
        .expect_err("parse error");
        let cause = std::error::Error::source(&error).expect("typed parser cause retained");
        assert!(
            cause.downcast_ref::<serde_json::Error>().is_some()
                || cause.downcast_ref::<serde_saphyr::Error>().is_some(),
            "original JSON/YAML parser error retained"
        );
        assert!(
            error.to_string().contains(name),
            "failed input context retained"
        );
        let output = scan(temp.path(), &["--outer", "--json"]);
        assert_eq!(output.status.code(), Some(2), "parse failure status");
        assert!(output.stdout.is_empty(), "no report on parse failure");
    }
}

#[test]
fn missing_alias_and_relative_targets_do_not_invent_resolved_graph_nodes() {
    let temp = tempfile::Builder::new()
        .prefix("jscycles-missing-")
        .tempdir()
        .expect("directory");
    fs::write(
        temp.path().join("index.ts"),
        "import './missing'; import '@missing';",
    )
    .expect("source");
    let config_path = temp.path().join("tsconfig.json");
    fs::write(
        &config_path,
        r#"{"compilerOptions":{"paths":{"@missing":["absent.ts"]}}}"#,
    )
    .expect("config");
    let config = jscycles::TsConfig::load(&config_path).expect("valid alias declaration");
    let imports = jscycles::ImportExtractor::new(vec!["ts".to_owned()], Some(config))
        .extract(temp.path())
        .expect("static scan");
    assert_eq!(imports.len(), 2, "retain both import values");
    assert!(
        imports
            .iter()
            .all(|import| matches!(import.target, jscycles::ImportTarget::Unresolved(_))),
        "a missing file is not a successfully resolved target"
    );
    assert_eq!(
        jscycles::DependencyGraph::from_imports(&imports).node_count(),
        1,
        "only the actual source node is present"
    );
}

#[test]
fn owned_cached_metadata_copies_observe_the_shared_payload_budget() {
    use jscycles::budget::{AnalysisBudget, AnalysisLimits, Resource};
    let temp = tempfile::tempdir().expect("directory");
    let config_path = temp.path().join("tsconfig.json");
    let paths: serde_json::Map<String, serde_json::Value> = (0..32)
        .map(|number| {
            (
                format!("@alias{number}"),
                serde_json::json!([format!("src/file{number}.ts")]),
            )
        })
        .collect();
    fs::write(
        &config_path,
        serde_json::json!({"compilerOptions":{"paths": paths}}).to_string(),
    )
    .expect("config");
    for explicit_load in [false, true] {
        let budget = AnalysisBudget::new(AnalysisLimits {
            max_graph_bytes: 65_536,
            ..AnalysisLimits::default()
        })
        .expect("limits");
        let cache = jscycles::TsConfigCache::new().with_budget(budget.clone());
        let mut retained = Vec::new();
        let mut exhausted = false;
        for _ in 0..64 {
            let next = if explicit_load {
                cache.load(&config_path)
            } else {
                cache.discover(temp.path())
            };
            if let Some(config) = next {
                retained.push(config);
            } else {
                exhausted = true;
                break;
            }
        }
        assert!(!retained.is_empty(), "a valid initial copy fits");
        assert!(
            exhausted,
            "repeated owned copies cannot bypass payload admission"
        );
        assert_eq!(
            budget.check().expect_err("sticky exhaustion").resource(),
            Resource::GraphBytes,
            "copy retention is charged"
        );
        assert!(
            retained.iter().all(|config| config.paths.len() == 32),
            "published copies remain complete on a later failure"
        );
    }
}

#[test]
fn tsconfig_alias_sorting_requires_work_admission() {
    use jscycles::budget::{AnalysisBudget, AnalysisLimits, Resource};
    let temp = tempfile::tempdir().expect("directory");
    let path = temp.path().join("tsconfig.json");
    for count in [4, 256] {
        let paths: serde_json::Map<String, serde_json::Value> = (0..count)
            .map(|number| {
                (
                    format!("@alias{number}"),
                    serde_json::json!([format!("file{number}.ts")]),
                )
            })
            .collect();
        fs::write(
            &path,
            serde_json::json!({"compilerOptions":{"paths": paths}}).to_string(),
        )
        .expect("config");
        let budget = AnalysisBudget::new(AnalysisLimits {
            max_work: 1024,
            ..AnalysisLimits::default()
        })
        .expect("limits");
        let result = jscycles::TsConfig::load_with_budget(&path, &budget);
        if count == 4 {
            assert!(result.is_ok(), "small sorted metadata fits");
        } else {
            assert!(
                result.is_err(),
                "sorting large alias metadata must be admitted"
            );
            assert_eq!(
                budget.check().expect_err("exhaustion").resource(),
                Resource::Work,
                "sorting is logical work"
            );
        }
    }
}

#[test]
fn package_override_sorting_requires_work_admission() {
    use jscycles::budget::{AnalysisBudget, AnalysisLimits, Resource};
    let temp = tempfile::tempdir().expect("directory");
    package(temp.path(), "example", "./index");
    for count in [4, 256] {
        let mut config = jscycles::Config::default();
        config.packages = (0..count)
            .map(|number| {
                (
                    format!("other-{number}"),
                    jscycles::config::PackageConfig::default(),
                )
            })
            .collect();
        let budget = AnalysisBudget::new(AnalysisLimits {
            max_work: 1024,
            ..AnalysisLimits::default()
        })
        .expect("limits");
        let discovery =
            jscycles::PackageDiscovery::new_with_budget(&config, &[], &[], budget.clone())
                .expect("selectors fit");
        let result = discovery.discover_explicit(&[temp.path().join("package.json")]);
        if count == 4 {
            assert!(result.is_ok(), "small override selection fits");
        } else {
            assert!(result.is_err(), "sorting override keys must be admitted");
            assert_eq!(
                budget.check().expect_err("exhaustion").resource(),
                Resource::Work,
                "override sorting is logical work"
            );
        }
    }
}

#[test]
fn workspace_star_matches_one_directory_component() {
    let temp = tempfile::tempdir().expect("directory");
    fs::write(
        temp.path().join("package.json"),
        r#"{"workspaces":["packages/*"]}"#,
    )
    .expect("workspace");
    package(&temp.path().join("packages/a"), "a", "nested");
    package(&temp.path().join("packages/a/nested"), "nested", "a");
    let workspace = jscycles::Workspace::discover(temp.path())
        .expect("workspace lookup")
        .expect("workspace");
    assert!(
        !workspace.packages.contains_key("nested"),
        "single star does not admit nested package names"
    );
    let output = scan(temp.path(), &["--outer", "--json"]);
    assert_eq!(
        output.status.code(),
        Some(0),
        "unlisted nested package cannot invent a package cycle"
    );
}

#[test]
fn checkout_asterisks_are_literal_in_exact_and_wildcard_alias_targets() {
    let temp = tempfile::tempdir().expect("directory");
    let root = temp.path().join("checkout[brackets]");
    // '*' cannot be a Windows directory component. Brackets are valid there;
    // wildcard substitution only assigns meaning to the configured '*' marker.
    #[cfg(unix)]
    let root = root.join("literal*star");
    package(&root, "a", "@self");
    for (specifier, mapping) in [
        ("@self", r#"{"@self":["index.ts"]}"#),
        ("@/index", r#"{"@/*":["*.ts"]}"#),
    ] {
        fs::write(root.join("index.ts"), format!("import '{specifier}';")).expect("alias source");
        fs::write(
            root.join("tsconfig.json"),
            format!(r#"{{"compilerOptions":{{"paths":{mapping}}}}}"#),
        )
        .expect("aliases");
        let output = scan(&root, &["--inner", "--json", "--tsconfig", "tsconfig.json"]);
        assert_eq!(
            output.status.code(),
            Some(1),
            "checkout spelling cannot become alias syntax"
        );
    }
}

#[cfg(target_os = "linux")]
#[test]
fn config_origin_paths_do_not_undergo_lossy_unicode_conversion() {
    use std::os::unix::ffi::OsStringExt as _;
    let temp = tempfile::tempdir().expect("directory");
    let root = temp.path().join("source");
    package(&root, "a", "@self");
    let config_dir = temp
        .path()
        .join(std::ffi::OsString::from_vec(b"config-\xff".to_vec()));
    fs::create_dir(&config_dir).expect("non-UTF-8 config directory");
    let config_path = config_dir.join("tsconfig.json");
    for (specifier, mapping) in [
        ("@self", r#"{"@self":["index.ts"]}"#),
        ("@/index", r#"{"@/*":["*.ts"]}"#),
    ] {
        fs::write(root.join("index.ts"), format!("import '{specifier}';")).expect("alias source");
        fs::write(
            &config_path,
            format!(r#"{{"compilerOptions":{{"baseUrl":"../source","paths":{mapping}}}}}"#),
        )
        .expect("config");
        let output = Command::new(env!("CARGO_BIN_EXE_jscycles"))
            .current_dir(&root)
            .args(["--inner", "--json", "--tsconfig"])
            .arg(&config_path)
            .output()
            .expect("CLI starts");
        assert_eq!(
            output.status.code(),
            Some(1),
            "native config path identity retained; reported source path is UTF-8"
        );
    }
}

#[cfg(unix)]
#[test]
fn alias_results_preserve_native_origin_bytes() {
    use std::os::unix::ffi::OsStringExt as _;
    let origin = std::path::PathBuf::from(std::ffi::OsString::from_vec(
        b"/jscycles-origin-\xff/source".to_vec(),
    ));
    let config = jscycles::TsConfig {
        base_url: origin.clone(),
        paths: vec![jscycles::tsconfig::PathAlias {
            prefix: "@/".to_owned(),
            has_wildcard: true,
            replacements: vec![origin.join("*.ts")],
        }],
    };
    assert_eq!(
        config.resolve_alias("@/module"),
        Some(origin.join("module.ts")),
        "native path bytes survive substitution even on a host whose filesystem rejects such filenames"
    );
}
