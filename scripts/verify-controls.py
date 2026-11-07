#!/usr/bin/env python3
"""Exercise adopted controls with passing, defective, and repaired disposable copies."""

import sys

if sys.version_info < (3, 11):
    raise SystemExit('verify-controls.py requires Python 3.11+ (tomllib)')

import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import shutil
import subprocess
import tempfile
import time
import tomllib


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--toolchain", default="1.98.1")
    parser.add_argument("--output", default="docs/maintainers/evidence/rust-oe-validation.json")
    parser.add_argument("--boundaries", action="store_true",
                        help="Also replay selected configuration/workspace faults in the disposable copy")
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[1]
    report = {"schema": 1, "toolchain": args.toolchain, "host": platform.platform(),
              "rust_oe_revision": "19e4133c0461129b0f5fdb286abd7b87511242e2", "cases": []}
    report["input_sha256"] = {
        str(path.relative_to(root)): hashlib.sha256(path.read_bytes()).hexdigest()
        for path in sorted(root.rglob("*"))
        if path.is_file() and "target" not in path.parts and ".git" not in path.parts
        and (path.suffix in (".rs", ".py") or str(path.relative_to(root)) in
             {"Cargo.toml", "Cargo.lock", ".cargo/config.toml", "xtask/Cargo.toml", "rust-toolchain.toml", "rustfmt.toml", "clippy.toml", "rust-oe.toml", "rust-oe.lock.json", "AGENTS.md", ".github/workflows/ci.yml", ".github/workflows/release.yml", ".github/workflows/maintenance.yml", "fuzz/Cargo.toml", "fuzz/Cargo.lock", "fuzz/deny.exceptions.toml", "deny.toml"})
    }
    environment = dict(os.environ, RUSTUP_TOOLCHAIN=args.toolchain,
                       CARGO_TARGET_DIR=str(root / "target/tooling/control-pilot"))

    with tempfile.TemporaryDirectory(prefix="jscycles-controls-") as directory:
        project = Path(directory) / "project"
        shutil.copytree(root, project, ignore=shutil.ignore_patterns(
            ".git", "target", ".claude-flow", ".hive-mind", "__pycache__", "artifacts"))

        def run(name, task=None, expected=True, diagnostic=None, command=None):
            command = command or ["cargo", "xtask", task]
            start = time.monotonic()
            result = subprocess.run(command, cwd=project, env=environment,
                                    capture_output=True, text=True, timeout=180)
            matched = (result.returncode == 0) == expected
            if diagnostic:
                matched = matched and diagnostic in result.stdout + result.stderr
            report["cases"].append({"name": name, "command": command,
                                    "exit": result.returncode, "matched": matched,
                                    "seconds": round(time.monotonic() - start, 3),
                                    "output_tail": (result.stdout + result.stderr)[-1800:]})
            print(f"{name}: {'passed' if matched else 'FAILED'}", flush=True)

        run("C01 passing", "fmt")
        tool = project / "xtask/src/main.rs"
        original = tool.read_text()
        tool.write_text(original + "\nfn formatting_probe( ){ }\n")
        run("C01 rejects unformatted xtask member", "fmt", False, "Diff")
        tool.write_text(original)
        run("C01 repaired", "fmt")

        run("C02 passing", "check")
        manifest = project / "Cargo.toml"
        original_manifest = manifest.read_text()
        version = tomllib.loads(original_manifest)["package"]["version"]
        manifest.write_text(original_manifest.replace(f'version = "{version}"', 'version = "0.0.0-rust-oe-probe"', 1))
        run("C02 rejects stale lock", "check", False, "locked")
        manifest.write_text(original_manifest)
        run("C02 repaired", "check")

        library = project / "src/lib.rs"
        original_library = library.read_text()
        run("C03 passing", "doctest")
        library.write_text(original_library.replace("assert_eq!(cycles.len(), 1)", "assert_eq!(cycles.len(), 2)", 1))
        run("C03 rejects incorrect public example", "doctest", False, "FAILED")
        library.write_text(original_library)
        run("C03 repaired", "doctest")

        # Compile only. The function is never invoked, and no invalid pointer is made.
        manifest.write_text(original_manifest.replace('unsafe-code = "forbid"', 'unsafe-code = "allow"'))
        library.write_text(original_library + "\nunsafe fn operation_probe(pointer: *const u8) -> u8 { *pointer }\n")
        run("C04 rejects implicit unsafe operation", "check", False, "unsafe-op-in-unsafe-fn")
        library.write_text(original_library + "\nunsafe fn operation_probe(pointer: *const u8) -> u8 { unsafe { *pointer } }\n")
        run("C04 explicit operation compiles", "check")
        library.write_text(original_library)
        manifest.write_text(original_manifest)
        run("C04 restored safe-only project", "check")

        run("Fuzz dependency policy passing", "deny")
        exceptions = project / "fuzz/deny.exceptions.toml"
        original_exceptions = exceptions.read_text()
        exceptions.write_text("exceptions = []\n")
        run("Fuzz dependency policy rejects an unapproved license", "deny", False, "license is not explicitly allowed")
        exceptions.write_text(original_exceptions)
        run("Fuzz dependency policy repaired", "deny")

        guard = [sys.executable, "-B", "scripts/check_adoption.py"]
        run("Guidance integrity passing", command=guard)
        index = project / ".rust-oe/INDEX.md"
        original_index = index.read_bytes()
        index.write_bytes(original_index + b"\nUnreviewed edit\n")
        run("Guidance integrity rejects drift", expected=False, diagnostic="content drift", command=guard)
        index.write_bytes(original_index)
        run("Guidance integrity repaired", command=guard)
        run("Guidance guard pass/defect/repair tests", command=[
            sys.executable, "-B", "-m", "unittest", "discover", "-s", "scripts", "-p", "test_*.py"])
        run("Baseline rejects unrecognized selection arguments", expected=False,
            diagnostic="no extra arguments", command=["cargo", "xtask", "check", "--unexpected"])

        if args.boundaries:
            # These are named defect/repair controls, not broad mutation coverage.
            # Only the disposable copy is changed, one fault at a time. Each
            # command has run()'s 180-second subprocess budget and Cargo scope.
            probes = [
                ("Package identity", "src/discovery.rs", "if previous_path != canonical {",
                 "if previous_path == canonical {",
                 "duplicate_names_fail_without_workspace_metadata_in_every_output_mode"),
                ("Workspace glob components", "src/workspace.rs", "require_literal_separator: true",
                 "require_literal_separator: false", "workspace_star_matches_one_directory_component"),
            ]
            if os.name == "posix":
                probes.append(("Native alias bytes", "src/tsconfig.rs", "return Some(resolved);",
                               "return Some(PathBuf::from(resolved.to_string_lossy().into_owned()));",
                               "alias_results_preserve_native_origin_bytes"))
            else:
                report["omitted_boundary_controls"] = ["Native alias bytes requires Unix OsString byte construction"]
            for name, relative, before, after, test in probes:
                path = project / relative
                content = path.read_text()
                if before not in content:
                    raise SystemExit(f"Selected fault no longer matches {relative}; review the probe")
                command = ["cargo", "test", "--locked", "--test", "boundary_closure", test, "--", "--exact"]
                run(f"{name} passing", command=command, diagnostic="1 passed")
                path.write_text(content.replace(before, after, 1))
                run(f"{name} rejects injected fault", command=command, expected=False, diagnostic="1 failed")
                path.write_text(content)
                run(f"{name} repaired", command=command, diagnostic="1 passed")

    report["source_unchanged"] = all(
        hashlib.sha256((root / path).read_bytes()).hexdigest() == expected
        for path, expected in report["input_sha256"].items())
    report["passed"] = report["source_unchanged"] and all(case["matched"] for case in report["cases"])
    output = root / args.output
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text(json.dumps(report, indent=2) + "\n")
    if not report["passed"]:
        raise SystemExit("A control failed its expected behavior; inspect the report")


if __name__ == "__main__":
    main()
