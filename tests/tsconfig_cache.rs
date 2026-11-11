//! Configuration search context, inherited snapshots, depth and shared quotas.
#![allow(clippy::expect_used, reason = "Fixture failures must fail the test")]

use std::fs;
use std::path::Path;
use std::process::Command;
use std::sync::{Arc, Barrier};

use jscycles::budget::{AnalysisBudget, AnalysisLimits, Resource};
use jscycles::{TsConfig, TsConfigCache};

#[test]
fn relative_package_operands_find_ancestor_configuration() {
    let temp = tempfile::tempdir().expect("fixture");
    let root = temp.path();
    let package = root.join("sources/pkg");
    fs::create_dir_all(&package).expect("package");
    fs::write(root.join("jscycles.yaml"), "{}").expect("configuration");
    fs::write(
        root.join("tsconfig.json"),
        r#"{"compilerOptions":{"paths":{"@target":["sources/pkg/target"]}}}"#,
    )
    .expect("ancestor configuration");
    fs::write(package.join("package.json"), r#"{"name":"context"}"#).expect("manifest");
    fs::write(package.join("entry.ts"), "import '@target';").expect("importer");
    fs::write(package.join("target.ts"), "import './entry';").expect("cycle");
    let caller = root.join("sources/runner");
    fs::create_dir(&caller).expect("sibling caller");
    fs::write(
        caller.join("tsconfig.json"),
        r#"{"compilerOptions":{"paths":{"@target":["decoy"]}}}"#,
    )
    .expect("unrelated caller configuration");
    fs::write(caller.join("decoy.ts"), "").expect("decoy leaf");
    for (cwd, operand) in [
        (root.join("sources"), Path::new("pkg")),
        (root.join("sources"), Path::new("./pkg")),
        (root.join("sources"), package.as_path()),
        (caller.clone(), Path::new("../pkg")),
        (caller, package.as_path()),
    ] {
        let result = Command::new(env!("CARGO_BIN_EXE_jscycles"))
            .current_dir(cwd)
            .arg(operand)
            .args(["--inner", "--json", "--config"])
            .arg(root.join("jscycles.yaml"))
            .output()
            .expect("CLI starts");
        assert_eq!(
            result.status.code(),
            Some(1),
            "ancestor aliases retain the cycle: {}",
            String::from_utf8_lossy(&result.stderr)
        );
        let report: serde_json::Value = serde_json::from_slice(&result.stdout).expect("JSON");
        assert_eq!(
            report["has_file_cycles"], true,
            "relative and absolute selection agree"
        );
    }
    for spelling in [package, root.join("sources/runner/../pkg")] {
        assert!(
            TsConfig::discover(&spelling)
                .expect("standalone ancestor search")
                .resolve_alias("@target")
                .expect("selected alias")
                .ends_with("sources/pkg/target.ts"),
            "standalone discovery walks the actual selected directory's ancestors"
        );
    }
}

#[test]
fn relative_cache_keys_include_the_working_directory() {
    const CHILD_ROOT: &str = "JSCYCLES_TEST_CACHE_CONTEXT_ROOT";
    if let Some(root) = std::env::var_os(CHILD_ROOT) {
        // This filtered subprocess owns its CWD; parallel parent tests never
        // observe these changes. Only successful snapshots are reused on delete.
        let root = std::path::PathBuf::from(root);
        let cache = TsConfigCache::new();
        let discovery = TsConfigCache::new();
        for name in ["a", "b", "a"] {
            std::env::set_current_dir(root.join(name)).expect("child CWD");
            let found = cache
                .load(Path::new("tsconfig.json"))
                .expect("contextual load");
            #[cfg(windows)]
            {
                let current = std::env::current_dir().expect("current directory");
                if let Some(std::path::Component::Prefix(prefix)) = current.components().next()
                    && let std::path::Prefix::Disk(drive) | std::path::Prefix::VerbatimDisk(drive) =
                        prefix.kind()
                {
                    let drive_relative = format!("{}:tsconfig.json", char::from(drive));
                    assert!(
                        cache
                            .load(Path::new(&drive_relative))
                            .expect("drive-relative load")
                            .resolve_alias("@target")
                            .expect("drive target")
                            .ends_with(format!("{name}.ts")),
                        "drive-relative keys use the current drive context"
                    );
                }
            }
            assert!(
                found
                    .resolve_alias("@target")
                    .expect("target")
                    .ends_with(format!("{name}.ts")),
                "do not reuse another CWD's config"
            );
            assert!(
                TsConfig::discover(Path::new("pkg"))
                    .expect("standalone relative search")
                    .resolve_alias("@target")
                    .expect("standalone target")
                    .ends_with(format!("{name}.ts")),
                "standalone discovery has the same origin"
            );
            assert!(
                discovery
                    .discover(Path::new("pkg"))
                    .expect("contextual discovery")
                    .resolve_alias("@target")
                    .expect("discovered target")
                    .ends_with(format!("{name}.ts")),
                "directory cache keys include CWD"
            );
        }
        fs::remove_file("tsconfig.json").expect("remove original after caching");
        assert!(
            cache.load(Path::new("tsconfig.json")).is_some(),
            "a cached successful snapshot does not require a live file"
        );
        return;
    }
    let temp = tempfile::tempdir().expect("fixture");
    for name in ["a", "b"] {
        let root = temp.path().join(name);
        fs::create_dir_all(root.join("pkg")).expect("package directory");
        fs::write(
            root.join("tsconfig.json"),
            serde_json::json!({"compilerOptions":{"paths":{"@target":[format!("{name}.ts")]}}})
                .to_string(),
        )
        .expect("configuration");
        fs::write(root.join(format!("{name}.ts")), "").expect("target");
    }
    let result = Command::new(std::env::current_exe().expect("test binary"))
        .args([
            "--exact",
            "relative_cache_keys_include_the_working_directory",
            "--test-threads=1",
        ])
        .env(CHILD_ROOT, temp.path())
        .output()
        .expect("isolated CWD test");
    assert!(
        result.status.success(),
        "child failed: {} {}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );
}

#[test]
fn concurrent_children_read_one_parent_and_keep_their_own_overrides() {
    let temp = tempfile::tempdir().expect("fixture");
    let root = fs::canonicalize(temp.path()).expect("root");
    fs::write(
        root.join("base.json"),
        r#"{"compilerOptions":{"baseUrl":".","paths":{"@shared":["shared"]}}}"#,
    )
    .expect("base");
    fs::write(root.join("shared.ts"), "").expect("shared target");
    for number in 0..16 {
        let child = root.join(format!("pkg{number}"));
        fs::create_dir(&child).expect("child");
        let value = match number % 5 {
            0 => {
                serde_json::json!({"extends":"../base.json", "compilerOptions":{"baseUrl":".","paths":{"@child":["local"]}}})
            },
            1 => serde_json::json!({"extends":"../base.json"}),
            2 => {
                serde_json::json!({"extends":"../base.json", "compilerOptions":{"baseUrl":".","paths":{}}})
            },
            3 => serde_json::json!({"extends":"../base.json", "compilerOptions":{"baseUrl":"."}}),
            _ => {
                serde_json::json!({"extends":"../base.json", "compilerOptions":{"paths":{"@child":[format!("pkg{number}/local")]}}})
            },
        };
        fs::write(child.join("tsconfig.json"), value.to_string()).expect("child config");
        fs::write(child.join("local.ts"), "").expect("local target");
        fs::write(child.join("shared.ts"), "").expect("rebased inherited target");
    }
    let budget = AnalysisBudget::new(AnalysisLimits {
        max_files: 17,
        ..AnalysisLimits::default()
    })
    .expect("one parent plus sixteen child reads");
    let cache = TsConfigCache::new().with_budget(budget.clone());
    let barrier = Arc::new(Barrier::new(16));
    let workers: Vec<_> = (0..16)
        .map(|number| {
            let child = root.join(format!("pkg{number}"));
            let selected_cache = cache.clone();
            let start = Arc::clone(&barrier);
            std::thread::spawn(move || {
                let _ = start.wait();
                let config = selected_cache.discover_shared(&child);
                (number, child, config)
            })
        })
        .collect();
    for worker in workers {
        let (number, child, config) = worker.join().expect("worker completes");
        let config = config.expect("shared parent stays inside read quota");
        if matches!(number % 5, 0 | 4) {
            assert_eq!(
                config.resolve_alias("@child"),
                Some(child.join("local.ts")),
                "child fields override parent fields"
            );
            assert!(
                config.resolve_alias("@shared").is_none(),
                "child paths replaces the parent option"
            );
        } else if number % 5 == 2 {
            assert!(
                config.paths.is_empty(),
                "empty child paths also replaces parent mappings"
            );
        } else {
            assert_eq!(
                config.resolve_alias("@shared"),
                Some(if number % 5 == 3 {
                    child.join("shared.ts")
                } else {
                    root.join("shared.ts")
                }),
                "baseUrl and paths inherit independently"
            );
        }
    }
    assert_eq!(
        budget.used(Resource::Files),
        17,
        "read each actual document once"
    );
    budget.check().expect("no hidden duplicate admission");
}

#[test]
fn warm_parent_documents_cannot_bypass_extends_depth_or_cycles() {
    let temp = tempfile::tempdir().expect("fixture");
    let root = temp.path();
    fs::write(root.join("base.json"), "{}").expect("base");
    let cache = TsConfigCache::new();
    let _ = cache.load(&root.join("base.json")).expect("warm parent");
    let mut previous = "./base.json".to_owned();
    for number in 1..=10 {
        let name = format!("layer{number}.json");
        fs::write(
            root.join(&name),
            serde_json::json!({"extends":previous}).to_string(),
        )
        .expect("layer");
        let loaded = cache.load(&root.join(&name));
        assert_eq!(
            loaded.is_some(),
            number < 10,
            "depth counts cached documents too"
        );
        previous = format!("./{name}");
    }
    fs::write(root.join("cycle-a.json"), r#"{"extends":"./cycle-b.json"}"#).expect("cycle A");
    fs::write(root.join("cycle-b.json"), r#"{"extends":"./cycle-a.json"}"#).expect("cycle B");
    assert!(
        cache.load(&root.join("cycle-a.json")).is_none(),
        "raw-document initialization does not recurse or deadlock"
    );
}

#[test]
fn parent_shapes_and_retained_document_payloads_still_fail_closed() {
    let temp = tempfile::tempdir().expect("fixture");
    let root = temp.path();
    fs::write(
        root.join("parent.json"),
        r#"{"compilerOptions":{"paths":{"@bad":1}}}"#,
    )
    .expect("malformed typed field");
    fs::write(
        root.join("child.json"),
        r#"{"extends":"./parent.json","compilerOptions":{"paths":{}}}"#,
    )
    .expect("overriding child");
    let malformed = TsConfigCache::new();
    for _ in 0..2 {
        assert!(
            malformed.load(&root.join("child.json")).is_none(),
            "overridden parents still parse known types, including after a cached initialization failure"
        );
    }
    fs::write(
        root.join("valid.json"),
        serde_json::json!({"compilerOptions":{"paths":{"@value":["value".repeat(1024)]}}})
            .to_string(),
    )
    .expect("valid document");
    for limit in [1, 512] {
        let budget = AnalysisBudget::new(AnalysisLimits {
            max_graph_bytes: limit,
            ..AnalysisLimits::default()
        })
        .expect("finite retention quota");
        let cache = TsConfigCache::new().with_budget(budget.clone());
        assert!(
            cache.load(&root.join("valid.json")).is_none(),
            "cache entries and document values are admitted"
        );
        assert_eq!(
            budget.check().expect_err("sticky failure").resource(),
            Resource::GraphBytes,
            "payload admission failure survives optional API"
        );
    }
    // The compiled child is tiny. Its overridden parent's raw strings still
    // live in the shared document cache and must not escape the payload quota.
    fs::write(
        root.join("large-parent.json"),
        serde_json::json!({"compilerOptions":{"paths":{"@unused":["value".repeat(1024)]}}})
            .to_string(),
    )
    .expect("large raw parent");
    fs::write(
        root.join("small-child.json"),
        r#"{"extends":"./large-parent.json","compilerOptions":{"paths":{}}}"#,
    )
    .expect("small compiled child");
    let retention_budget = AnalysisBudget::new(AnalysisLimits {
        max_graph_bytes: 4096,
        ..AnalysisLimits::default()
    })
    .expect("raw retention quota");
    let retained = TsConfigCache::new().with_budget(retention_budget.clone());
    assert!(
        retained.load(&root.join("small-child.json")).is_none(),
        "overridden raw parent data remains retained and must be charged"
    );
    assert_eq!(
        retention_budget
            .check()
            .expect_err("raw payload rejection")
            .resource(),
        Resource::GraphBytes,
        "this rejection is independent of compiled aliases"
    );
}

#[test]
fn rebinding_a_budget_cannot_reuse_another_budgets_raw_or_compiled_snapshots() {
    let temp = tempfile::tempdir().expect("fixture");
    let root = temp.path();
    fs::write(
        root.join("large-parent.json"),
        serde_json::json!({"compilerOptions":{"paths":{"@unused":["value".repeat(1024)]}}})
            .to_string(),
    )
    .expect("large raw parent");
    fs::write(
        root.join("small-child.json"),
        r#"{"extends":"./large-parent.json","compilerOptions":{"paths":{}}}"#,
    )
    .expect("small compiled child");
    for warm_child in [false, true] {
        let original = TsConfigCache::new();
        let _ = original
            .load(&root.join("large-parent.json"))
            .expect("original raw parent snapshot");
        if warm_child {
            let _ = original
                .load(&root.join("small-child.json"))
                .expect("original compiled child snapshot");
        }
        let replacement = AnalysisBudget::new(AnalysisLimits {
            max_total_bytes: 512,
            ..AnalysisLimits::default()
        })
        .expect("new operation's byte quota");
        let rebound = original.clone().with_budget(replacement.clone());
        assert!(
            rebound.load(&root.join("small-child.json")).is_none(),
            "rebinding cannot reuse raw or compiled values paid for by another budget"
        );
        assert_eq!(
            replacement
                .check()
                .expect_err("new budget pays for the parent read")
                .resource(),
            Resource::Bytes
        );
        assert!(
            original.load(&root.join("small-child.json")).is_some(),
            "the original clone keeps its snapshots and healthy shared budget"
        );
    }
}

#[cfg(unix)]
#[test]
fn shared_parent_bytes_keep_each_extends_spelling_origin() {
    let temp = tempfile::tempdir().expect("fixture");
    let root = fs::canonicalize(temp.path()).expect("root");
    fs::create_dir(root.join("shared")).expect("shared directory");
    let shared = root.join("shared/base.json");
    fs::write(
        &shared,
        r#"{"compilerOptions":{"paths":{"@target":["target"]}}}"#,
    )
    .expect("shared document");
    let cache = TsConfigCache::new();
    let _ = cache.load(&shared).expect("warm canonical document");
    for name in ["a", "b"] {
        let origin = root.join(name);
        fs::create_dir(&origin).expect("origin directory");
        std::os::unix::fs::symlink("../shared/base.json", origin.join("parent.json"))
            .expect("parent link");
        fs::write(origin.join("child.json"), r#"{"extends":"./parent.json"}"#)
            .expect("selected child");
        fs::write(origin.join("target.ts"), "").expect("local target");
        assert_eq!(
            cache
                .load(&origin.join("child.json"))
                .expect("child")
                .resolve_alias("@target"),
            Some(origin.join("target.ts")),
            "reuse values without freezing a parent's first origin"
        );
    }

    // Filesystem selection follows a directory symlink before `..`; lexical
    // normalization would choose the unrelated root-level configuration.
    fs::create_dir_all(root.join("physical/deep")).expect("physical parent");
    fs::create_dir(root.join("physical/pkg")).expect("physical package");
    fs::write(
        root.join("physical/tsconfig.json"),
        r#"{"compilerOptions":{"paths":{"@target":["pkg/target"]}}}"#,
    )
    .expect("physical ancestor");
    fs::write(root.join("physical/pkg/target.ts"), "").expect("physical target");
    fs::write(
        root.join("tsconfig.json"),
        r#"{"compilerOptions":{"paths":{"@target":["decoy"]}}}"#,
    )
    .expect("lexical decoy");
    fs::write(root.join("decoy.ts"), "").expect("logical ancestor target");
    std::os::unix::fs::symlink("physical/deep", root.join("linked")).expect("directory link");
    for config in [
        TsConfig::discover(&root.join("linked")),
        cache.discover(&root.join("linked")),
    ] {
        assert_eq!(
            config
                .expect("ordinary spelling retains logical ancestors")
                .resolve_alias("@target"),
            Some(root.join("decoy.ts")),
            "physical parent resolution applies only to parent-containing operands"
        );
    }
    let selected = root.join("linked/../pkg");
    let expected = Some(root.join("physical/pkg/target.ts"));
    assert_eq!(
        TsConfig::discover(&selected)
            .expect("standalone physical search")
            .resolve_alias("@target"),
        expected
    );
    assert_eq!(
        cache
            .discover(&selected)
            .expect("cached physical search")
            .resolve_alias("@target"),
        expected
    );
    fs::remove_file(root.join("physical/tsconfig.json")).expect("remove after snapshot");
    fs::remove_file(root.join("linked")).expect("remove link after snapshot");
    assert_eq!(
        cache
            .discover(&selected)
            .expect("logical discovery key retains a successful snapshot")
            .resolve_alias("@target"),
        expected
    );
}
