//! Workspace detection against the shared fixture projects.
#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    reason = "Tests are allowed to use expect/unwrap for simplicity"
)]

use std::path::{Path, PathBuf};

use jscycles::workspace::{Workspace, WorkspaceFormat};

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

#[test]
fn test_pnpm_workspace_discovery() {
    let fixture_path = fixture("pnpm-workspace");

    let workspace = Workspace::discover(&fixture_path)
        .expect("workspace discovery should succeed")
        .expect("fixture should be a workspace");

    assert_eq!(
        workspace.format,
        WorkspaceFormat::Pnpm,
        "should detect pnpm format"
    );
    assert_eq!(workspace.packages.len(), 2, "should find 2 packages");
}

#[test]
fn test_pnpm_workspace_package_names() {
    let fixture_path = fixture("pnpm-workspace");

    let workspace = Workspace::discover(&fixture_path)
        .expect("workspace discovery should succeed")
        .expect("fixture should be a workspace");

    assert!(
        workspace.packages.contains_key("@pnpm-test/a"),
        "should find @pnpm-test/a"
    );
    assert!(
        workspace.packages.contains_key("@pnpm-test/b"),
        "should find @pnpm-test/b"
    );
}

#[test]
fn test_pnpm_workspace_complex_discovery() {
    let fixture_path = fixture("pnpm-workspace-complex");

    let workspace = Workspace::discover(&fixture_path)
        .expect("workspace discovery should succeed")
        .expect("fixture should be a workspace");

    assert_eq!(
        workspace.format,
        WorkspaceFormat::Pnpm,
        "should detect pnpm format"
    );

    // Should find core and utils, but NOT ignored (due to negative glob !packages/ignored)
    assert!(
        workspace.packages.contains_key("@pnpm-complex/core"),
        "should find @pnpm-complex/core"
    );
    assert!(
        workspace.packages.contains_key("@pnpm-complex/utils"),
        "should find @pnpm-complex/utils"
    );
}

#[test]
fn test_pnpm_workspace_root_has_correct_path() {
    let fixture_path = fixture("pnpm-workspace");

    let workspace = Workspace::discover(&fixture_path)
        .expect("workspace discovery should succeed")
        .expect("fixture should be a workspace");

    assert!(
        workspace.root.ends_with("pnpm-workspace"),
        "root should be the fixture directory"
    );
}

#[test]
fn test_npm_workspace_discovery() {
    let fixture_path = fixture("inter-package-simple");

    let workspace = Workspace::discover(&fixture_path)
        .expect("workspace discovery should succeed")
        .expect("fixture should be a workspace");

    assert_eq!(
        workspace.format,
        WorkspaceFormat::Npm,
        "should detect npm format"
    );
}

#[test]
fn test_npm_workspace_package_paths() {
    let fixture_path = fixture("inter-package-simple");

    let workspace = Workspace::discover(&fixture_path)
        .expect("workspace discovery should succeed")
        .expect("fixture should be a workspace");

    // Verify paths are resolved correctly
    let pkg_a_path = workspace
        .packages
        .get("@simple/a")
        .expect("should find @simple/a");
    assert!(
        pkg_a_path.ends_with("packages/pkg-a"),
        "pkg-a path should end with packages/pkg-a"
    );

    let pkg_b_path = workspace
        .packages
        .get("@simple/b")
        .expect("should find @simple/b");
    assert!(
        pkg_b_path.ends_with("packages/pkg-b"),
        "pkg-b path should end with packages/pkg-b"
    );
}

#[test]
fn test_non_workspace_directory() {
    let fixture_path = fixture("simple-cycle");

    let workspace = Workspace::discover(&fixture_path).expect("discovery should not error");

    assert!(
        workspace.is_none(),
        "simple-cycle fixture is not a workspace"
    );
}

