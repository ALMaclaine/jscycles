//! tsconfig.json loading and `extends` against the shared fixture projects.
#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    reason = "Tests are allowed to use expect/unwrap for simplicity"
)]

use std::path::{Path, PathBuf};

use jscycles::tsconfig::TsConfig;

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

#[test]
fn test_extends_relative_path_resolution() {
    // Test that relative extends paths work
    let fixture_path = fixture("tsconfig-extends");

    let config_opt = TsConfig::discover(&fixture_path).expect("readable tsconfig");
    assert!(
        config_opt.is_some(),
        "should discover tsconfig with extends"
    );

    let config = config_opt.expect("config should exist");

    // The child's paths option replaces inherited mappings.
    assert!(
        !config.paths.is_empty(),
        "should have paths from extends chain"
    );
}

#[test]
fn test_extends_child_paths_replace_parent_aliases() {
    let fixture_path = fixture("tsconfig-extends");

    let config = TsConfig::discover(&fixture_path)
        .expect("readable tsconfig")
        .expect("should discover config");

    // Only the child's mappings survive the option replacement.
    let alias_prefixes: Vec<&str> = config.paths.iter().map(|a| a.prefix.as_str()).collect();

    assert!(
        alias_prefixes.contains(&"@app/"),
        "should have @app/* from child config"
    );
    assert!(
        !alias_prefixes.contains(&"@shared/"),
        "child paths replace parent aliases"
    );
    assert!(
        !alias_prefixes.contains(&"@base/"),
        "child paths replace grandparent aliases"
    );
}

#[test]
fn test_extends_from_node_modules() {
    let fixture_path = fixture("tsconfig-extends-pkg");

    let config_opt = TsConfig::discover(&fixture_path).expect("readable tsconfig");
    assert!(
        config_opt.is_some(),
        "should discover tsconfig extending from node_modules"
    );

    let config = config_opt.expect("config should exist");

    // The local config's paths replace the company's inherited paths.
    let alias_prefixes: Vec<&str> = config.paths.iter().map(|a| a.prefix.as_str()).collect();

    assert!(
        alias_prefixes.contains(&"@local/"),
        "should have @local/* from child config"
    );
    assert!(
        !alias_prefixes.contains(&"@company/"),
        "child paths replace the inherited paths option"
    );
}
