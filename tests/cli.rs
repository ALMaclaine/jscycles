//! Process-level tests of scan completion, configuration, and output failures.
#![allow(
    clippy::expect_used,
    reason = "Process-test setup failures should fail the test"
)]

use std::fs;
use std::io::Write as _;
use std::path::Path;
use std::process::{Command, Output, Stdio};

use tempfile::tempdir;

fn scan(root: &Path, arguments: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_jscycles"))
        .current_dir(root)
        .args(arguments)
        .output()
        .expect("CLI should start")
}

fn package(root: &Path) {
    fs::write(root.join("package.json"), r#"{"name":"example"}"#)
        .expect("manifest should be written");
    fs::write(root.join("a.ts"), "export const a = 1;").expect("source should be written");
}

#[test]
fn argument_errors_escape_controls_and_bidi_without_destroying_usage_layout() {
    let temp = tempdir().expect("temporary directory");
    for character in [
        '\n', '\t', '\u{1b}', '\u{9b}', '\u{061c}', '\u{200e}', '\u{200f}', '\u{202a}', '\u{202b}',
        '\u{202c}', '\u{202d}', '\u{202e}', '\u{2066}', '\u{2067}', '\u{2068}', '\u{2069}',
        '\u{2028}', '\u{2029}',
    ] {
        let unknown = format!("--bad{character}flag");
        let extension = format!("ts{character}");
        for arguments in [
            vec![unknown.as_str()],
            vec!["--extensions", extension.as_str()],
        ] {
            let output = scan(temp.path(), &arguments);
            assert_eq!(output.status.code(), Some(2), "invalid arguments fail");
            assert!(output.stdout.is_empty(), "no report for parser errors");
            let diagnostic = String::from_utf8(output.stderr).expect("UTF-8 diagnostic");
            assert!(
                diagnostic.contains(&character.escape_unicode().to_string()),
                "the argument is visible as an escape: {diagnostic:?}"
            );
            if !matches!(character, '\n' | '\t') {
                assert!(!diagnostic.contains(character), "no raw terminal control");
            }
            if arguments.len() == 1 {
                assert!(
                    diagnostic.contains("\nUsage:"),
                    "preserve generated usage layout"
                );
            }
        }
    }
}

#[test]
fn valid_paths_keep_their_original_unicode_and_control_characters() {
    let temp = tempdir().expect("temporary directory");
    for name in ["ordinary-λ", "direction-\u{202e}"] {
        let root = temp.path().join(name);
        fs::create_dir(&root).expect("package directory");
        package(&root);
        fs::write(root.join("a.ts"), "import './b';").expect("source a");
        fs::write(root.join("b.ts"), "import './a';").expect("source b");
        let output = Command::new(env!("CARGO_BIN_EXE_jscycles"))
            .arg(&root)
            .args(["--inner", "--json", "--no-tsconfig"])
            .output()
            .expect("CLI starts");
        assert_eq!(output.status.code(), Some(1), "preserve the selected path");
        let report: serde_json::Value = serde_json::from_slice(&output.stdout).expect("JSON");
        assert!(
            report["file_cycles"].to_string().contains(name),
            "preserve path identity"
        );
    }
}

fn workspace(root: &Path, imports_cycle: bool, manifests_cycle: bool) {
    fs::write(
        root.join("package.json"),
        r#"{"name":"root","workspaces":["packages/*"]}"#,
    )
    .expect("workspace manifest");
    for (name, other) in [("a", "b"), ("b", "a"), ("c", "c")] {
        let path = root.join("packages").join(name);
        fs::create_dir_all(&path).expect("package directory");
        let dependency = if manifests_cycle { other } else { "c" };
        fs::write(
            path.join("package.json"),
            format!(r#"{{"name":"{name}","dependencies":{{"{dependency}":"workspace:*"}}}}"#),
        )
        .expect("package manifest");
        let source = if imports_cycle && name != "c" {
            format!("import {{ value }} from '{other}'; export const local = value;")
        } else {
            "export const value = 1;".to_owned()
        };
        fs::write(path.join("index.ts"), source).expect("package source");
    }
}

#[test]
fn adherence_outer_is_a_filter_of_the_same_import_graph() {
    for (imports_cycle, manifests_cycle) in [(true, false), (false, true)] {
        let temp = tempdir().expect("temporary directory");
        workspace(temp.path(), imports_cycle, manifests_cycle);
        for explicit in [false, true] {
            let mut arguments = vec!["--json"];
            if explicit {
                arguments.push(".");
            }
            let full = scan(temp.path(), &arguments);
            arguments.push("--outer");
            let outer = scan(temp.path(), &arguments);
            let expected = i32::from(imports_cycle);
            assert_eq!(
                full.status.code(),
                Some(expected),
                "source imports determine the full result"
            );
            assert_eq!(
                outer.status.code(),
                Some(expected),
                "filtering must preserve package-cycle status"
            );
            let full: serde_json::Value = serde_json::from_slice(&full.stdout).expect("full JSON");
            let outer: serde_json::Value =
                serde_json::from_slice(&outer.stdout).expect("outer JSON");
            assert_eq!(
                full["package_cycles"], outer["package_cycles"],
                "same package graph in both modes"
            );
        }
    }
}

#[test]
fn adherence_automatic_scans_use_configured_include_patterns() {
    let temp = tempdir().expect("temporary directory");
    workspace(temp.path(), true, false);
    fs::write(
        temp.path().join("jscycles.yaml"),
        "scan:\n  include: ['packages/a']\n",
    )
    .expect("scan selection");
    for filter in ["--inner", "--outer"] {
        let output = scan(temp.path(), &["--json", filter]);
        assert_eq!(
            output.status.code(),
            Some(0),
            "only package a is selected: {output:?}"
        );
    }
    let output = scan(temp.path(), &["--json", "--inner"]);
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).expect("scan JSON");
    let packages: Vec<_> = json["file_cycles"]
        .as_array()
        .expect("package reports")
        .iter()
        .map(|package| package["name"].as_str().expect("package name"))
        .collect();
    assert_eq!(
        packages,
        vec!["a"],
        "scan.include selects automatic package roots"
    );
    assert_eq!(
        scan(temp.path(), &["--json", ".", "--outer"]).status.code(),
        Some(1),
        "explicit directory overrides automatic includes and scans members"
    );
}

#[test]
fn adherence_alias_cycles_use_consistent_source_paths() {
    let temp = tempdir().expect("temporary directory");
    package(temp.path());
    fs::create_dir(temp.path().join("src")).expect("source directory");
    fs::write(
        temp.path().join("tsconfig.json"),
        r#"{"compilerOptions":{"baseUrl":".","paths":{"@/*":["src/*"]}}}"#,
    )
    .expect("aliases");
    fs::write(
        temp.path().join("src/a.ts"),
        "import {b} from '@/b'; export const a = b;",
    )
    .expect("source a");
    fs::write(
        temp.path().join("src/b.ts"),
        "import {a} from '@/a'; export const b = a;",
    )
    .expect("source b");
    let absolute = temp.path().join("tsconfig.json");
    for config in [None, Some("tsconfig.json"), absolute.to_str()] {
        let mut arguments = vec!["--json", "--inner"];
        if let Some(path) = config {
            arguments.extend(["--tsconfig", path]);
        }
        let output = scan(temp.path(), &arguments);
        assert_eq!(
            output.status.code(),
            Some(1),
            "aliases must retain cycles for {config:?}: {output:?}"
        );
        let json: serde_json::Value = serde_json::from_slice(&output.stdout).expect("scan JSON");
        let path = json["file_cycles"][0]["cycles"][0][0]
            .as_str()
            .expect("cycle path");
        assert!(
            !Path::new(path).is_absolute(),
            "CLI cycle paths stay relative to the reported package"
        );
    }
}

fn assert_error(output: &Output) {
    assert_eq!(
        output.status.code(),
        Some(2),
        "failure must exit 2: {output:?}"
    );
    assert!(
        output.stdout.is_empty(),
        "failed scan must not emit a successful report"
    );
    assert!(!output.stderr.is_empty(), "failure should be diagnosed");
}

#[test]
fn invalid_configuration_is_not_a_clean_scan() {
    let temp = tempdir().expect("temporary directory");
    package(temp.path());
    for yaml in [
        "scan: [",
        "typo: true",
        "scan:\n  incldue: [src]",
        "defaults:\n  extensions: 5",
        "scan: {}\nscan: {}",
        "packages:\n  '[': {}",
        "packages:\n  '*':\n    ignore: ['[']",
    ] {
        fs::write(temp.path().join("jscycles.yaml"), yaml).expect("config should be written");
        assert_error(&scan(temp.path(), &["--json", "--inner"]));
    }
}

#[test]
fn explicit_extensions_override_yaml_defaults_and_package_settings() {
    let temp = tempdir().expect("temporary directory");
    package(temp.path());
    fs::write(temp.path().join("a.ts"), "import './a';").expect("cycle source");
    fs::write(
        temp.path().join("jscycles.yaml"),
        "defaults:\n  extensions: [js]\npackages:\n  example:\n    extensions: [js]\n",
    )
    .expect("configuration");
    assert_eq!(
        scan(temp.path(), &["--inner"]).status.code(),
        Some(0),
        "YAML excludes TypeScript"
    );
    assert_eq!(
        scan(temp.path(), &["--inner", "--extensions", "ts"])
            .status
            .code(),
        Some(1),
        "explicit CLI extensions must override YAML"
    );
}

#[test]
fn optional_absent_config_and_explicit_missing_config_differ() {
    let temp = tempdir().expect("temporary directory");
    package(temp.path());
    let clean = scan(temp.path(), &["--json", "--inner"]);
    assert_eq!(
        clean.status.code(),
        Some(0),
        "absent default config is optional"
    );
    let json: serde_json::Value = serde_json::from_slice(&clean.stdout).expect("valid JSON");
    assert_eq!(
        json["has_cycles"], false,
        "clean JSON must agree with exit status"
    );
    assert_error(&scan(temp.path(), &["--config", "missing.yaml", "--inner"]));
    assert_error(&scan(
        temp.path(),
        &["--tsconfig", "missing.json", "--inner"],
    ));
}

#[test]
fn explicit_tsconfig_errors_are_reported_in_outer_mode() {
    let temp = tempdir().expect("temporary directory");
    workspace(temp.path(), false, false);
    let output = scan(temp.path(), &["--outer", "--tsconfig", "missing.json"]);
    assert_error(&output);
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("missing.json"),
        "diagnostic identifies the selected input"
    );
    assert_error(&scan(
        temp.path(),
        &["--no-tsconfig", "--tsconfig", "missing.json"],
    ));
}