#[test]
fn test_tsconfig_aliases_discovery() {
    let fixture_path = fixture("tsconfig-aliases");

    let workspace = Workspace::discover(&fixture_path)
        .expect("workspace discovery should succeed")
        .expect("fixture should be a workspace");

    assert_eq!(
        workspace.format,
        WorkspaceFormat::Pnpm,
        "should detect pnpm format"
    );
    assert_eq!(workspace.packages.len(), 2, "should find 2 packages");
}

#[test]
fn test_tsconfig_aliases_loaded() {
    let fixture_path = fixture("tsconfig-aliases");

    let workspace = Workspace::discover(&fixture_path)
        .expect("workspace discovery should succeed")
        .expect("fixture should be a workspace");

    // Check that aliases are loaded (wildcard aliases are skipped)
    assert_eq!(
        workspace.aliases.len(),
        2,
        "should have 2 non-wildcard aliases"
    );

    // Verify specific aliases
    assert_eq!(
        workspace.aliases.get("@alias-test/package-a"),
        Some(&"@alias-test/pkg-a".to_owned()),
        "package-a alias should map to pkg-a"
    );
    assert_eq!(
        workspace.aliases.get("@alias-test/package-b"),
        Some(&"@alias-test/pkg-b".to_owned()),
        "package-b alias should map to pkg-b"
    );
}

#[test]
fn test_tsconfig_aliases_is_workspace_package() {
    let fixture_path = fixture("tsconfig-aliases");

    let workspace = Workspace::discover(&fixture_path)
        .expect("workspace discovery should succeed")
        .expect("fixture should be a workspace");

    // Direct package names work
    assert!(
        workspace.is_workspace_package("@alias-test/pkg-a"),
        "direct package name should work"
    );
    assert!(
        workspace.is_workspace_package("@alias-test/pkg-b"),
        "direct package name should work"
    );

    // Aliases also work
    assert!(
        workspace.is_workspace_package("@alias-test/package-a"),
        "alias should be recognized as workspace package"
    );
    assert!(
        workspace.is_workspace_package("@alias-test/package-b"),
        "alias should be recognized as workspace package"
    );

    // With subpaths
    assert!(
        workspace.is_workspace_package("@alias-test/package-a/utils"),
        "alias with subpath should be recognized"
    );
}

#[test]
fn test_tsconfig_aliases_resolve_package_name() {
    let fixture_path = fixture("tsconfig-aliases");

    let workspace = Workspace::discover(&fixture_path)
        .expect("workspace discovery should succeed")
        .expect("fixture should be a workspace");

    // Direct names resolve to themselves
    assert_eq!(
        workspace.resolve_package_name("@alias-test/pkg-a"),
        "@alias-test/pkg-a",
        "direct name resolves to itself"
    );

    // Aliases resolve to real package names
    assert_eq!(
        workspace.resolve_package_name("@alias-test/package-a"),
        "@alias-test/pkg-a",
        "alias resolves to real package name"
    );

    // With subpaths
    assert_eq!(
        workspace.resolve_package_name("@alias-test/package-a/utils"),
        "@alias-test/pkg-a",
        "alias with subpath resolves to real package name"
    );
}

#[test]
fn test_tsconfig_aliases_package_path() {
    let fixture_path = fixture("tsconfig-aliases");

    let workspace = Workspace::discover(&fixture_path)
        .expect("workspace discovery should succeed")
        .expect("fixture should be a workspace");

    // Direct package name
    let direct_path = workspace.package_path("@alias-test/pkg-a");
    assert!(
        direct_path.is_some(),
        "direct package name should have path"
    );
    assert!(
        direct_path
            .expect("checked above")
            .ends_with("packages/pkg-a"),
        "path should end with packages/pkg-a"
    );

    // Alias should resolve to same path
    let alias_path = workspace.package_path("@alias-test/package-a");
    assert!(alias_path.is_some(), "alias should resolve to path");
    assert_eq!(
        direct_path, alias_path,
        "alias should resolve to same path as direct name"
    );
}
