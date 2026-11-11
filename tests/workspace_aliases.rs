//! Workspace name precedence, alias-table cost and public configuration behavior.
#![allow(clippy::expect_used, reason = "Fixture failures must fail the test")]

use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::process::Command;

use jscycles::budget::{AnalysisBudget, AnalysisLimits, Resource};
use jscycles::tsconfig::PathAlias;
use jscycles::workspace::WorkspaceFormat;
use jscycles::{TsConfig, Workspace};

fn workspace(root: &Path) -> Workspace {
    let mut packages = HashMap::new();
    for name in ["a", "b"] {
        let path = root.join("packages").join(name);
        fs::create_dir_all(&path).expect("package directory");
        let _ = packages.insert(name.to_owned(), fs::canonicalize(path).expect("identity"));
    }
    Workspace {
        root: root.to_path_buf(),
        format: WorkspaceFormat::Npm,
        packages,
        aliases: [("sentinel".to_owned(), "a".to_owned())].into(),
    }
}

#[test]
fn known_workspace_names_win_over_conflicting_tsconfig_aliases() {
    for real_cycle in [false, true] {
        let temp = tempfile::tempdir().expect("temporary workspace");
        let root = temp.path();
        fs::write(
            root.join("package.json"),
            r#"{"workspaces":["packages/*"]}"#,
        )
        .expect("workspace");
        fs::write(root.join("jscycles.yaml"), "{}").expect("configuration");
        fs::write(
            root.join("tsconfig.json"),
            r#"{"compilerOptions":{"paths":{"a":["packages/b/index.ts"]}}}"#,
        )
        .expect("conflicting alias");
        let sources = if real_cycle {
            ["import 'b';", "import 'a';", "export const c = 1;"]
        } else {
            ["export const a = 1;", "import 'c';", "import 'a';"]
        };
        for (name, source) in ["a", "b", "c"].into_iter().zip(sources) {
            let folder = root.join("packages").join(name);
            fs::create_dir_all(&folder).expect("member");
            fs::write(
                folder.join("package.json"),
                serde_json::json!({"name":name}).to_string(),
            )
            .expect("manifest");
            fs::write(folder.join("index.ts"), source).expect("source");
        }
        for outer in [false, true] {
            let mut command = Command::new(env!("CARGO_BIN_EXE_jscycles"));
            let _ = command.current_dir(root).args(["--json"]);
            if outer {
                let _ = command.arg("--outer");
            }
            let result = command.output().expect("CLI starts");
            assert_eq!(
                result.status.code(),
                Some(i32::from(real_cycle)),
                "known package identity must win; stdout={}, stderr={}",
                String::from_utf8_lossy(&result.stdout),
                String::from_utf8_lossy(&result.stderr)
            );
            let report: serde_json::Value = serde_json::from_slice(&result.stdout).expect("JSON");
            assert_eq!(
                report["has_package_cycles"], real_cycle,
                "package graph follows declared names"
            );
        }
    }
}

#[test]
fn loaded_alias_tables_fit_linear_work_and_fail_without_replacing_old_mappings() {
    let temp = tempfile::tempdir().expect("temporary workspace");
    let root = temp.path();
    let original = workspace(root);
    let mut paths: serde_json::Map<String, serde_json::Value> = (0..1024)
        .map(|number| {
            (
                format!("@alias{number:04}"),
                serde_json::json!(["packages/a"]),
            )
        })
        .collect();
    let _ = paths.insert(
        "@wildcard-prefix-longer-than-every-exact/*".to_owned(),
        serde_json::json!(["packages/b/*"]),
    );
    let file = root.join("tsconfig.json");
    fs::write(
        &file,
        serde_json::json!({"compilerOptions":{"paths":paths}}).to_string(),
    )
    .expect("alias configuration");
    let config = TsConfig::load(&file).expect("validated loaded table");
    let mut loaded = original.clone();
    let successful_budget = AnalysisBudget::new(AnalysisLimits {
        max_work: 10000,
        ..AnalysisLimits::default()
    })
    .expect("finite work");
    loaded
        .load_path_aliases(Some(&config), &successful_budget)
        .expect("one lookup per declared exact alias");
    assert_eq!(
        loaded.aliases.len(),
        1024,
        "retain all exact mappings and skip wildcard package aliases"
    );
    assert!(
        loaded.aliases.values().all(|name| name == "a"),
        "keep target package identity"
    );
    successful_budget
        .check()
        .expect("linear matching fits the work quota");

    for (limits, resource) in [
        (
            AnalysisLimits {
                max_work: 1024,
                ..AnalysisLimits::default()
            },
            Resource::Work,
        ),
        (
            AnalysisLimits {
                max_work: 3000,
                ..AnalysisLimits::default()
            },
            Resource::Work,
        ),
        (
            AnalysisLimits {
                max_graph_bytes: 1024,
                ..AnalysisLimits::default()
            },
            Resource::GraphBytes,
        ),
    ] {
        let budget = AnalysisBudget::new(limits).expect("finite quota");
        let mut failed = original.clone();
        let error = failed
            .load_path_aliases(Some(&config), &budget)
            .expect_err("bounded admission");
        assert_eq!(error.resource(), resource, "preserve the failed admission");
        assert_eq!(
            failed.aliases, original.aliases,
            "publish no partial alias replacement"
        );
        assert_eq!(
            budget.check().expect_err("sticky failure").resource(),
            resource,
            "exhaustion remains visible"
        );
    }
}

#[test]
fn caller_constructed_alias_tables_preserve_first_match_order() {
    let temp = tempfile::tempdir().expect("temporary workspace");
    let original = workspace(temp.path());
    let a = original.packages.get("a").expect("a").clone();
    let b = original.packages.get("b").expect("b").clone();
    let exact_a = PathAlias {
        prefix: "alias".to_owned(),
        has_wildcard: false,
        replacements: vec![a],
    };
    let exact_b = PathAlias {
        prefix: "alias".to_owned(),
        has_wildcard: false,
        replacements: vec![b.clone()],
    };
    let wildcard_b = PathAlias {
        prefix: String::new(),
        has_wildcard: true,
        replacements: vec![b.join("*")],
    };
    let empty = PathAlias {
        prefix: "alias".to_owned(),
        has_wildcard: false,
        replacements: vec![],
    };
    for paths in [
        vec![wildcard_b.clone(), exact_a.clone()],
        vec![exact_b, exact_a],
        vec![empty, wildcard_b],
    ] {
        let config = TsConfig {
            base_url: temp.path().to_path_buf(),
            paths,
        };
        let mut selected = original.clone();
        selected
            .load_path_aliases(Some(&config), &AnalysisBudget::default())
            .expect("legacy public table");
        assert_eq!(
            selected.aliases.get("alias").map(String::as_str),
            Some("b"),
            "the first matching replacement wins for public vectors"
        );
        assert_eq!(
            selected.resolve_package_name("alias/subpath"),
            "b",
            "canonical name follows the same table"
        );
        assert_eq!(
            selected.get_package_path("alias/subpath"),
            Some(&b),
            "path and name agree"
        );
    }
}
