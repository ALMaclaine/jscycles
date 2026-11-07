//! End-to-end module identity regressions for dotted source names.
#![allow(
    clippy::expect_used,
    reason = "Fixture and subprocess failures must fail tests"
)]

use std::fs;
use std::path::Path;
use std::process::Command;

fn fixture(root: &Path, mappings: &str) {
    fs::write(root.join("package.json"), r#"{"name":"dotted"}"#).expect("manifest");
    fs::write(root.join("jscycles.yaml"), "{}").expect("isolated configuration");
    fs::write(
        root.join("tsconfig.json"),
        format!(r#"{{"compilerOptions":{{"baseUrl":".","paths":{mappings}}}}}"#),
    )
    .expect("aliases");
    fs::create_dir(root.join("src")).expect("source directory");
    fs::write(root.join("src/a.ts"), "import '@local/model.v2';").expect("importer");
}

fn assert_status(root: &Path, expected: i32) -> serde_json::Value {
    let output = Command::new(env!("CARGO_BIN_EXE_jscycles"))
        .args(["--inner", "--json"])
        .arg("--config")
        .arg(root.join("jscycles.yaml"))
        .arg(root)
        .output()
        .expect("run CLI");
    assert_eq!(
        output.status.code(),
        Some(expected),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).expect("JSON report");
    assert_eq!(
        report.get("has_file_cycles"),
        Some(&serde_json::Value::Bool(expected == 1)),
        "status and JSON agree"
    );
    report
}

#[test]
fn dotted_aliases_preserve_the_complete_stem_before_extension_completion() {
    for mappings in [
        r#"{"@local/*":["missing/*","src/*"]}"#,
        r#"{"@local/model.v2":["src/model.v2"]}"#,
    ] {
        for decoy in [false, true] {
            let directory = tempfile::tempdir().expect("temporary directory");
            let root = directory.path();
            fixture(root, mappings);
            fs::write(root.join("src/model.v2.ts"), "import './a';").expect("cycle target");
            if decoy {
                fs::write(root.join("src/model.ts"), "export const unrelated = 1;")
                    .expect("unrelated shorter stem");
            }
            let report = assert_status(root, 1);
            let paths = report["file_cycles"].to_string();
            assert!(
                paths.contains("model.v2.ts"),
                "cycle keeps the dotted identity"
            );
            assert!(!paths.contains("model.ts"), "shorter stem is unrelated");
        }
    }
}

#[test]
fn a_shorter_stem_cannot_invent_an_alias_cycle() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let root = directory.path();
    fixture(root, r#"{"@local/*":["src/*"]}"#);
    fs::write(root.join("src/model.v2.ts"), "export const leaf = 1;").expect("leaf");
    fs::write(root.join("src/model.ts"), "import './a';").expect("unrelated importer");
    let _report = assert_status(root, 0);
}

#[cfg(unix)]
#[test]
fn alias_and_relative_dot_segments_agree_when_an_intermediate_directory_is_absent() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let root = directory.path();
    fixture(root, r#"{"@local/b":["src/absent/../b"]}"#);
    fs::write(root.join("src/b.ts"), "import './a';").expect("would-be cycle target");
    for specifier in ["./absent/../b", "@local/b"] {
        fs::write(root.join("src/a.ts"), format!("import '{specifier}';"))
            .expect("source with missing intermediate directory");
        let _report = assert_status(root, 1);
    }
    fs::create_dir(root.join("src/absent")).expect("make intermediate directory real");
    for specifier in ["./absent/../b", "@local/b"] {
        fs::write(root.join("src/a.ts"), format!("import '{specifier}';"))
            .expect("source with existing intermediate directory");
        let _report = assert_status(root, 1);
    }
}

#[cfg(unix)]
#[test]
fn alias_and_relative_parent_components_are_normalized_before_following_symlinks() {
    for physical_cycle in [false, true] {
        let directory = tempfile::tempdir().expect("temporary directory");
        let root = directory.path();
        fixture(root, r#"{"@local/b":["src/bridge/../b"]}"#);
        fs::create_dir_all(root.join("external/child")).expect("physical directory");
        std::os::unix::fs::symlink("../external/child", root.join("src/bridge"))
            .expect("import-path directory symlink");
        let physical = if physical_cycle {
            "import '../src/a';"
        } else {
            "export const physical = 1;"
        };
        let lexical = if physical_cycle {
            "export const decoy = 1;"
        } else {
            "import './a';"
        };
        fs::write(root.join("external/b.ts"), physical).expect("physical target");
        fs::write(root.join("src/b.ts"), lexical).expect("lexical decoy");
        for specifier in ["./bridge/../b", "@local/b"] {
            fs::write(root.join("src/a.ts"), format!("import '{specifier}';"))
                .expect("symlink-parent import");
            let report = assert_status(root, i32::from(!physical_cycle));
            if !physical_cycle {
                assert!(
                    report["file_cycles"].to_string().contains("src/b.ts"),
                    "the cycle witness uses the normalized module path"
                );
            }
        }
    }
}

#[test]
fn dotted_stems_keep_each_supported_extension_and_directory_identity() {
    for target in [
        "model.v2.tsx",
        "model.v2.js",
        "model.v2.jsx",
        "model.v2/index.ts",
    ] {
        let directory = tempfile::tempdir().expect("temporary directory");
        let root = directory.path();
        fixture(root, r#"{"@local/*":["src/*"]}"#);
        let target = root.join("src").join(target);
        fs::create_dir_all(target.parent().expect("parent directory")).expect("target directory");
        let relative = if target.file_name().is_some_and(|name| name == "index.ts") {
            "../a"
        } else {
            "./a"
        };
        fs::write(&target, format!("import '{relative}';")).expect("cycle target");
        fs::write(root.join("src/model.ts"), "export const unrelated = 1;").expect("decoy");
        let _report = assert_status(root, 1);
    }
}

#[test]
fn known_javascript_extensions_keep_alias_source_substitution() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let root = directory.path();
    fixture(root, r#"{"@local/model.v2":["src/model.v2.js"]}"#);
    fs::write(root.join("src/model.v2.ts"), "import './a';").expect("TypeScript source");
    let _report = assert_status(root, 1);
}

#[test]
fn javascript_relative_specifiers_resolve_their_typescript_sources() {
    for (specifier, filename) in [("model.v2.js", "model.v2.ts"), ("view.jsx", "view.tsx")] {
        let directory = tempfile::tempdir().expect("temporary directory");
        let root = directory.path();
        fixture(root, "{}");
        fs::write(root.join("src/a.ts"), format!("import './{specifier}';"))
            .expect("runtime extension import");
        fs::write(root.join("src").join(filename), "import './a.js';")
            .expect("TypeScript source cycle");
        let report = assert_status(root, 1);
        assert!(
            report["file_cycles"].to_string().contains(filename),
            "report the physical source identity"
        );
    }
}

#[test]
fn an_existing_exact_source_wins_over_extension_completion() {
    for import in ["./model.v2.js", "@local/model.v2.js"] {
        let directory = tempfile::tempdir().expect("temporary directory");
        let root = directory.path();
        fixture(root, r#"{"@local/*":["src/*"]}"#);
        fs::write(root.join("src/a.ts"), format!("import '{import}';")).expect("importer");
        fs::write(root.join("src/model.v2.js"), "export const leaf = 1;").expect("exact source");
        fs::write(root.join("src/model.v2.ts"), "import './a';").expect("other source");
        let _report = assert_status(root, 0);
    }
}

#[test]
fn a_runtime_extension_is_substituted_without_appending_a_second_extension() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let root = directory.path();
    fixture(root, "{}");
    fs::write(root.join("src/a.ts"), "import './model.js';").expect("runtime extension import");
    fs::write(root.join("src/model.js.ts"), "import './a';").expect("unrelated compound extension");
    let _report = assert_status(root, 0);
}

#[test]
fn esm_and_commonjs_runtime_suffixes_keep_their_typescript_families() {
    for (runtime, source) in [("mjs", "mts"), ("cjs", "cts")] {
        for prefix in ["./", "@local/"] {
            let directory = tempfile::tempdir().expect("temporary directory");
            let root = directory.path();
            fixture(root, r#"{"@local/*":["src/*"]}"#);
            fs::write(
                root.join("jscycles.yaml"),
                "defaults:\n  extensions: [ts, tsx, js, jsx, mts, cts, mjs, cjs]\n",
            )
            .expect("explicit module extensions");
            fs::write(
                root.join("src/a.ts"),
                format!("import '{prefix}model.v2.{runtime}';"),
            )
            .expect("runtime extension import");
            fs::write(
                root.join("src").join(format!("model.v2.{source}")),
                "import './a.js';",
            )
            .expect("TypeScript module source");
            let _report = assert_status(root, 1);
        }
    }
}

#[test]
fn modern_runtime_suffixes_cannot_resolve_an_unrelated_typescript_family() {
    for runtime in ["mjs", "cjs"] {
        let directory = tempfile::tempdir().expect("temporary directory");
        let root = directory.path();
        fixture(root, r#"{"@local/*":["src/*"]}"#);
        fs::write(
            root.join("src/a.ts"),
            format!("import '@local/model.v2.{runtime}';"),
        )
        .expect("runtime extension import");
        fs::write(root.join("src/model.v2.ts"), "import './a';").expect("other module family");
        let _report = assert_status(root, 0);
    }
}

#[test]
fn repeated_extractions_observe_alias_files_appearing_changing_and_disappearing() {
    use jscycles::{ImportExtractor, ImportTarget, tsconfig::TsConfig};

    let directory = tempfile::tempdir().expect("temporary directory");
    let root = directory.path();
    fixture(root, r#"{"@local/*":["src/*"]}"#);
    let config = TsConfig::load(&root.join("tsconfig.json")).expect("aliases");
    let extractor = ImportExtractor::new(vec!["ts".to_owned(), "tsx".to_owned()], Some(config));
    for suffix in [None, Some("ts"), Some("tsx"), None] {
        for extension in ["ts", "tsx"] {
            let path = root.join(format!("src/model.v2.{extension}"));
            if path.exists() {
                fs::remove_file(path).expect("remove earlier target");
            }
        }
        let expected = suffix.map_or_else(
            || ImportTarget::Unresolved("@local/model.v2".to_owned()),
            |suffix| {
                let target = root.join(format!("src/model.v2.{suffix}"));
                fs::write(&target, "export const leaf = 1;").expect("new target");
                ImportTarget::Resolved(fs::canonicalize(target).expect("canonical identity"))
            },
        );
        let imports = extractor
            .extract(root)
            .expect("new scan of stable checkout");
        assert_eq!(imports.len(), 1, "one importing source");
        assert_eq!(
            imports[0].target, expected,
            "observe this extraction's files"
        );
    }
}

#[test]
fn parallel_repeated_aliases_keep_canonical_file_identity_and_missing_classification() {
    use jscycles::{ImportExtractor, ImportTarget, tsconfig::TsConfig};

    let directory = tempfile::tempdir().expect("temporary directory");
    let root = directory.path();
    fixture(root, r#"{"@local/*":["src/*"]}"#);
    fs::remove_file(root.join("src/a.ts")).expect("replace the initial source");
    for number in 0..16 {
        fs::write(
            root.join(format!("src/caller{number}.ts")),
            "import '@local/model.v2'; require('@local/model.v2'); import('@local/model.v2');\nimport '@local/missing'; require('@local/missing'); import './model.v2';",
        )
        .expect("repeated file and missing aliases");
    }
    let target = root.join("src/model.v2.ts");
    fs::write(&target, "export const leaf = 1;").expect("target");
    let canonical = fs::canonicalize(target).expect("canonical target");
    let config = TsConfig::load(&root.join("tsconfig.json")).expect("aliases");
    let imports = ImportExtractor::new(vec!["ts".to_owned()], Some(config))
        .extract(root)
        .expect("repeated references");
    assert_eq!(imports.len(), 96, "retain every literal reference");
    assert_eq!(
        imports
            .iter()
            .map(|import| &import.source)
            .collect::<std::collections::HashSet<_>>()
            .len(),
        16,
        "every parallel source contributes"
    );
    let mut resolved = 0;
    let mut missing = 0;
    for import in imports {
        if import.specifier == "@local/missing" {
            assert_eq!(
                import.target,
                ImportTarget::Unresolved(import.specifier),
                "a matched missing alias never becomes an external package"
            );
            missing += 1;
        } else {
            assert_eq!(
                import.target,
                ImportTarget::Resolved(canonical.clone()),
                "cold, repeated and relative references share the same file identity"
            );
            resolved += 1;
        }
    }
    assert_eq!(
        (resolved, missing),
        (64, 32),
        "both alias outcomes are stable"
    );
}

#[test]
fn unused_alias_tables_do_not_exhaust_repeated_relative_workspace_or_cached_imports() {
    use jscycles::budget::{AnalysisBudget, AnalysisLimits};
    use jscycles::tsconfig::{PathAlias, TsConfig};
    use jscycles::{ImportExtractor, ImportTarget, Workspace};

    let directory = tempfile::tempdir().expect("temporary directory");
    let root = directory.path();
    fs::write(root.join("leaf.ts"), "export const leaf = 1;").expect("leaf source");
    let mut paths: Vec<_> = (0..1024)
        .map(|number| PathAlias {
            prefix: format!("@unused/{number}"),
            has_wildcard: false,
            replacements: vec![root.join("missing")],
        })
        .collect();
    paths.push(PathAlias {
        prefix: "@used".to_owned(),
        has_wildcard: false,
        replacements: vec![root.join("leaf")],
    });
    paths.push(PathAlias {
        prefix: "@missing".to_owned(),
        has_wildcard: false,
        replacements: vec![root.join("missing")],
    });
    paths.push(PathAlias {
        prefix: "known".to_owned(),
        has_wildcard: false,
        replacements: vec![root.join("leaf")],
    });
    let config = TsConfig {
        base_url: root.to_path_buf(),
        paths,
    };
    let workspace = Workspace {
        root: root.to_path_buf(),
        format: jscycles::workspace::WorkspaceFormat::Npm,
        packages: [("known".to_owned(), root.to_path_buf())].into(),
        aliases: std::collections::HashMap::new(),
    };
    let physical = fs::canonicalize(root.join("leaf.ts")).expect("canonical leaf");
    for (specifier, expected) in [
        ("./leaf", ImportTarget::Resolved(physical.clone())),
        ("@used", ImportTarget::Resolved(physical)),
        ("@missing", ImportTarget::Unresolved("@missing".to_owned())),
        (
            "known",
            ImportTarget::WorkspacePackage {
                package_name: "known".to_owned(),
                subpath: None,
            },
        ),
    ] {
        fs::write(
            root.join("entry.ts"),
            format!("import '{specifier}';\n").repeat(96),
        )
        .expect("repeated source");
        let budget = AnalysisBudget::new(AnalysisLimits {
            max_work: 10000,
            ..AnalysisLimits::default()
        })
        .expect("finite extraction budget");
        let imports = ImportExtractor::new(vec!["ts".to_owned()], Some(config.clone()))
            .with_shared_workspace(Some(std::sync::Arc::new(workspace.clone())))
            .with_budget(budget.clone())
            .extract(root)
            .expect("do not charge alias traversal when no traversal runs");
        assert_eq!(imports.len(), 96, "retain every import");
        assert!(
            imports.iter().all(|import| import.target == expected),
            "preserve target identity for {specifier}"
        );
        budget.check().expect("actual lookup work fits the budget");
    }
}

#[test]
fn uncached_alias_searches_still_exhaust_the_work_budget() {
    use jscycles::ImportExtractor;
    use jscycles::budget::{AnalysisBudget, AnalysisLimits, Resource};
    use jscycles::tsconfig::{PathAlias, TsConfig};

    let directory = tempfile::tempdir().expect("temporary directory");
    let root = directory.path();
    let mut source = String::new();
    for number in 0..96 {
        use std::fmt::Write as _;
        writeln!(source, "import 'unknown{number}';").expect("source string");
    }
    fs::write(root.join("entry.ts"), source).expect("distinct unmatched imports");
    for replacements in [1, 16] {
        let config = TsConfig {
            base_url: root.to_path_buf(),
            paths: (0..64)
                .map(|number| PathAlias {
                    prefix: format!("@unused/{number}"),
                    has_wildcard: false,
                    replacements: vec![root.join("missing"); replacements],
                })
                .collect(),
        };
        let budget = AnalysisBudget::new(AnalysisLimits {
            max_work: 10000,
            ..AnalysisLimits::default()
        })
        .expect("finite matching budget");
        let result = ImportExtractor::new(vec!["ts".to_owned()], Some(config))
            .with_budget(budget.clone())
            .extract(root);
        assert!(
            matches!(result, Err(jscycles::ExtractError::Limit(_))),
            "actual uncached searches must remain bounded"
        );
        assert_eq!(
            budget.check().expect_err("sticky exhaustion").resource(),
            Resource::Work,
            "the lookup failed its work admission"
        );
    }
}
