//! Shared exact-alias targets preserve fallback, public-table and quota contracts.
#![allow(clippy::expect_used, reason = "Fixture failures must fail the test")]

use std::collections::HashMap;
use std::fs;
use std::path::Path;

use jscycles::budget::{AnalysisBudget, AnalysisLimits, Resource};
use jscycles::tsconfig::PathAlias;
use jscycles::workspace::WorkspaceFormat;
use jscycles::{TsConfig, Workspace};

fn workspace(root: &Path) -> Workspace {
    let mut packages = HashMap::new();
    for (name, folder) in [("a", "first"), ("b", "second")] {
        let path = root.join(folder);
        fs::create_dir(&path).expect("package");
        fs::write(path.join("index.ts"), "").expect("index");
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
fn repeated_ordered_targets_preserve_fallback_membership_and_reload_between_calls() {
    let temp = tempfile::tempdir().expect("fixture");
    let root = temp.path();
    let mut selected = workspace(root);
    let nested = root.join("first/nested");
    fs::create_dir(&nested).expect("nested member");
    fs::write(nested.join("index.ts"), "").expect("nested target");
    let _ = selected.packages.insert(
        "c".to_owned(),
        fs::canonicalize(nested).expect("nested identity"),
    );
    fs::write(root.join("outside.ts"), "").expect("nonmember target");
    let config_path = root.join("tsconfig.json");
    fs::write(
        &config_path,
        serde_json::json!({"compilerOptions":{"paths":{
            "@a1":["first/index.ts", "second/index.ts"],
            "@a2":["first/index.ts", "second/index.ts"],
            "@b":["second/index.ts", "first/index.ts"],
            "@fallback1":["generated.ts", "first/index.ts"],
            "@fallback2":["generated.ts", "first/index.ts"],
            "@nested1":["first/nested/index.ts"],
            "@nested2":["first/nested/index.ts"],
            "@outside1":["outside.ts"],
            "@outside2":["outside.ts"]
        }}})
        .to_string(),
    )
    .expect("configuration");
    let config = TsConfig::load(&config_path).expect("ordered loaded table");
    for generated in [false, true] {
        if generated {
            fs::write(root.join("generated.ts"), "").expect("new first fallback");
        }
        selected
            .load_path_aliases(Some(&config), &AnalysisBudget::default())
            .expect("fresh setup");
        for (alias, name) in [
            ("@a1", "a"),
            ("@a2", "a"),
            ("@b", "b"),
            ("@nested1", "c"),
            ("@nested2", "c"),
        ] {
            assert_eq!(selected.resolve_package_name(alias), name);
        }
        for alias in ["@fallback1", "@fallback2"] {
            assert_eq!(
                selected.aliases.get(alias).map(String::as_str),
                (!generated).then_some("a"),
                "each setup observes the current first existing replacement"
            );
        }
        assert!(!selected.aliases.contains_key("@outside1"));
        assert!(!selected.aliases.contains_key("@outside2"));
        assert_eq!(selected.aliases.len(), if generated { 5 } else { 7 });
    }
}

#[test]
fn ineligible_public_tables_do_not_share_results_by_exact_replacements() {
    let temp = tempfile::tempdir().expect("fixture");
    let root = fs::canonicalize(temp.path()).expect("root");
    let mut selected = workspace(&root);
    let same = selected.packages.get("a").expect("a").clone();
    let config = TsConfig {
        base_url: root.clone(),
        paths: vec![
            PathAlias {
                prefix: String::new(),
                has_wildcard: true,
                replacements: vec![root.join("*")],
            },
            PathAlias {
                prefix: "first".to_owned(),
                has_wildcard: false,
                replacements: vec![same.clone()],
            },
            PathAlias {
                prefix: "second".to_owned(),
                has_wildcard: false,
                replacements: vec![same],
            },
        ],
    };
    selected
        .load_path_aliases(Some(&config), &AnalysisBudget::default())
        .expect("caller-owned first-match table");
    assert_eq!(selected.resolve_package_name("first"), "a");
    assert_eq!(
        selected.resolve_package_name("second"),
        "b",
        "an earlier wildcard depends on the alias name even when later exact replacements agree"
    );
}

#[test]
fn distinct_retained_targets_exhaust_payload_without_publishing_partial_aliases() {
    let temp = tempfile::tempdir().expect("fixture");
    let root = temp.path();
    let mut selected = workspace(root);
    let previous = selected.aliases.clone();
    let paths: serde_json::Map<String, serde_json::Value> = (0..64)
        .map(|number| {
            (
                format!("@u{number:04}"),
                serde_json::json!([format!(
                    "first/generated-output/future-module-{number:04}.ts"
                )]),
            )
        })
        .collect();
    let path = root.join("tsconfig.json");
    fs::write(
        &path,
        serde_json::json!({"compilerOptions":{"paths":paths}}).to_string(),
    )
    .expect("unique generated targets");
    let config = TsConfig::load(&path).expect("finite config");
    let budget = AnalysisBudget::new(AnalysisLimits {
        max_graph_bytes: 4096,
        ..AnalysisLimits::default()
    })
    .expect("finite payload budget");
    let error = selected
        .load_path_aliases(Some(&config), &budget)
        .expect_err("retained target paths consume the payload quota");
    assert_eq!(error.resource(), Resource::GraphBytes);
    assert_eq!(
        selected.aliases, previous,
        "keep the previously published map"
    );
    assert_eq!(
        budget.check().expect_err("sticky failure").resource(),
        Resource::GraphBytes
    );
    selected
        .load_path_aliases(Some(&config), &AnalysisBudget::default())
        .expect("a new setup has no retained failed cache");
    assert_eq!(selected.aliases.len(), 64);
    assert!(selected.aliases.values().all(|name| name == "a"));
}
