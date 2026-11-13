//! The embedding API: a library [`Scan`] must give the same answer as the
//! command, byte for byte, because the command is built on it.
#![allow(
    clippy::expect_used,
    reason = "Test setup failures should fail the test"
)]

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use jscycles::{Config, CycleFilter, DiscoveryError, JscyclesError, Scan, TsConfigSource};
use tempfile::tempdir;

const FIXTURES: [&str; 7] = [
    "simple-cycle",
    "no-cycles",
    "inter-package-cycle",
    "inter-package-no-cycle",
    "monorepo-mock",
    "pnpm-workspace",
    "tsconfig-aliases",
];

/// Command-line options and the equivalent library settings.
type Variant = (Vec<&'static str>, fn(Scan) -> Scan);

fn variants() -> Vec<Variant> {
    vec![
        (vec![], |scan| scan),
        (vec!["--inner"], |scan| scan.filter(CycleFilter::Inner)),
        (vec!["--outer"], |scan| scan.filter(CycleFilter::Outer)),
        (vec!["--verbose"], |scan| scan.verbose(true)),
        (vec!["--skip-type-imports"], |scan| {
            scan.skip_type_imports(true)
        }),
        (vec!["--extensions", "ts"], |scan| scan.extensions(["ts"])),
        (vec!["--no-tsconfig"], |scan| {
            scan.tsconfig(TsConfigSource::Disabled)
        }),
    ]
}

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

fn cli(cwd: &Path, arguments: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_jscycles"))
        .current_dir(cwd)
        .args(arguments)
        .output()
        .expect("CLI should start")
}

fn status_for(has_cycles: bool) -> i32 {
    i32::from(has_cycles)
}

#[test]
fn explicit_package_scans_match_the_command_byte_for_byte() {
    // An empty working directory, so no ambient jscycles.yaml applies.
    let cwd = tempdir().expect("temporary directory");
    for name in FIXTURES {
        let root = fixture(name);
        let root_text = root.to_str().expect("UTF-8 fixture path");
        for (flags, configure) in variants() {
            let scan = configure(Scan::packages([&root]));
            let report = scan.run().expect("fixture scans succeed");
            let context = format!("{name} {flags:?}");

            let mut json_arguments = vec!["--json"];
            json_arguments.extend(&flags);
            json_arguments.push(root_text);
            let output = cli(cwd.path(), &json_arguments);
            assert_eq!(
                report.to_json().expect("JSON fits"),
                output.stdout,
                "JSON for {context}"
            );
            assert_eq!(
                output.status.code(),
                Some(status_for(report.has_cycles())),
                "status for {context}"
            );

            let mut human_arguments = flags.clone();
            human_arguments.push(root_text);
            assert_eq!(
                report.to_human(false).expect("output fits"),
                cli(cwd.path(), &human_arguments).stdout,
                "human output for {context}"
            );
        }
    }
}

#[test]
fn root_scans_match_the_command_run_from_that_directory() {
    for name in FIXTURES {
        let root = fixture(name);
        let config = Config::try_load_or_default(&root.join("jscycles.yaml"))
            .expect("fixture configuration loads");
        let report = Scan::new(&root)
            .config(config)
            .run()
            .expect("fixture scans succeed");
        // The command reports paths under `.`; the library under the root it
        // was given. JSON escapes the root the same way in both.
        let escaped = serde_json::to_string(root.to_str().expect("UTF-8 fixture path"))
            .expect("path encodes");
        let escaped = escaped.trim_matches('"');
        let library = String::from_utf8(report.to_json().expect("JSON fits"))
            .expect("JSON is UTF-8")
            .replace(escaped, ".");
        let output = cli(&root, &["--json"]);
        assert_eq!(
            library,
            String::from_utf8(output.stdout).expect("JSON is UTF-8"),
            "{name}"
        );
        assert_eq!(
            output.status.code(),
            Some(status_for(report.has_cycles())),
            "{name}"
        );
    }
}

#[test]
fn results_are_structured_data() {
    let report = Scan::new(fixture("inter-package-cycle"))
        .run()
        .expect("scan succeeds");
    assert!(report.has_cycles(), "the fixture has a package cycle");
    let cycles = &report.results().package_cycles;
    assert!(!cycles.is_empty(), "package cycles are reported");
    for cycle in cycles {
        assert_eq!(
            cycle.packages.first(),
            cycle.packages.last(),
            "a cycle starts and ends with the same package"
        );
    }
    let owned = report.clone().into_results();
    assert_eq!(
        owned.package_cycles, *cycles,
        "into_results keeps the cycles"
    );
    assert_eq!(
        owned.file_results.len(),
        report.results().file_results.len(),
        "and every package"
    );
    let verbose = Scan::new(fixture("inter-package-cycle"))
        .verbose(true)
        .run()
        .expect("scan succeeds");
    assert!(
        verbose.results().package_cycles.is_empty(),
        "verbose moves package cycles"
    );
    assert_eq!(
        verbose.results().package_cycles_with_files.len(),
        cycles.len(),
        "with their files"
    );
    assert!(
        verbose
            .results()
            .package_cycles_with_files
            .iter()
            .all(|cycle| cycle.edges.iter().all(|edge| !edge.files.is_empty())),
        "every package edge names the files behind it"
    );
}

#[test]
fn a_selection_with_no_packages_is_an_error_not_a_clean_report() {
    let empty = tempdir().expect("temporary directory");
    let result = Scan::new(empty.path()).run();
    assert!(
        matches!(
            result,
            Err(JscyclesError::Discovery(DiscoveryError::NoPackages {
                filtered: false
            }))
        ),
        "{result:?}"
    );
    let filtered = Scan::new(fixture("monorepo-mock"))
        .only(["no-such-package"])
        .run();
    assert!(
        matches!(
            filtered,
            Err(JscyclesError::Discovery(DiscoveryError::NoPackages {
                filtered: true
            }))
        ),
        "{filtered:?}"
    );
}

#[test]
fn invalid_settings_are_errors() {
    for extension in ["", ".ts", "t s"] {
        let result = Scan::new(fixture("simple-cycle"))
            .extensions([extension])
            .run();
        assert!(
            matches!(result, Err(JscyclesError::Io(ref error)) if error.kind() == std::io::ErrorKind::InvalidInput),
            "extension {extension:?}: {result:?}"
        );
    }
    let missing = Scan::new(fixture("simple-cycle"))
        .tsconfig(TsConfigSource::File(
            fixture("simple-cycle").join("missing.json"),
        ))
        .run();
    assert!(missing.is_err(), "a required tsconfig must exist");
    let mut config = Config::default();
    config.limits.max_files = 0;
    assert!(
        Scan::new(fixture("simple-cycle"))
            .config(config)
            .run()
            .is_err(),
        "an invalid limit is rejected, not ignored"
    );
}

#[test]
fn exhausted_limits_are_errors_not_partial_results() {
    let mut config = Config::default();
    config.limits.max_files = 1;
    let result = Scan::new(fixture("monorepo-mock")).config(config).run();
    assert!(
        result
            .as_ref()
            .is_err_and(|error| error.to_string().contains("limit")),
        "{result:?}"
    );
}

#[test]
fn an_ignored_tsconfig_is_reported_to_the_caller() {
    let temp = tempdir().expect("temporary directory");
    // Outside any workspace, a package's own unusable tsconfig is skipped
    // with a warning rather than failing the scan.
    let root = temp.path().join("app");
    fs::create_dir(&root).expect("package directory");
    let root = root.as_path();
    fs::write(root.join("package.json"), r#"{"name":"app"}"#).expect("manifest");
    fs::write(root.join("tsconfig.json"), "{ not json").expect("tsconfig");
    fs::write(root.join("a.ts"), "import './b';").expect("source");
    fs::write(root.join("b.ts"), "import './a';").expect("source");
    let report = Scan::new(temp.path())
        .run()
        .expect("a bad discovered tsconfig is tolerated");
    assert!(report.has_cycles(), "relative imports still form the cycle");
    let ignored = report.ignored_tsconfigs();
    assert_eq!(ignored.len(), 1, "{ignored:?}");
    assert!(
        ignored
            .first()
            .is_some_and(|(path, _)| path.ends_with("tsconfig.json")),
        "{ignored:?}"
    );
}

#[test]
fn packages_from_different_workspaces_are_rejected() {
    let first = fixture("pnpm-workspace");
    let second = fixture("monorepo-mock");
    let result = Scan::packages([first.join("packages"), second.join("packages")]).run();
    assert!(
        matches!(result, Err(JscyclesError::Io(ref error)) if error.kind() == std::io::ErrorKind::InvalidInput),
        "{result:?}"
    );
}