#[test]
fn missing_scan_root_is_an_error() {
    let temp = tempdir().expect("temporary directory");
    assert_error(&scan(temp.path(), &["does-not-exist", "--inner"]));
}

#[test]
fn unsupported_file_operand_is_not_a_clean_scan() {
    let temp = tempdir().expect("temporary directory");
    package(temp.path());
    assert_error(&scan(temp.path(), &["a.ts", "--inner"]));
}

#[test]
fn cycles_keep_the_existing_exit_and_json_contract() {
    let temp = tempdir().expect("temporary directory");
    package(temp.path());
    fs::write(temp.path().join("a.ts"), "import './a';").expect("cycle source");
    let output = scan(temp.path(), &["--json", "--inner"]);
    assert_eq!(output.status.code(), Some(1), "cycles must exit 1");
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).expect("valid JSON");
    assert_eq!(
        json["has_cycles"], true,
        "cycle report must agree with exit status"
    );
}

#[test]
fn invalid_utf8_stdin_is_not_silently_truncated() {
    let temp = tempdir().expect("temporary directory");
    package(temp.path());
    let mut child = Command::new(env!("CARGO_BIN_EXE_jscycles"))
        .current_dir(temp.path())
        .args(["--stdin", "--inner", "--json"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("CLI should start");
    child
        .stdin
        .take()
        .expect("stdin pipe")
        .write_all(b".\n\xff\n")
        .expect("write input");
    assert_error(&child.wait_with_output().expect("CLI should finish"));
}

#[cfg(target_os = "linux")]
#[test]
fn real_output_failure_is_nonzero() {
    let temp = tempdir().expect("temporary directory");
    package(temp.path());
    let output = Command::new(env!("CARGO_BIN_EXE_jscycles"))
        .current_dir(temp.path())
        .args(["--json", "--inner"])
        .stdout(
            fs::OpenOptions::new()
                .write(true)
                .open("/dev/full")
                .expect("open /dev/full"),
        )
        .output()
        .expect("CLI should finish");
    assert_eq!(
        output.status.code(),
        Some(2),
        "write/flush failure must exit 2"
    );
}

#[cfg(unix)]
#[test]
fn closed_stdout_is_an_output_error() {
    use std::os::fd::OwnedFd;
    use std::os::unix::net::UnixStream;

    let temp = tempdir().expect("temporary directory");
    package(temp.path());
    for arguments in [vec!["--json", "--inner"], vec!["--help"], vec!["--version"]] {
        let (reader, writer) = UnixStream::pair().expect("stdout socket");
        // Force an OS write error without depending on peer-close propagation or
        // concurrent child-descriptor inheritance timing on Unix socket pairs.
        writer
            .shutdown(std::net::Shutdown::Write)
            .expect("disable stdout writes");
        drop(reader);
        let output = Command::new(env!("CARGO_BIN_EXE_jscycles"))
            .current_dir(temp.path())
            .args(arguments)
            .stdout(Stdio::from(OwnedFd::from(writer)))
            .output()
            .expect("CLI should finish");
        assert_eq!(output.status.code(), Some(2), "output failure must exit 2");
        assert!(
            String::from_utf8_lossy(&output.stderr).contains("Error"),
            "output failure retains an operational diagnostic"
        );
    }
}

/// The README's usage block is published as the website's command-line
/// reference, so it must stay identical to the real `--help` text.
#[test]
fn readme_usage_block_matches_help_output() {
    let output = Command::new(env!("CARGO_BIN_EXE_jscycles"))
        .arg("--help")
        .output()
        .expect("CLI should start");
    assert!(output.status.success(), "--help should succeed");
    let help = String::from_utf8(output.stdout).expect("help is UTF-8");
    let readme = include_str!("../README.md").replace("\r\n", "\n");
    let (_, after) = readme
        .split_once("```text\nUsage: jscycles")
        .expect("README.md has a usage block");
    let (rest, _) = after.split_once("```").expect("usage block is closed");
    let usage = format!("Usage: jscycles{rest}");
    assert!(
        help.ends_with(&usage),
        "README.md's usage block differs from `jscycles --help`; replace it with:\n{help}"
    );
}

/// A scan that selects no package has checked nothing, so it must not pass as
/// clean: a CI job started in the wrong directory would otherwise succeed.
#[test]
fn an_empty_selection_is_an_error_not_a_clean_scan() {
    let temp = tempdir().expect("temporary directory");
    for arguments in [&[][..], &["--json", "--quiet"][..], &["--outer"][..]] {
        let output = scan(temp.path(), arguments);
        assert_eq!(output.status.code(), Some(2), "{arguments:?}");
        assert!(output.stdout.is_empty(), "no report for {arguments:?}");
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains("no packages to check"), "{stderr}");
        assert!(!stderr.contains("--only"), "no filter was given: {stderr}");
    }

    package(temp.path());
    let output = scan(temp.path(), &["--only", "@nothing/*"]);
    assert_eq!(
        output.status.code(),
        Some(2),
        "a filter that matches nothing"
    );
    assert!(
        String::from_utf8_lossy(&output.stderr)
            .contains("no package name matches --only/--exclude"),
        "the message names the filters"
    );
    assert_eq!(
        scan(temp.path(), &[]).status.code(),
        Some(0),
        "the package itself is clean"
    );
}

/// Release build records hash `Cargo.lock`, so every runner must check it out
/// byte for byte the same. `.gitattributes` keeps LF endings on Windows,
/// where a CRLF checkout once made the release reject the Windows binary.
#[test]
fn lockfile_has_lf_line_endings_on_every_platform() {
    let lock = include_bytes!("../Cargo.lock");
    assert!(
        !lock.contains(&b'\r'),
        "Cargo.lock was checked out with CRLF; check .gitattributes"
    );
}
