//! Every path-taking input must mean the same thing however it is spelled:
//! `x`, `./x`, `x/`, absolute, and `..` detours. Results are compared after
//! dropping the `path` display field, which deliberately keeps the caller's
//! spelling. Each baseline is also checked to be non-trivial, so a spelling
//! that silently matches nothing cannot pass by agreeing with another failure.
#![allow(
    clippy::expect_used,
    reason = "Filesystem/process setup failures must fail tests"
)]

use std::fs;
use std::path::Path;
use std::process::Command;

fn write(root: &Path, relative: &str, contents: &str) {
    let path = root.join(relative);
    fs::create_dir_all(path.parent().expect("parent")).expect("directories");
    fs::write(path, contents).expect("file");
}

/// Workspace with root package, members `a` and `b`, and cycles in
/// `a/src` (via a tsconfig alias), `a/src/legacy` and `b`.
fn fixture() -> (tempfile::TempDir, std::path::PathBuf) {
    let temp = tempfile::tempdir().expect("directory");
    let root = fs::canonicalize(temp.path()).expect("canonical root");
    write(
        &root,
        "package.json",
        r#"{"name":"root","workspaces":["packages/*"]}"#,
    );
    write(&root, "packages/a/package.json", r#"{"name":"a"}"#);
    write(&root, "packages/b/package.json", r#"{"name":"b"}"#);
    write(&root, "packages/a/src/x.ts", "import '@a/y';");
    write(&root, "packages/a/src/y.ts", "import './x';");
    write(&root, "packages/a/src/u.ts", "import './v';");
    write(&root, "packages/a/src/v.ts", "import './u';");
    write(&root, "packages/a/src/legacy/p.ts", "import './q';");
    write(&root, "packages/a/src/legacy/q.ts", "import './p';");
    write(&root, "packages/b/m.ts", "import './n';");
    write(&root, "packages/b/n.ts", "import './m';");
    write(&root, "src/r.ts", "import './s';");
    write(&root, "src/s.ts", "import './r';");
    write(
        &root,
        "configs/aliases.json",
        r#"{"compilerOptions":{"baseUrl":"..","paths":{"@a/*":["packages/a/src/*"]}}}"#,
    );
    (temp, root)
}

/// Package name -> sorted cycles, plus package cycles; `None` on a non-1/0 exit.
fn outcome(root: &Path, args: &[&str]) -> (Option<i32>, String) {
    let output = Command::new(env!("CARGO_BIN_EXE_jscycles"))
        .current_dir(root)
        .args(["--inner", "--json"])
        .args(args)
        .output()
        .expect("CLI starts");
    let status = output.status.code();
    if !matches!(status, Some(0 | 1)) {
        return (status, String::from_utf8_lossy(&output.stderr).into_owned());
    }
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).expect("JSON");
    let mut packages: Vec<String> = report
        .get("file_cycles")
        .and_then(serde_json::Value::as_array)
        .expect("file cycles")
        .iter()
        .map(|entry| {
            let field = |key| entry.get(key).map_or_else(String::new, ToString::to_string);
            format!("{}={}", field("name"), field("cycles"))
        })
        .collect();
    packages.sort();
    (status, packages.join("\n"))
}

fn assert_same(root: &Path, label: &str, spellings: &[Vec<String>], expect: impl Fn(&str) -> bool) {
    let render = |args: &Vec<String>| {
        let args: Vec<&str> = args.iter().map(String::as_str).collect();
        outcome(root, &args)
    };
    let (first, rest) = spellings.split_first().expect("at least one spelling");
    let baseline = render(first);
    assert!(
        matches!(baseline.0, Some(0 | 1)) && expect(&baseline.1),
        "{label}: unexpected baseline {:?} for {first:?}:\n{}",
        baseline.0,
        baseline.1
    );
    for spelling in rest {
        assert_eq!(
            render(spelling),
            baseline,
            "{label}: {spelling:?} differs from {first:?}"
        );
    }
}

fn strings(items: &[&str]) -> Vec<String> {
    items.iter().map(|item| (*item).to_owned()).collect()
}

#[test]
fn operands_mean_the_same_however_spelled() {
    let (_temp, root) = fixture();
    let absolute = root.display().to_string();
    let roots = [
        ".",
        "./",
        "packages/..",
        "packages/a/../..",
        absolute.as_str(),
    ];
    let spellings: Vec<Vec<String>> = roots.iter().map(|r| strings(&[r])).collect();
    assert_same(&root, "root operand", &spellings, |out| {
        out.contains("\"root\"") && out.contains("legacy/p.ts")
    });

    let member = format!("{absolute}/packages/a");
    let members = [
        "packages/a",
        "./packages/a",
        "packages/a/",
        "packages/b/../a",
        member.as_str(),
        "./packages/./a",
    ];
    let member_spellings: Vec<Vec<String>> = members.iter().map(|m| strings(&[m])).collect();
    assert_same(&root, "member operand", &member_spellings, |out| {
        out.contains("legacy/p.ts") && !out.contains("\"b\"")
    });

    let manifests = [
        "packages/a/package.json",
        "./packages/a/package.json",
        "packages/b/../a/package.json",
    ];
    let manifest_spellings: Vec<Vec<String>> = manifests.iter().map(|m| strings(&[m])).collect();
    assert_same(&root, "manifest operand", &manifest_spellings, |out| {
        out.contains("\"a\"")
    });
}

#[test]
fn scan_excludes_mean_the_same_however_spelled() {
    let (_temp, root) = fixture();
    let absolute = format!("{}/packages/a/src/legacy", root.display());
    let excludes = [
        "packages/a/src/legacy",
        "./packages/a/src/legacy",
        "packages/a/src/legacy/",
        "**/legacy",
        absolute.as_str(),
    ];
    let mut spellings = Vec::new();
    for (index, exclude) in excludes.iter().enumerate() {
        let name = format!("exclude{index}.yaml");
        write(
            &root,
            &name,
            &format!("scan:\n  exclude:\n    - {exclude:?}\n"),
        );
        spellings.push(vec!["--config".to_owned(), name]);
    }
    assert_same(&root, "scan.exclude", &spellings, |out| {
        !out.contains("legacy") && out.contains("src/u.ts") && out.contains("m.ts")
    });
    // Absolute excludes also hold for operands spelled with `..`.
    let with_operand: Vec<Vec<String>> = ["packages/a", "packages/b/../a", "./packages/a/"]
        .iter()
        .map(|op| strings(&["--config", "exclude4.yaml", op]))
        .collect();
    assert_same(
        &root,
        "absolute exclude with operands",
        &with_operand,
        |out| !out.contains("legacy") && out.contains("src/u.ts"),
    );
}

#[test]
fn scan_includes_mean_the_same_however_spelled() {
    let (_temp, root) = fixture();
    let absolute = format!("{}/packages", root.display());
    let includes = [
        "packages/*",
        "./packages/*",
        "packages/*/",
        "packages",
        "packages/",
        absolute.as_str(),
    ];
    let mut spellings = Vec::new();
    for (index, include) in includes.iter().enumerate() {
        let name = format!("include{index}.yaml");
        write(
            &root,
            &name,
            &format!("scan:\n  include:\n    - {include:?}\n"),
        );
        spellings.push(vec!["--config".to_owned(), name]);
    }
    assert_same(&root, "scan.include", &spellings, |out| {
        out.contains("\"a\"") && out.contains("\"b\"") && !out.contains("\"root\"")
    });
}

#[test]
fn package_ignores_mean_the_same_however_spelled() {
    let (_temp, root) = fixture();
    let absolute = format!("{}/packages/a/src/legacy", root.display());
    let ignores = [
        "src/legacy",
        "./src/legacy",
        "src/legacy/",
        "src/legacy/**",
        "**/legacy/*.ts",
        absolute.as_str(),
    ];
    let mut spellings = Vec::new();
    for (index, ignore) in ignores.iter().enumerate() {
        let name = format!("ignore{index}.yaml");
        write(
            &root,
            &name,
            &format!("packages:\n  a:\n    ignore:\n      - {ignore:?}\n"),
        );
        spellings.push(vec!["--config".to_owned(), name]);
    }
    assert_same(&root, "package ignore", &spellings, |out| {
        !out.contains("legacy") && out.contains("src/u.ts")
    });
}

#[test]
fn config_and_tsconfig_paths_mean_the_same_however_spelled() {
    let (_temp, root) = fixture();
    write(
        &root,
        "jscycles.yaml",
        "scan:\n  exclude:\n    - \"packages/b\"\n",
    );
    let absolute = root.join("jscycles.yaml").display().to_string();
    let configs = [
        "jscycles.yaml",
        "./jscycles.yaml",
        "configs/../jscycles.yaml",
        absolute.as_str(),
    ];
    let spellings: Vec<Vec<String>> = configs.iter().map(|c| strings(&["--config", c])).collect();
    assert_same(&root, "--config", &spellings, |out| {
        !out.contains("m.ts") && out.contains("legacy")
    });

    let alias_config = root.join("configs/aliases.json").display().to_string();
    let tsconfigs = [
        "configs/aliases.json",
        "./configs/aliases.json",
        "src/../configs/aliases.json",
        alias_config.as_str(),
    ];
    let tsconfig_spellings: Vec<Vec<String>> = tsconfigs
        .iter()
        .map(|t| strings(&["--tsconfig", t]))
        .collect();
    assert_same(&root, "--tsconfig", &tsconfig_spellings, |out| {
        out.contains("src/x.ts")
    });
}
